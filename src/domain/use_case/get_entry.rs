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
        entry_to_continue: match last.status {
            EntryStatus::Applied => EntryToContinue::CurrentEntry,
            EntryStatus::Reverted(_) => EntryToContinue::Sequence(last.sequence),
            EntryStatus::Revert(_) => EntryToContinue::Sequence(last.sequence),
        },
    });
    Ok((entries, cursor))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        app::test::{get_repository, get_rng},
        domain::{
            entity::DeleteEntryRequest,
            use_case::{
                delete_entries_use_case, push_entries::test::push_multiple_entries,
                push_entries_use_case,
            },
        },
    };
    use anyhow::{bail, Result};
    use fake::{Fake, Faker};

    #[tokio_shared_rt::test(shared)]
    async fn get_entry_without_any_revert() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();
        let entries = push_multiple_entries(&repository, &account_id, 1).await;
        let result = get_entry_use_case(&repository, &account_id, &entries[0].entry_id, 10).await?;
        assert_eq!(entries, result.0);
        assert_eq!(None, result.1);
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn get_single_entry_with_cursor() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();
        let entries = push_multiple_entries(&repository, &account_id, 1).await;
        let (entry, Some(cursor)) =
            get_entry_use_case(&repository, &account_id, &entries[0].entry_id, 1).await?
        else {
            bail!("expected a cursor");
        };
        assert_eq!(entries, entry);
        assert_eq!(
            Cursor::FromEntryQuery {
                account_id: account_id.clone(),
                entry_id: entries[0].entry_id.clone(),
                entry_to_continue: EntryToContinue::CurrentEntry
            },
            cursor.clone()
        );
        let (entry, cursor) = get_entry_from_cursor_use_case(&repository, cursor, 1).await?;
        assert!(entry.is_empty());
        assert_eq!(None, cursor);
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn get_entry_with_one_revert() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();
        let mut entries = push_multiple_entries(&repository, &account_id, 1).await;
        let (revert_entries, non_applied) = delete_entries_use_case(
            &repository,
            get_rng().await,
            [DeleteEntryRequest {
                account_id: entries[0].account_id.clone(),
                entry_id: entries[0].entry_id.clone(),
            }]
            .into_iter(),
        )
        .await;
        assert!(non_applied.is_empty());
        let result = get_entry_use_case(&repository, &account_id, &entries[0].entry_id, 10).await?;
        entries[0].status = EntryStatus::Reverted(1);
        assert_eq!(
            vec![revert_entries[0].clone(), entries[0].clone()],
            result.0
        );
        assert_eq!(None, result.1);
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn get_entry_with_multiple_reverts() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();
        let mut entry_1 = push_multiple_entries(&repository, &account_id, 1)
            .await
            .remove(0);
        let revert_1 = revert_entry(&repository, &entry_1).await;
        let mut entry_2 = push_entries_use_case(
            &repository,
            get_rng().await,
            [entry_1.clone().into()].into_iter(),
        )
        .await
        .0
        .remove(0);
        let revert_2 = revert_entry(&repository, &entry_1).await;
        let entry_3 = push_entries_use_case(
            &repository,
            get_rng().await,
            [entry_1.clone().into()].into_iter(),
        )
        .await
        .0
        .remove(0);

        let result = get_entry_use_case(&repository, &account_id, &entry_1.entry_id, 10).await?;
        entry_1.status = EntryStatus::Reverted(1);
        entry_2.status = EntryStatus::Reverted(3);
        assert_eq!(
            vec![entry_3, revert_2, entry_2, revert_1, entry_1],
            result.0
        );
        assert_eq!(None, result.1);
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn get_entry_with_cursor() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();
        let mut entry_1 = push_multiple_entries(&repository, &account_id, 1)
            .await
            .remove(0);
        let entry_id = entry_1.entry_id.clone();
        let revert_1 = revert_entry(&repository, &entry_1).await;
        let mut entry_2 = push_entries_use_case(
            &repository,
            get_rng().await,
            [entry_1.clone().into()].into_iter(),
        )
        .await
        .0
        .remove(0);
        let revert_2 = revert_entry(&repository, &entry_1).await;
        entry_1.status = EntryStatus::Reverted(1);
        entry_2.status = EntryStatus::Reverted(3);
        let (entries, Some(cursor)) =
            get_entry_use_case(&repository, &account_id, &entry_id, 2).await?
        else {
            bail!("Expect a cursor");
        };
        assert_eq!(
            Cursor::FromEntryQuery {
                account_id: account_id.clone(),
                entry_id: entry_id.clone(),
                entry_to_continue: EntryToContinue::Sequence(2)
            },
            cursor.clone()
        );
        assert_eq!(vec![revert_2, entry_2], entries);
        let (entries, cursor) = get_entry_from_cursor_use_case(&repository, cursor, 3).await?;
        assert_eq!(vec![revert_1, entry_1], entries);
        assert_eq!(None, cursor);

        Ok(())
    }

    async fn revert_entry(
        repository: &impl LedgerEntryRepository,
        entry: &EntryWithBalance,
    ) -> EntryWithBalance {
        let (mut revert_entries, non_applied) = delete_entries_use_case(
            repository,
            get_rng().await,
            [DeleteEntryRequest {
                account_id: entry.account_id.clone(),
                entry_id: entry.entry_id.clone(),
            }]
            .into_iter(),
        )
        .await;
        assert!(non_applied.is_empty());
        revert_entries.remove(0)
    }
}
