use std::collections::HashMap;

use axum::{debug_handler, extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{app::AppState, gateway::ledger_entry_repository::DynamoDbLedgerEntryRepository};
use crate::domain::entity::{Entry, EntryId, EntryStatus};
use crate::domain::entity::AccountId;
use crate::domain::entity::LedgerFieldName;
use crate::domain::use_case::push_entries_use_case;

use super::LedgerResponse;

#[debug_handler]
pub async fn push_entries(
    State(app_state): State<AppState>,
    Json(push_entries): Json<Vec<PushEntryRequest>>,
) -> Json<PushEntryResponse> {
    let (applied, non_applied) = push_entries_use_case(
        &DynamoDbLedgerEntryRepository::from(app_state.dynamo_client),
        app_state.random_number_generator,
        push_entries.into_iter().map(|entry| entry.into()),
    )
    .await;
    let response = PushEntryResponse {
        applied_entries: applied.into_iter().map(|v| v.into()).collect(),
        non_applied_entries: non_applied
            .into_iter()
            .map(|(reason, entry)| NonAppliedEntry {
                error: reason.message(),
                error_code: reason.reason_code(),
                entry: entry.into(),
            })
            .collect(),
    };
    Json(response)
}

#[derive(Serialize, Deserialize)]
pub struct PushEntryRequest {
    account_id: AccountId,
    entry_id: EntryId,
    ledger_fields: HashMap<LedgerFieldName, i128>,
    additional_fields: Value,
}

#[derive(Serialize)]
pub struct PushEntryResponse {
    applied_entries: Vec<LedgerResponse>,
    non_applied_entries: Vec<NonAppliedEntry>,
}
#[derive(Serialize)]
struct NonAppliedEntry {
    error: String,
    error_code: u16,
    entry: PushEntryRequest,
}

impl From<PushEntryRequest> for Entry {
    fn from(value: PushEntryRequest) -> Self {
        Self {
            account_id: value.account_id,
            entry_id: value.entry_id,
            ledger_fields: value.ledger_fields,
            additional_fields: value.additional_fields,
            status: EntryStatus::Applied,
        }
    }
}

impl From<Entry> for PushEntryRequest {
    fn from(value: Entry) -> Self {
        Self {
            account_id: value.account_id,
            entry_id: value.entry_id,
            ledger_fields: value.ledger_fields,
            additional_fields: value.additional_fields,
        }
    }
}
