use std::collections::HashMap;
use anyhow::bail;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq)]
struct AccountId(Uuid);

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Hash)]
#[serde(try_from = "String")]
struct LedgerFieldName(String);

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

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Hash)]
#[serde(try_from = "String")]
struct EntryId(String);

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

#[derive(Serialize, Deserialize)]
struct PushEntryRequest {
    account_id: AccountId,
    entry_id: EntryId,
    ledger_fields: HashMap<LedgerFieldName, i128>,
    additional_fields: Value
}

#[derive(Serialize, Deserialize)]
struct LedgerResponse {
    account_id: AccountId,
    entry_id: EntryId,
    ledger_balances: HashMap<String, i128>,
    ledger_fields: HashMap<LedgerFieldName, i128>,
    additional_fields: Value,
    created_at: DateTime<Utc>,
}
