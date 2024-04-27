use crate::domain::entity::AccountId;
use crate::domain::entity::EntryId;
use crate::domain::entity::EntryWithBalance;
use crate::domain::gateway::{GetBalanceError, LedgerEntryRepository};

pub async fn get_entry_use_case(
    repository: &impl LedgerEntryRepository,
    account_id: &AccountId,
    entry_id: &EntryId,
) -> Result<Vec<EntryWithBalance>, GetBalanceError> {
    repository.get_entry(account_id, entry_id).await
}
