use std::time::Duration;

use itertools::Itertools;
use rand::Rng;
use tokio::time::sleep;

use crate::domain::entity::{Entry, EntryWithBalance};
use crate::domain::gateway::{AppendEntriesError, LedgerEntryRepository};
use crate::domain::use_case;
use crate::domain::use_case::NonAppliedReason;

pub async fn push_entries_use_case(
    repository: &impl LedgerEntryRepository,
    mut random_number_generator: impl Rng,
    entries: impl Iterator<Item = Entry> + Send + Sync,
) -> (Vec<EntryWithBalance>, Vec<(NonAppliedReason, Entry)>) {
    let entries_by_account_id = entries.into_group_map_by(|v| v.account_id.clone());
    let mut applied_entries_with_balance = Vec::new();
    let mut non_applied_entries = Vec::new();

    for (account_id, total_entries) in entries_by_account_id.into_iter() {
        for entries in total_entries.chunks(99) {
            let mut entries = Vec::from(entries);
            let mut tries = 0;
            loop {
                tries += 1;
                match repository.append_entries(&account_id, &entries).await {
                    Ok(applied) => {
                        applied_entries_with_balance.extend(applied);
                        break;
                    }
                    Err(AppendEntriesError::OptimisticLockError(_)) if tries != 5 => {
                        if tries == 1 {
                            continue;
                        }
                        sleep(Duration::from_millis(
                            random_number_generator.gen_range(10..100),
                        ))
                        .await;
                    }
                    Err(AppendEntriesError::EntriesAlreadyExists(_, duplicated_entries_ids)) => {
                        let duplicated_entries = use_case::extract_if(&mut entries, |entry| {
                            duplicated_entries_ids.contains(&entry.entry_id)
                        });
                        non_applied_entries.extend(
                            duplicated_entries
                                .into_iter()
                                .map(|entry| (NonAppliedReason::EntriesAlreadyExists, entry)),
                        );
                    }
                    Err(err) => {
                        non_applied_entries.extend(entries.into_iter().map(|entry| {
                            (NonAppliedReason::from_append_entries_error(&err), entry)
                        }));
                        break;
                    }
                }
            }
        }
    }

    (applied_entries_with_balance, non_applied_entries)
}
