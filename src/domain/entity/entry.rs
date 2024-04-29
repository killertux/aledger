use std::collections::HashMap;
use std::fmt::Display;

use anyhow::bail;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    Applied,
    RevertedBy(EntryId),
    Reverts(EntryId),
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
    pub created_at: DateTime<Utc>,
}
