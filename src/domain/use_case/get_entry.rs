use anyhow::anyhow;

use crate::domain::entity::AccountId;
use crate::domain::entity::Cursor;
use crate::domain::entity::EntryId;
use crate::domain::entity::EntryStatus;
use crate::domain::entity::EntryToContinue;
use crate::domain::entity::EntryWithBalance;
use crate::domain::gateway::{GetBalanceError, LedgerEntryRepository};

pub async fn get_entry_use_case(
    repository: &impl LedgerEntryRepository,
    account_id: &AccountId,
    entry_id: &EntryId,
    limit: u8,
) -> Result<(Vec<EntryWithBalance>, Option<Cursor>), GetBalanceError> {
    get_entry(
        repository,
        account_id,
        entry_id,
        EntryToContinue::Start,
        limit,
    )
    .await
}

pub async fn get_entry_from_cursor_use_case(
    repository: &impl LedgerEntryRepository,
    cursor: Cursor,
    limit: u8,
) -> Result<(Vec<EntryWithBalance>, Option<Cursor>), GetBalanceError> {
    let Cursor::FromEntryQuery {
        account_id,
        entry_id,
        entry_to_continue,
    } = cursor
    else {
        return Err(anyhow!("Invalid cursor").into());
    };
    get_entry(repository, &account_id, &entry_id, entry_to_continue, limit).await
}

async fn get_entry(
    repository: &impl LedgerEntryRepository,
    account_id: &AccountId,
    entry_id: &EntryId,
    entry_to_continue: EntryToContinue,
    limit: u8,
) -> Result<(Vec<EntryWithBalance>, Option<Cursor>), GetBalanceError> {
    let entries = repository
        .get_entry(account_id, entry_id, entry_to_continue, limit)
        .await?;
    if entries.len() < limit as usize {
        return Ok((entries, None));
    }
    let cursor = entries.last().map(|last| Cursor::FromEntryQuery {
        account_id: account_id.clone(),
        entry_id: entry_id.clone(),
        entry_to_continue: match &last.status {
            EntryStatus::Applied => EntryToContinue::CurrentEntry,
            EntryStatus::Reverted(sequence) => EntryToContinue::RevertedBy(*sequence),
            EntryStatus::Revert(_) => EntryToContinue::Start,
        },
    });
    Ok((entries, cursor))
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use fake::{Fake, Faker};

    #[tokio_shared_rt::test(shared)]
    async fn get_entry_withou_any_revert() -> Result<()> {
        Ok(())
    }
}
