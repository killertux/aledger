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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonAppliedReason {
    OptimisticLockFailed,
    EntriesAlreadyExists,
    EntriesDoesNotExists,
    Other(String),
}

impl NonAppliedReason {
    pub fn from_append_entries_error(error: &AppendEntriesError) -> Self {
        tracing::warn!("Error appending entries: {error}");
        match error {
            AppendEntriesError::OptimisticLockError(_) => Self::OptimisticLockFailed,
            AppendEntriesError::EntriesAlreadyExists(_, _) => Self::EntriesAlreadyExists,
            AppendEntriesError::Other(err) => Self::Other(err.to_string()),
        }
    }

    pub fn from_revert_entries_error(error: &RevertEntriesError) -> Self {
        tracing::warn!("Error reverting entries: {error}");
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

#[cfg(test)]
pub mod test {
    use std::collections::HashMap;

    use aws_sdk_dynamodb::Client;
    use fake::{Fake, Faker};
    use lazy_static::lazy_static;
    use rand::SeedableRng;
    use rand::{rngs::SmallRng, Rng};
    use serde_json::Value::Null;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use crate::domain::entity::{
        AccountId, Entry, EntryId, EntryStatus, EntryWithBalance, LedgerBalanceName,
        LedgerFieldName,
    };
    use crate::utils::utc_now;
    use crate::{
        domain::gateway::LedgerEntryRepository,
        gateway::ledger_entry_repository::DynamoDbLedgerEntryRepository,
    };

    lazy_static! {
        static ref CLIENT: Mutex<Option<Client>> = Mutex::new(None);
        static ref RNG: Mutex<Option<SmallRng>> = Mutex::new(None);
    }

    pub async fn set_up_dynamo_db_for_test() -> Client {
        let mut client = CLIENT.lock().await;
        match client.as_ref() {
            Some(client) => client.clone(),
            None => {
                dotenv::dotenv().expect("Error loading env for test");
                let new_client = crate::dynamo_db_client().await;
                *client = Some(new_client.clone());
                new_client
            }
        }
    }

    pub async fn get_rng() -> impl Rng {
        let mut rng = RNG.lock().await;
        match rng.as_ref() {
            Some(rng) => rng.clone(),
            None => {
                let new_rng = SmallRng::from_entropy();
                *rng = Some(new_rng.clone());
                new_rng
            }
        }
    }

    pub async fn get_repository() -> impl LedgerEntryRepository {
        DynamoDbLedgerEntryRepository::from(set_up_dynamo_db_for_test().await)
    }

    pub struct EntryBuilder {
        entry: Entry,
    }

    impl EntryBuilder {
        pub fn new() -> Self {
            Self {
                entry: Entry {
                    account_id: AccountId::new(Faker.fake()),
                    entry_id: EntryId::new_unchecked(Faker.fake::<Uuid>().to_string()),
                    ledger_fields: HashMap::new(),
                    additional_fields: Null,
                    status: EntryStatus::Applied,
                },
            }
        }

        pub fn with_account_id(mut self, account_id: AccountId) -> Self {
            self.entry.account_id = account_id;
            self
        }

        pub fn with_ledger_field(mut self, key: impl Into<String>, value: i128) -> Self {
            self.entry.ledger_fields.insert(
                LedgerFieldName::new(key.into()).expect("Error with ledger field name"),
                value,
            );
            self
        }

        pub fn build(self) -> Entry {
            self.entry
        }
    }

    pub struct EntryWithBalanceBuilder {
        entry: EntryWithBalance,
    }

    impl EntryWithBalanceBuilder {
        pub fn from_entry(entry: Entry) -> Self {
            Self {
                entry: EntryWithBalance {
                    account_id: entry.account_id,
                    entry_id: entry.entry_id,
                    ledger_fields: entry.ledger_fields,
                    additional_fields: entry.additional_fields,
                    ledger_balances: HashMap::new(),
                    status: entry.status,
                    created_at: utc_now(),
                },
            }
        }

        pub fn with_ledger_balance(mut self, key: impl Into<String>, value: i128) -> Self {
            self.entry.ledger_balances.insert(
                LedgerBalanceName::new(key.into()).expect("Error with ledger field name"),
                value,
            );
            self
        }

        pub fn build(self) -> EntryWithBalance {
            self.entry
        }
    }
}
