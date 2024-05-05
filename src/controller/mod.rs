use std::{borrow::Cow, collections::HashMap};

use axum::{http::StatusCode, Json, response::IntoResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::entity::{EntryId, EntryStatus, EntryWithBalance};
use crate::domain::entity::AccountId;
use crate::domain::entity::LedgerBalanceName;
use crate::domain::entity::LedgerFieldName;

pub mod delete_entries;
pub mod get_balance;
pub mod get_entries;
pub mod get_entry;
pub mod push_entries;

#[derive(Serialize, Deserialize)]
pub struct LedgerResponse {
    account_id: AccountId,
    entry_id: EntryId,
    ledger_balances: HashMap<LedgerBalanceName, i128>,
    ledger_fields: HashMap<LedgerFieldName, i128>,
    additional_fields: Value,
    status: EntryStatus,
    created_at: DateTime<Utc>,
}

impl From<EntryWithBalance> for LedgerResponse {
    fn from(value: EntryWithBalance) -> Self {
        LedgerResponse {
            account_id: value.account_id,
            entry_id: value.entry_id,
            ledger_balances: value.ledger_balances,
            ledger_fields: value.ledger_fields,
            additional_fields: value.additional_fields,
            status: value.status,
            created_at: value.created_at,
        }
    }
}

#[derive(Serialize)]
pub struct GetEntriesLedgerResponse {
    entries: Vec<LedgerResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
}

pub struct JsonError<'a> {
    status_code: StatusCode,
    message: Error<'a>,
}

impl<'a> JsonError<'a> {
    pub fn new(status_code: StatusCode, message: Cow<'a, str>) -> Self {
        Self {
            status_code,
            message: Error { error: message },
        }
    }

    pub fn not_found(message: Cow<'a, str>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    pub fn unprocessable_entity(message: Cow<'a, str>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, message)
    }

    pub fn internal_server_error() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".into(),
        )
    }
}

#[derive(Serialize)]
struct Error<'a> {
    error: Cow<'a, str>,
}

impl<'a> IntoResponse for JsonError<'a> {
    fn into_response(self) -> axum::response::Response {
        (self.status_code, Json(self.message)).into_response()
    }
}

impl<'a> From<anyhow::Error> for JsonError<'a> {
    fn from(e: anyhow::Error) -> JsonError<'a> {
        tracing::error!("Internal server error {}", e);
        JsonError::internal_server_error()
    }
}
