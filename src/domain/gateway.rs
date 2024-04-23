use super::entity::{AccountId, Entry, EntryId, EntryWithBalance};
use thiserror::Error;

pub trait LedgerEntryRepository {
    async fn append_entries(
        &self,
        account_id: &AccountId,
        entries: &[Entry],
    ) -> Result<Vec<EntryWithBalance>, AppendEntriesError>;
}

#[derive(Debug, Error)]
pub enum AppendEntriesError {
    #[error("Optimistic lock error in updating HEAD of account `{0:?}`")]
    OptimisticLockError(AccountId),
    #[error("Entries `{1:?}` already exists in account `{0:?}`")]
    EntriesAlreadyExists(AccountId, Vec<EntryId>),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
