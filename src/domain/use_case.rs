use super::{
    entity::{AccountId, Cursor, DeleteEntryRequest, Entry, EntryId, EntryWithBalance, Order},
    gateway::{AppendEntriesError, GetBalanceError, LedgerEntryRepository, RevertEntriesError},
};
use chrono::{DateTime, Utc};
use itertools::Itertools;
use rand::Rng;
use std::time::Duration;
use tokio::time::sleep;

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
                        let entries_not_found = extract_if(&mut entries_to_delete, |entry| {
                            entries_non_existent_ids.contains(&entry.entry_id)
                        });
                        let _ = extract_if(&mut entries_ids, |entry_id| {
                            entries_non_existent_ids.contains(&entry_id)
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

pub async fn get_balance_use_case(
    repository: &impl LedgerEntryRepository,
    account_id: &AccountId,
) -> Result<EntryWithBalance, GetBalanceError> {
    repository.get_balance(account_id).await
}

pub async fn get_entry_use_case(
    repository: &impl LedgerEntryRepository,
    account_id: &AccountId,
    entry_id: &EntryId,
) -> Result<Vec<EntryWithBalance>, GetBalanceError> {
    repository.get_entry(account_id, entry_id).await
}

pub async fn get_entries_use_case(
    repository: &impl LedgerEntryRepository,
    account_id: &AccountId,
    start_date: &DateTime<Utc>,
    end_date: &DateTime<Utc>,
    limit: u8,
    order: &Order,
) -> Result<(Vec<EntryWithBalance>, Option<Cursor>), GetBalanceError> {
    repository
        .get_entries(account_id, start_date, end_date, limit, order)
        .await
}

pub async fn get_entries_from_cursor_use_case(
    repository: &impl LedgerEntryRepository,
    cursor: Cursor,
    limit: u8,
) -> Result<(Vec<EntryWithBalance>, Option<Cursor>), GetBalanceError> {
    repository
        .get_entries(
            cursor.account_id(),
            cursor.start_date(),
            cursor.end_date(),
            limit,
            cursor.order(),
        )
        .await
}

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
                        let duplicated_entries = extract_if(&mut entries, |entry| {
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

fn extract_if<T, F>(vector: &mut Vec<T>, predicate: F) -> Vec<T>
where
    F: Fn(&T) -> bool,
{
    let mut result = Vec::new();
    let mut i = 0;
    while i < vector.len() {
        if predicate(&vector[i]) {
            result.push(vector.remove(i));
        } else {
            i += 1;
        }
    }
    result
}

pub enum NonAppliedReason {
    OptimisticLockFailed,
    EntriesAlreadyExists,
    EntriesDoesNotExists,
    Other(String),
}

impl NonAppliedReason {
    pub fn from_append_entries_error(error: &AppendEntriesError) -> Self {
        match error {
            AppendEntriesError::OptimisticLockError(_) => Self::OptimisticLockFailed,
            AppendEntriesError::EntriesAlreadyExists(_, _) => Self::EntriesAlreadyExists,
            AppendEntriesError::Other(err) => Self::Other(err.to_string()),
        }
    }

    pub fn from_revert_entries_error(error: &RevertEntriesError) -> Self {
        match error {
            RevertEntriesError::OptimisticLockError(_) => Self::OptimisticLockFailed,
            RevertEntriesError::EntriesDoesNotExists(_, _) => Self::EntriesAlreadyExists,
            RevertEntriesError::Other(err) => Self::Other(err.to_string()),
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::OptimisticLockFailed => "Optimistic lock failed. Try again later".into(),
            Self::EntriesAlreadyExists => "Entry already exists for this account".into(),
            Self::EntriesDoesNotExists => {
                "Entry does not exists or reverted for this account".into()
            }
            Self::Other(err) => format!("Other unexpected error: {err}"),
        }
    }

    pub fn reason_code(&self) -> u16 {
        match self {
            Self::OptimisticLockFailed => 100,
            Self::EntriesAlreadyExists => 200,
            Self::EntriesDoesNotExists => 300,
            Self::Other(_) => 900,
        }
    }
}
