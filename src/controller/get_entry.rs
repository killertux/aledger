use super::LedgerResponse;
use crate::{
    controller::JsonError,
    domain::{
        entity::{AccountId, EntryId},
        gateway::GetBalanceError,
        use_case::get_entry_use_case,
    },
    gateway::ledger_entry_repository::DynamoDbLedgerEntryRepository,
    AppState,
};
use axum::{
    debug_handler,
    extract::{Path, State},
    Json,
};

#[debug_handler]
pub async fn get_entry(
    State(app_state): State<AppState>,
    Path((account_id, entry_id)): Path<(AccountId, EntryId)>,
) -> Result<Json<Vec<LedgerResponse>>, JsonError<'static>> {
    match get_entry_use_case(
        &DynamoDbLedgerEntryRepository::from(app_state.dynamo_client),
        &account_id,
        &entry_id,
    )
    .await
    {
        Ok(entries) => Ok(Json(
            entries.into_iter().map(|entry| entry.into()).collect(),
        )),
        Err(GetBalanceError::NotFound(_)) => Err(JsonError::not_found(
            format!("Entry {} not found", entry_id.to_string()).into(),
        )),
        Err(e) => Err(anyhow::Error::from(e).into()),
    }
}
