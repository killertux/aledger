use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::entity::{AccountId, Order};

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
