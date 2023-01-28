#![allow(clippy::type_complexity)]
#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
#![feature(nonzero_min_max)]
#![feature(result_option_inspect)]
#![feature(return_position_impl_trait_in_trait)]
#![feature(type_alias_impl_trait)]

mod domain;
mod infra;

use crate::infra::account::in_mem_ids_projection::InMemAccountIdsProjection;
use anyhow::{Context, Result};
use configured::Configured;
#[cfg(feature = "nats")]
use eventsourced_nats::{NatsEvtLog, NatsEvtLogConfig, NatsSnapshotStore, NatsSnapshotStoreConfig};
#[cfg(feature = "postgres")]
use eventsourced_postgres::{
    PostgresEvtLog, PostgresEvtLogConfig, PostgresSnapshotStore, PostgresSnapshotStoreConfig,
};
use infra::{
    account::lru_cache_factory::{self, LruCacheAccountFactory},
    server,
};
use serde::Deserialize;
use std::{error::Error, future::Future};
use tokio::{select, signal};
use tracing::{debug, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    server: server::Config,

    #[cfg(feature = "nats")]
    evt_log: NatsEvtLogConfig,
    #[cfg(feature = "postgres")]
    evt_log: PostgresEvtLogConfig,

    #[cfg(feature = "nats")]
    snapshot_store: NatsSnapshotStoreConfig,
    #[cfg(feature = "postgres")]
    snapshot_store: PostgresSnapshotStoreConfig,

    account_factory: lru_cache_factory::Config,
}

pub async fn run() -> Result<()> {
    // Load configuration.
    let config = Config::load();
    if let Err(error) = &config {
        eprintln!(
            "rusty-bank exited with ERROR:\n\tCannot load configuration\n\t{error}\n\t{:?}",
            error.source()
        );
    };
    let config = config?;

    // Initialize tracing.
    init_tracing()?;

    // Log configuration.
    debug!(?config, "Starting");

    // Create event log.
    #[cfg(feature = "nats")]
    let evt_log = NatsEvtLog::new(config.evt_log)
        .await
        .context("Cannot create event log")?;
    #[cfg(feature = "postgres")]
    let evt_log = PostgresEvtLog::new(config.evt_log)
        .await
        .context("Cannot create event log")?;

    // Create snapshot store.
    #[cfg(feature = "nats")]
    let snapshot_store = NatsSnapshotStore::new(config.snapshot_store)
        .await
        .context("Cannot create snapshot store")?;
    #[cfg(feature = "postgres")]
    let snapshot_store = PostgresSnapshotStore::new(config.snapshot_store)
        .await
        .context("Cannot create snapshot store")?;

    // Create AccountFactory.
    let account_factory = LruCacheAccountFactory::spawn(
        config.account_factory,
        evt_log.clone(),
        snapshot_store.clone(),
    )
    .await;

    // Create AccountIdsProjection.
    let (account_ids_projection, account_ids_projection_terminated) =
        InMemAccountIdsProjection::new(evt_log).await;

    // Run server.
    let server = server::run(
        config.server,
        account_ids_projection,
        account_factory,
        shutdown_signal(account_ids_projection_terminated),
    );
    info!("Started");
    server.await?;

    Ok(())
}

fn init_tracing() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().json())
        .try_init()
        .context("Cannot initialize tracing")
}

async fn shutdown_signal<F>(account_ids_projection_terminated: F)
where
    F: Future<Output = ()>,
{
    let ctrl_c = async { signal::ctrl_c().await.expect("Failed to listen for ctrl-c") };

    select! {
        _ = account_ids_projection_terminated => {
            warn!("Shutting down, because account IDs projection terminated");
        }
        _ = ctrl_c =>  {
            warn!("Shutting down, because ctrl-c received");
        }
    }
}
