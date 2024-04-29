pub use delete_entries::delete_entries_use_case;
pub use get_balance::get_balance_use_case;
pub use get_entries::{get_entries_from_cursor_use_case, get_entries_use_case};
pub use get_entry::{get_entry_from_cursor_use_case, get_entry_use_case};
pub use push_entries::push_entries_use_case;

use super::gateway::{AppendEntriesError, RevertEntriesError};

mod delete_entries;
mod get_balance;
mod get_entries;
mod get_entry;
mod push_entries;

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
