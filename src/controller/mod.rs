use axum::{http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{borrow::Cow, collections::HashMap};

use crate::domain::entity::{
    AccountId, EntryId, EntryWithBalance, LedgerBalanceName, LedgerFieldName,
};

pub mod push_entries;

#[derive(Serialize, Deserialize)]
pub struct LedgerResponse {
    account_id: AccountId,
    entry_id: EntryId,
    ledger_balances: HashMap<LedgerBalanceName, i128>,
    ledger_fields: HashMap<LedgerFieldName, i128>,
    additional_fields: Value,
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
            created_at: value.created_at,
        }
    }
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

impl<'a, E> From<E> for JsonError<'a>
where
    E: std::error::Error,
{
    fn from(_: E) -> JsonError<'a> {
        JsonError::internal_server_error()
    }
}

pub trait MapErrToInternalServerError<'a, T> {
    fn map_err_to_internal_server_error(self) -> Result<T, JsonError<'a>>;
}

impl<'a, T> MapErrToInternalServerError<'a, T> for Result<T, anyhow::Error> {
    fn map_err_to_internal_server_error(self) -> Result<T, JsonError<'a>> {
        self.map_err(|error| {
            tracing::error!("Fatal error: {}", error);
            JsonError::internal_server_error()
        })
    }
}
