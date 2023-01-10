use super::AccountIdsProjection;
use crate::domain::account;
use eventsourced::{convert, EvtLog, SeqNo};
use futures::StreamExt;
use parking_lot::RwLock;
use std::{collections::HashSet, sync::Arc};
use tokio::{pin, task};
use tracing::debug;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct InMemAccountIdsProjection {
    account_ids: Arc<RwLock<HashSet<Uuid>>>,
}

impl InMemAccountIdsProjection {
    pub async fn new<L>(evt_log: L) -> Self
    where
        L: EvtLog,
    {
        let account_ids = Arc::new(RwLock::new(HashSet::default()));

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
        });

        // TODO: Handle stream termination!

        Self { account_ids }
    }
}

impl AccountIdsProjection for InMemAccountIdsProjection {
    async fn contains(&self, id: Uuid) -> bool {
        self.account_ids.read().contains(&id)
    }
}
