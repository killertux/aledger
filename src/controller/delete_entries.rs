use axum::{extract::State, Json};
use serde::Serialize;

use crate::domain::use_case::delete_entries_use_case;
use crate::{
    app::AppState, domain::entity::DeleteEntryRequest,
    gateway::ledger_entry_repository::DynamoDbLedgerEntryRepository,
};

use super::LedgerResponse;

pub async fn delete_entries(
    State(app_state): State<AppState>,
    Json(delete_entries): Json<Vec<DeleteEntryRequest>>,
) -> Json<DeleteEntryResponse> {
    let (applied, non_applied) = delete_entries_use_case(
        &DynamoDbLedgerEntryRepository::from(app_state.dynamo_client),
        app_state.random_number_generator,
        delete_entries.into_iter(),
    )
    .await;
    let response = DeleteEntryResponse {
        applied_entries: applied.into_iter().map(|v| v.into()).collect(),
        non_applied_entries: non_applied
            .into_iter()
            .map(|(reason, delete_entry_request)| NonAppliedDeleteEntry {
                error: reason.message(),
                error_code: reason.reason_code(),
                delete_entry_request,
            })
            .collect(),
    };
    Json(response)
}

#[derive(Serialize)]
pub struct DeleteEntryResponse {
    applied_entries: Vec<LedgerResponse>,
    non_applied_entries: Vec<NonAppliedDeleteEntry>,
}

#[derive(Serialize)]
struct NonAppliedDeleteEntry {
    error: String,
    error_code: u16,
    delete_entry_request: DeleteEntryRequest,
}
