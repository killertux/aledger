use crate::domain::entity::AccountId;
use crate::domain::entity::EntryWithBalance;
use crate::domain::gateway::{GetBalanceError, LedgerEntryRepository};

pub async fn get_balance_use_case(
    repository: &impl LedgerEntryRepository,
    account_id: &AccountId,
) -> Result<EntryWithBalance, GetBalanceError> {
    repository.get_balance(account_id).await
}
