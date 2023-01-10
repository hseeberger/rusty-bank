pub mod in_mem_ids_projection;
pub mod lru_cache_factory;

use crate::domain::account::Account;
use eventsourced::EntityRef;
use std::{error::Error as StdError, future::Future};
use uuid::Uuid;

/// A factory for [Account]s, either creating new ones or returning existing managed ones.
pub trait AccountFactory: Clone + Send + Sync + 'static {
    type Error: StdError + Send + Sync + 'static;

    /// Create a new [Account] or return an existing managed one.
    fn get(
        &self,
        id: Uuid,
    ) -> impl Future<Output = Result<EntityRef<Account>, Self::Error>> + Send + '_;
}

pub trait AccountIdsProjection: Clone + Send + Sync + 'static {
    /// Is the given ID in the set of all account IDs?
    fn contains(&self, id: Uuid) -> impl Future<Output = bool> + Send + '_;
}
