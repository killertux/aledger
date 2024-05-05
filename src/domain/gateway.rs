use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::domain::entity::{Entry, EntryId, EntryWithBalance};
use crate::domain::entity::AccountId;
use crate::domain::entity::Cursor;

use super::entity::EntryToContinue;
use super::entity::Order;

pub trait LedgerEntryRepository {
    async fn append_entries(
        &self,
        account_id: &AccountId,
        entries: &[Entry],
    ) -> Result<Vec<EntryWithBalance>, AppendEntriesError>;

    async fn revert_entries(
        &self,
        account_id: &AccountId,
        entries: &[EntryId],
    ) -> Result<Vec<EntryWithBalance>, RevertEntriesError>;

    async fn get_balance(
        &self,
        account_id: &AccountId,
    ) -> Result<EntryWithBalance, GetBalanceError>;

    async fn get_entry(
        &self,
        account_id: &AccountId,
        entry_id: &EntryId,
        entry_to_continue: EntryToContinue,
        limit: u8,
    ) -> Result<Vec<EntryWithBalance>, GetBalanceError>;

    async fn get_entries(
        &self,
        account_id: &AccountId,
        start_date: &DateTime<Utc>,
        end_date: &DateTime<Utc>,
        limit: u8,
        order: &Order,
        sequence: Option<u64>,
    ) -> Result<(Vec<EntryWithBalance>, Option<Cursor>), GetBalanceError>;
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

#[derive(Debug, Error)]
pub enum RevertEntriesError {
    #[error("Optimistic lock error in updating HEAD of account `{0:?}`")]
    OptimisticLockError(AccountId),
    #[error("Entries `{1:?}` does not exists in account `{0:?}`")]
    EntriesDoesNotExists(AccountId, Vec<EntryId>),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<AppendEntriesError> for RevertEntriesError {
    fn from(value: AppendEntriesError) -> Self {
        match value {
            AppendEntriesError::OptimisticLockError(account_id) => {
                Self::OptimisticLockError(account_id)
            }
            err => Self::Other(err.into()),
        }
    }
}

#[derive(Debug, Error)]
pub enum GetBalanceError {
    #[error("Account not found with id `{0}`")]
    NotFound(AccountId),
    #[error("Missing field `{0}`")]
    MissingField(String),
    #[error("Error reading field `{0}`")]
    ErrorReadingField(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
