use anyhow::bail;
use base64::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fmt::Display};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Hash, Clone)]
pub struct AccountId(Uuid);

impl AccountId {
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<AccountId> for String {
    fn from(value: AccountId) -> String {
        value.0.into()
    }
}

impl Display for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_string())
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Clone)]
pub enum Order {
    Asc,
    Desc,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Clone)]
pub struct Cursor {
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
    order: Order,
    account_id: AccountId,
}

impl Cursor {
    pub fn new(
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        order: Order,
        account_id: AccountId,
    ) -> Self {
        Cursor {
            start_date,
            end_date,
            order,
            account_id,
        }
    }
    pub fn start_date(&self) -> &DateTime<Utc> {
        &self.start_date
    }
    pub fn end_date(&self) -> &DateTime<Utc> {
        &self.end_date
    }
    pub fn order(&self) -> &Order {
        &self.order
    }
    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    pub fn encode(&self) -> anyhow::Result<String> {
        Ok(BASE64_STANDARD.encode(serde_json::to_string(&self)?))
    }

    pub fn decode(value: String) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(&BASE64_STANDARD.decode(value)?)?)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct DeleteEntryRequest {
    pub account_id: AccountId,
    pub entry_id: EntryId,
}
