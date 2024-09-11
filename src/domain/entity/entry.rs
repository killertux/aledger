use std::collections::HashMap;
use std::fmt::Display;

use anyhow::bail;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::entity::conditional::Conditional;
use crate::domain::entity::{AccountId, LedgerBalanceName, LedgerFieldName};

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Hash, Clone)]
#[serde(try_from = "String")]
pub struct EntryId(String);

impl Display for EntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl EntryId {
    pub fn new(entry_id: String) -> anyhow::Result<Self> {
        if entry_id.contains('|') {
            bail!("Entry id cannot contains the `|` char")
        }
        if entry_id.len() > 64 {
            bail!("Entry id cannot be longer the 64 characters")
        }
        Ok(Self(entry_id))
    }

    pub fn new_unchecked(entry_id: String) -> Self {
        Self(entry_id)
    }
}

impl TryFrom<String> for EntryId {
    type Error = anyhow::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        EntryId::new(value)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Entry {
    pub account_id: AccountId,
    pub entry_id: EntryId,
    pub ledger_fields: HashMap<LedgerFieldName, i128>,
    pub additional_fields: Value,
    pub status: EntryStatus,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct EntryWithConditionals {
    pub entry: Entry,
    pub conditionals: Vec<Conditional>,
}

impl From<Entry> for EntryWithConditionals {
    fn from(value: Entry) -> Self {
        Self {
            entry: value,
            conditionals: vec![],
        }
    }
}

impl From<EntryWithBalance> for EntryWithConditionals {
    fn from(value: EntryWithBalance) -> Self {
        Self {
            entry: value.into(),
            conditionals: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    Applied,
    Reverted(u64),
    Revert(u64),
}

impl From<EntryWithBalance> for Entry {
    fn from(value: EntryWithBalance) -> Self {
        Self {
            account_id: value.account_id,
            entry_id: value.entry_id,
            ledger_fields: value.ledger_fields,
            additional_fields: value.additional_fields,
            status: value.status,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct EntryWithBalance {
    pub account_id: AccountId,
    pub entry_id: EntryId,
    pub ledger_balances: HashMap<LedgerBalanceName, i128>,
    pub ledger_fields: HashMap<LedgerFieldName, i128>,
    pub additional_fields: Value,
    pub status: EntryStatus,
    pub sequence: u64,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
pub mod test {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use fake::{Fake, Faker};
    use serde_json::Value::Null;
    use uuid::Uuid;

    use crate::domain::entity::{
        AccountId, Entry, EntryId, EntryStatus, EntryWithBalance, LedgerBalanceName,
        LedgerFieldName,
    };
    use crate::utils::utc_now;

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

    thread_local! {
        pub static SEQUENCE_FAKE: RefCell<HashMap<AccountId, u64>> = RefCell::new(HashMap::new()) ;
    }

    pub struct EntryWithBalanceBuilder {
        entry: EntryWithBalance,
    }

    impl EntryWithBalanceBuilder {
        pub fn from_entry(entry: Entry) -> Self {
            let sequence = SEQUENCE_FAKE.with_borrow_mut(|v| {
                v.entry(entry.account_id.clone())
                    .and_modify(|sequence| *sequence += 1)
                    .or_insert(0)
                    .clone()
            });
            Self {
                entry: EntryWithBalance {
                    account_id: entry.account_id,
                    entry_id: entry.entry_id,
                    ledger_fields: entry.ledger_fields,
                    additional_fields: entry.additional_fields,
                    ledger_balances: HashMap::new(),
                    status: entry.status,
                    sequence,
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
