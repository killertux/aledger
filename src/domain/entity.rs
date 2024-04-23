use anyhow::bail;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Hash, Clone)]
pub struct AccountId(Uuid);

impl From<AccountId> for String {
    fn from(value: AccountId) -> String {
        value.0.into()
    }
}

impl ToString for AccountId {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Hash, Clone)]
#[serde(try_from = "String")]
pub struct LedgerFieldName(String);

impl From<LedgerFieldName> for String {
    fn from(value: LedgerFieldName) -> String {
        value.0
    }
}

impl LedgerFieldName {
    pub fn new(field_name: String) -> anyhow::Result<Self> {
        if field_name.starts_with("balance_") {
            bail!("Ledge field cannot start with `balance_`")
        }
        Ok(Self(field_name))
    }
}

impl TryFrom<String> for LedgerFieldName {
    type Error = anyhow::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        LedgerFieldName::new(value)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Hash, Clone)]
pub struct LedgerBalanceName(String);

impl LedgerBalanceName {
    pub fn new(value: String) -> anyhow::Result<Self> {
        if !value.starts_with("balance_") {
            bail!("Ledger balance name must start with balance_");
        }
        Ok(Self(value))
    }
}

impl From<LedgerBalanceName> for String {
    fn from(value: LedgerBalanceName) -> String {
        value.0
    }
}

impl From<LedgerFieldName> for LedgerBalanceName {
    fn from(value: LedgerFieldName) -> Self {
        Self(format!("balance_{}", value.0))
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Hash, Clone)]
#[serde(try_from = "String")]
pub struct EntryId(String);

impl ToString for EntryId {
    fn to_string(&self) -> String {
        self.0.to_string()
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
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct EntryWithBalance {
    pub account_id: AccountId,
    pub entry_id: EntryId,
    pub ledger_balances: HashMap<LedgerBalanceName, i128>,
    pub ledger_fields: HashMap<LedgerFieldName, i128>,
    pub additional_fields: Value,
    pub created_at: DateTime<Utc>,
}
