use super::AccountIdsProjection;
use crate::domain::account;
use eventsourced::{convert, EvtLog, SeqNo};
use futures::{FutureExt, StreamExt};
use parking_lot::RwLock;
use std::{collections::HashSet, future::Future, sync::Arc};
use tokio::{pin, sync::oneshot, task};
use tracing::{debug, error};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct InMemAccountIdsProjection {
    account_ids: Arc<RwLock<HashSet<Uuid>>>,
}

impl InMemAccountIdsProjection {
    pub async fn new<L>(evt_log: L) -> (Self, impl Future<Output = ()>)
    where
        L: EvtLog,
    {
        let account_ids = Arc::new(RwLock::new(HashSet::default()));
        let (terminated_sdr, terminated_rcv) = oneshot::channel::<()>();

        // TODO: Proper error handling!
        let account_ids_clone = account_ids.clone();
        task::spawn(async move {
            let ids = evt_log
                .evts_by_tag::<account::Evt, _, _, _>(
                    account::ACCOUNT_LIFECYCLE_TAG,
                    SeqNo::MIN,
                    convert::serde_json::from_bytes,
                )
                .await
                .unwrap();
            pin!(ids);
            while let Some(Ok((_, account::Evt::Created(id)))) = ids.next().await {
                debug!(%id, "Inserting ID");
                account_ids_clone.write().insert(id);
            }

            error!("Account IDs projection terminated");
            let _ = terminated_sdr.send(());
        });

        (Self { account_ids }, terminated_rcv.map(|_| ()))
    }
}

impl AccountIdsProjection for InMemAccountIdsProjection {
    async fn contains(&self, id: Uuid) -> bool {
        self.account_ids.read().contains(&id)
    }
}
