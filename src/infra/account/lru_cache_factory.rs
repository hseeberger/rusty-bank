use super::AccountFactory;
use crate::domain::account::Account;
use anyhow::Context;
use eventsourced::{convert, EntityRef, EventSourcedExt, EvtLog, SnapshotStore};
use lru::LruCache;
use parking_lot::RwLock;
use serde::Deserialize;
use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::Arc,
};
use thiserror::Error;
use tokio::{
    runtime::Handle,
    sync::{mpsc, oneshot},
    task::{self, JoinError},
};
use tracing::error;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LruCacheAccountFactory {
    get_account_sdr: mpsc::Sender<(Uuid, oneshot::Sender<Result<EntityRef<Account>, Error>>)>,
}

impl LruCacheAccountFactory {
    pub async fn spawn<L, S>(config: Config, evt_log: L, snapshot_store: S) -> Self
    where
        L: EvtLog,
        S: SnapshotStore,
    {
        let accounts: Arc<RwLock<LruCache<Uuid, EntityRef<Account>>>> =
            Arc::new(RwLock::new(LruCache::new(config.cache_capacity)));

        let (get_account_sdr, mut get_account_rcv) = mpsc::channel::<(
            Uuid,
            oneshot::Sender<Result<EntityRef<Account>, Error>>,
        )>(config.cache_buffer.get());
        task::spawn(async move {
            while let Some((id, account_sdr)) = get_account_rcv.recv().await {
                let accounts = accounts.clone();
                let evt_log = evt_log.clone();
                let snapshot_store = snapshot_store.clone();

                let account = task::spawn_blocking(move || {
                    accounts
                        .write()
                        .get_or_insert(id, || {
                            Handle::current().block_on(async move {
                                Account::default()
                                    .with_snapshot_after(config.entity_snapshot_after)
                                    .spawn(
                                        id,
                                        config.entity_cmd_buffer,
                                        evt_log,
                                        snapshot_store,
                                        convert::serde_json::binarizer(),
                                    )
                                    .await
                                    .context("Cannot spawn Account entity")
                                    .inspect_err(|error| {
                                        error!(
                                            error = format!("{error:#}"),
                                            "Cannot get Account entity"
                                        )
                                    })
                                    .unwrap()
                            })
                        })
                        .clone()
                })
                .await
                .map_err(Error::SpawnEntity);

                if account_sdr.send(account).is_err() {
                    error!(%id, "Cannot send back spawn result");
                }
            }
        });

        Self { get_account_sdr }
    }
}

impl AccountFactory for LruCacheAccountFactory {
    type Error = Error;

    async fn get(&self, id: Uuid) -> Result<EntityRef<Account>, Self::Error> {
        let (account_srd, account_rcv) = oneshot::channel();
        self.get_account_sdr
            .send((id, account_srd))
            .await
            .map_err(Error::Send)?;
        account_rcv.await.map_err(Error::Rcv)?
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    cache_capacity: NonZeroUsize,
    cache_buffer: NonZeroUsize,
    entity_cmd_buffer: NonZeroUsize,
    entity_snapshot_after: Option<NonZeroU64>,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Cannot spawn entity")]
    SpawnEntity(JoinError),

    #[error("Cannot send spawn command to account entity factory")]
    Send(mpsc::error::SendError<(Uuid, oneshot::Sender<Result<EntityRef<Account>, Error>>)>),

    #[error("Cannot receive result from entity factory")]
    Rcv(oneshot::error::RecvError),
}
