use std::time::Duration;

use itertools::Itertools;
use rand::Rng;
use tokio::time::sleep;

use crate::domain::entity::DeleteEntryRequest;
use crate::domain::entity::{EntryId, EntryWithBalance};
use crate::domain::gateway::{LedgerEntryRepository, RevertEntriesError};
use crate::domain::use_case;
use crate::domain::use_case::NonAppliedReason;

pub async fn delete_entries_use_case(
    repository: &impl LedgerEntryRepository,
    mut random_number_generator: impl Rng,
    entries_to_delete: impl Iterator<Item = DeleteEntryRequest> + Send + Sync,
) -> (
    Vec<EntryWithBalance>,
    Vec<(NonAppliedReason, DeleteEntryRequest)>,
) {
    let entries_by_account_id = entries_to_delete.into_group_map_by(|v| v.account_id.clone());
    let mut applied_entries_with_balance = Vec::new();
    let mut non_applied_entries = Vec::new();

    for (account_id, total_entries) in entries_by_account_id.into_iter() {
        for entries_to_delete in total_entries.chunks(33) {
            let mut entries_to_delete = Vec::from(entries_to_delete);
            let mut entries_ids = entries_to_delete
                .iter()
                .map(|entry_to_delete| entry_to_delete.entry_id.clone())
                .collect::<Vec<EntryId>>();
            let mut tries = 0;
            loop {
                tries += 1;
                match repository.revert_entries(&account_id, &entries_ids).await {
                    Ok(applied) => {
                        applied_entries_with_balance.extend(applied);
                        break;
                    }
                    Err(RevertEntriesError::OptimisticLockError(_)) if tries != 5 => {
                        if tries == 1 {
                            continue;
                        }
                        sleep(Duration::from_millis(
                            random_number_generator.gen_range(10..100),
                        ))
                        .await;
                    }
                    Err(RevertEntriesError::EntriesDoesNotExists(_, entries_non_existent_ids)) => {
                        let entries_not_found =
                            use_case::extract_if(&mut entries_to_delete, |entry| {
                                entries_non_existent_ids.contains(&entry.entry_id)
                            });
                        let _ = use_case::extract_if(&mut entries_ids, |entry_id| {
                            entries_non_existent_ids.contains(entry_id)
                        });
                        non_applied_entries.extend(
                            entries_not_found
                                .into_iter()
                                .map(|entry| (NonAppliedReason::EntriesDoesNotExists, entry)),
                        );
                    }
                    Err(err) => {
                        non_applied_entries.extend(entries_to_delete.into_iter().map(|entry| {
                            (NonAppliedReason::from_revert_entries_error(&err), entry)
                        }));
                        break;
                    }
                }
            }
        }
    }

    (applied_entries_with_balance, non_applied_entries)
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use fake::{Fake, Faker};

    use crate::app::test::{get_repository, get_rng};
    use crate::domain::entity::{LedgerBalanceName, Order};
    use crate::domain::{
        entity::{DeleteEntryRequest, EntryStatus},
        use_case::{
            get_entries_use_case, push_entries::test::push_multiple_entries, push_entries_use_case,
        },
    };
    use crate::utils::utc_now;

    use super::*;

    #[tokio_shared_rt::test(shared)]
    async fn delete_entry_that_does_not_exist() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();

        let entries = push_multiple_entries(&repository, &account_id, 1).await;
        let (applied, non_applied) = delete_entries_use_case(
            &repository,
            get_rng().await,
            vec![
                DeleteEntryRequest {
                    account_id: account_id.clone(),
                    entry_id: EntryId::new("invalid".into())?,
                },
                DeleteEntryRequest {
                    account_id: account_id.clone(),
                    entry_id: entries[0].entry_id.clone(),
                },
            ]
            .into_iter(),
        )
        .await;

        assert_eq!(1, applied.len());
        assert_eq!(EntryStatus::Revert(entries[0].sequence), applied[0].status);
        assert_eq!(
            0,
            *applied[0]
                .ledger_balances
                .get(&LedgerBalanceName::new("balance_amount".into())?)
                .expect("We know that there is this field")
        );
        assert_eq!(
            vec![(
                NonAppliedReason::EntriesDoesNotExists,
                DeleteEntryRequest {
                    account_id: account_id.clone(),
                    entry_id: EntryId::new("invalid".into())?,
                }
            )],
            non_applied
        );

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn delete_entries() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();

        let entries = push_multiple_entries(&repository, &account_id, 2).await;
        let (applied, non_applied) = delete_entries_use_case(
            &repository,
            get_rng().await,
            vec![
                DeleteEntryRequest {
                    account_id: account_id.clone(),
                    entry_id: entries[0].entry_id.clone(),
                },
                DeleteEntryRequest {
                    account_id: account_id.clone(),
                    entry_id: entries[1].entry_id.clone(),
                },
            ]
            .into_iter(),
        )
        .await;
        assert!(non_applied.is_empty());
        assert_eq!(EntryStatus::Revert(entries[0].sequence), applied[0].status);
        assert_eq!(EntryStatus::Revert(entries[1].sequence), applied[1].status);

        let entries = get_entries_use_case(
            &repository,
            &account_id,
            &utc_now(),
            &utc_now(),
            10,
            &Order::Desc,
        )
        .await?
        .0;
        assert_eq!(4, entries.len());
        assert_eq!(
            EntryStatus::Reverted(entries[0].sequence),
            entries[2].status
        );
        assert_eq!(
            EntryStatus::Reverted(entries[1].sequence),
            entries[3].status
        );
        assert_eq!(
            0,
            *entries[0]
                .ledger_balances
                .get(&LedgerBalanceName::new("balance_amount".into())?)
                .expect("We know that there is this field")
        );

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn delete_entries_should_allow_to_re_add() -> Result<()> {
        let repository = get_repository().await;
        let account_id = Faker.fake();

        let entries = push_multiple_entries(&repository, &account_id, 1).await;
        let (_, non_applied) = delete_entries_use_case(
            &repository,
            get_rng().await,
            vec![DeleteEntryRequest {
                account_id: account_id.clone(),
                entry_id: entries[0].entry_id.clone(),
            }]
            .into_iter(),
        )
        .await;
        assert!(non_applied.is_empty());
        let (mut applied, non_applied) = push_entries_use_case(
            &repository,
            get_rng().await,
            [entries[0].clone().into()].into_iter(),
        )
        .await;
        assert!(non_applied.is_empty());
        assert_ne!(entries[0].sequence, applied[0].sequence);
        applied[0].sequence = entries[0].sequence;
        assert_eq!(entries, applied);
        Ok(())
    }
}
