use axum::{
    debug_handler,
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::domain::entity::{Cursor, EntryId};
use crate::domain::use_case::{get_entry_from_cursor_use_case, get_entry_use_case};
use crate::{
    app::AppState, controller::JsonError, domain::gateway::GetBalanceError,
    gateway::ledger_entry_repository::DynamoDbLedgerEntryRepository,
};
use crate::{controller::GetEntriesLedgerResponse, domain::entity::AccountId};

#[debug_handler]
pub async fn get_entry(
    State(app_state): State<AppState>,
    Path((account_id, entry_id)): Path<(AccountId, EntryId)>,
    Query(params): Query<GetEntryParams>,
) -> Result<Json<GetEntriesLedgerResponse>, JsonError<'static>> {
    let repository = DynamoDbLedgerEntryRepository::from(app_state.dynamo_client);
    let limit = params.limit.unwrap_or(100);
    let result = match params.cursor {
        Some(cursor) => {
            get_entry_from_cursor_use_case(&repository, Cursor::decode(cursor)?, limit).await
        }
        None => get_entry_use_case(&repository, &account_id, &entry_id, limit).await,
    };
    match result {
        Ok((entries, cursor)) => Ok(Json(GetEntriesLedgerResponse {
            entries: entries.into_iter().map(|entry| entry.into()).collect(),
            cursor: cursor.map(|c| c.encode()).transpose()?,
        })),
        Err(GetBalanceError::NotFound(_)) => Err(JsonError::not_found(
            format!("Entry {} not found", entry_id.to_string()).into(),
        )),
        Err(e) => Err(anyhow::Error::from(e).into()),
    }
}

#[derive(Deserialize)]
pub struct GetEntryParams {
    limit: Option<u8>,
    cursor: Option<String>,
}
