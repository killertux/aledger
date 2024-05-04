use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::entity::{AccountId, Order};

use super::EntryId;

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Clone)]
pub enum Cursor {
    FromEntriesQuery {
        account_id: AccountId,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        sequence: u128,
        order: Order,
    },
    FromEntryQuery {
        account_id: AccountId,
        entry_id: EntryId,
        entry_to_continue: EntryToContinue,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Clone)]
pub enum EntryToContinue {
    Start,
    CurrentEntry,
    RevertedBy(EntryId),
}

impl Cursor {
    pub fn encode(&self) -> anyhow::Result<String> {
        Ok(BASE64_STANDARD.encode(serde_json::to_string(&self)?))
    }

    pub fn decode(value: String) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(&BASE64_STANDARD.decode(value)?)?)
    }

    pub fn account_id(&self) -> &AccountId {
        match self {
            Self::FromEntriesQuery {
                start_date: _,
                end_date: _,
                order: _,
                sequence: _,
                account_id,
            } => account_id,
            Self::FromEntryQuery {
                account_id,
                entry_id: _,
                entry_to_continue: _,
            } => account_id,
        }
    }
}
