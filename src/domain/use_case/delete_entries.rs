use std::time::Duration;

use itertools::Itertools;
use rand::Rng;
use tokio::time::sleep;

use crate::domain::entity::{EntryId, EntryWithBalance};
use crate::domain::entity::DeleteEntryRequest;
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
