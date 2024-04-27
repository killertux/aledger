use axum::{
    debug_handler,
    extract::{Path, State},
    Json,
};

use crate::{
    app::AppState, controller::JsonError, domain::gateway::GetBalanceError,
    gateway::ledger_entry_repository::DynamoDbLedgerEntryRepository,
};
use crate::domain::entity::AccountId;
use crate::domain::entity::EntryId;
use crate::domain::use_case::get_entry_use_case;

use super::LedgerResponse;

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
