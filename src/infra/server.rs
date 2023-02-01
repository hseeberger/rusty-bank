use super::account::{AccountFactory, AccountIdsProjection};
use crate::domain::{account, euro_cent::EuroCent};
use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::{Path, State},
    headers::{Header, Location},
    http::{HeaderValue, Request, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router, Server, TypedHeader,
};
use serde::Deserialize;
use std::{
    future::Future,
    iter,
    net::{IpAddr, SocketAddr},
};
use tokio::task;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{debug, error, info_span};
use uuid::Uuid;

/// Server configuration.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    addr: IpAddr,
    port: u16,
}

impl Config {
    fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.addr, self.port)
    }
}

/// Run the server with the given [Config].
pub async fn run<P, F, S>(
    config: Config,
    account_ids_projection: P,
    account_factory: F,
    shutdown_signal: S,
) -> Result<()>
where
    P: AccountIdsProjection,
    F: AccountFactory,
    S: Future<Output = ()> + Send + 'static,
{
    let app_state = AppState {
        account_ids_projection,
        account_factory,
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/accounts", post(create_account))
        .route("/accounts/:id/deposits", post(deposit_to_account))
        .route("/accounts/:id/withdrawals", post(withdraw_from_account))
        .with_state(app_state)
        .layer(
            ServiceBuilder::new().layer(TraceLayer::new_for_http().make_span_with(
                |request: &Request<Body>| {
                    let headers = request.headers();
                    info_span!("request", ?headers)
                },
            )),
        );

    task::spawn(
        Server::bind(&config.socket_addr())
            .serve(app.into_make_service())
            .with_graceful_shutdown(shutdown_signal),
    )
    .await
    .map(|server_result| server_result.context("Server completed with error"))
    .context("Server panicked")
    .and_then(|r| r)
}

#[derive(Debug, Clone)]
struct AppState<P, F> {
    account_ids_projection: P,
    account_factory: F,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct Deposit {
    amount: EuroCent,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct Withdraw {
    amount: EuroCent,
}

async fn root() -> impl IntoResponse {
    debug!("Endpoint / invoked");
    StatusCode::OK
}

async fn create_account<P, F>(State(app_state): State<AppState<P, F>>) -> impl IntoResponse
where
    P: AccountIdsProjection,
    F: AccountFactory,
{
    let id = Uuid::now_v7();
    match app_state
        .account_factory
        .get(id)
        .await
        .context("Cannot get Account entity")
    {
        Ok(account) => match account
            .handle_cmd(account::Cmd::Create(id))
            .await
            .context("Cannot handle Create command")
        {
            Ok(Ok(_)) => {
                let location_value = HeaderValue::from_str(&format!("/accounts/{id}")).unwrap();
                let mut location_value = iter::once(&location_value);
                let location = Location::decode(&mut location_value).unwrap();
                (StatusCode::CREATED, TypedHeader(location)).into_response()
            }

            Ok(Err(error)) => (StatusCode::BAD_REQUEST, error.to_string()).into_response(),

            Err(error) => {
                error!(%id, error = format!("{error:#}"), "Cannot create account");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        },

        Err(error) => {
            error!(%id, error = format!("{error:#}"), "Cannot create account");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn deposit_to_account<P, F>(
    State(app_state): State<AppState<P, F>>,
    Path(id): Path<Uuid>,
    Json(Deposit { amount }): Json<Deposit>,
) -> impl IntoResponse
where
    P: AccountIdsProjection,
    F: AccountFactory,
{
    if app_state.account_ids_projection.contains(id).await {
        match app_state
            .account_factory
            .get(id)
            .await
            .context("Cannot get Account entity")
        {
            Ok(account) => {
                let deposit_id = Uuid::now_v7();
                match account
                    .handle_cmd(account::Cmd::Deposit(deposit_id, amount))
                    .await
                    .context("Cannot handle Deposit command")
                {
                    Ok(Ok(_)) => {
                        let location_value =
                            HeaderValue::from_str(&format!("/accounts/{id}/deposits/{deposit_id}"))
                                .unwrap();
                        let mut location_value = iter::once(&location_value);
                        let location = Location::decode(&mut location_value).unwrap();
                        (StatusCode::CREATED, TypedHeader(location)).into_response()
                    }

                    Ok(Err(error)) => (StatusCode::BAD_REQUEST, error.to_string()).into_response(),

                    Err(error) => {
                        error!(%id, error = format!("{error:#}"), "Cannot deposit");
                        StatusCode::INTERNAL_SERVER_ERROR.into_response()
                    }
                }
            }

            Err(error) => {
                error!(%id, error = format!("{error:#}"), "Cannot deposit");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn withdraw_from_account<P, F>(
    State(app_state): State<AppState<P, F>>,
    Path(id): Path<Uuid>,
    Json(Withdraw { amount }): Json<Withdraw>,
) -> impl IntoResponse
where
    P: AccountIdsProjection,
    F: AccountFactory,
{
    if app_state.account_ids_projection.contains(id).await {
        match app_state
            .account_factory
            .get(id)
            .await
            .context("Cannot get Account entity")
        {
            Ok(account) => {
                let withdrawal_id = Uuid::now_v7();
                match account
                    .handle_cmd(account::Cmd::Withdraw(withdrawal_id, amount))
                    .await
                    .context("Cannot handle Withdraw command")
                {
                    Ok(Ok(_)) => {
                        let location_value = HeaderValue::from_str(&format!(
                            "/accounts/{id}/withdrawals/{withdrawal_id}"
                        ))
                        .unwrap();
                        let mut location_value = iter::once(&location_value);
                        let location = Location::decode(&mut location_value).unwrap();
                        (StatusCode::CREATED, TypedHeader(location)).into_response()
                    }

                    Ok(Err(error)) => (StatusCode::BAD_REQUEST, error.to_string()).into_response(),

                    Err(error) => {
                        error!(%id, error = format!("{error:#}"), "Cannot withdraw");
                        StatusCode::INTERNAL_SERVER_ERROR.into_response()
                    }
                }
            }

            Err(error) => {
                error!(%id, error = format!("{error:#}"), "Cannot withdraw");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
