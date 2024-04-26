use super::LedgerResponse;
use crate::{
    controller::JsonError,
    domain::{
        entity::{AccountId, Cursor, Order},
        gateway::GetBalanceError,
        use_case::{get_entries_from_cursor_use_case, get_entries_use_case},
    },
    gateway::ledger_entry_repository::DynamoDbLedgerEntryRepository,
    AppState,
};
use axum::{
    debug_handler,
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[debug_handler]
pub async fn get_entries(
    State(app_state): State<AppState>,
    Path(account_id): Path<AccountId>,
    Query(query_params): Query<GetEntriesParams>,
) -> Result<Json<GetEntriesLedgerResponse>, JsonError<'static>> {
    if query_params.limit > 100 {
        return Err(JsonError::unprocessable_entity(
            "Limit must be lower or equal to 100".into(),
        ));
    }
    let result = match (
        query_params.cursor,
        query_params.start_date,
        query_params.end_date,
        query_params.order,
    ) {
        (Some(cursor), None, None, None) => {
            let cursor = Cursor::decode(cursor)?;
            if *cursor.account_id() != account_id {
                return Err(JsonError::unprocessable_entity("Invalid cursor".into()));
            }
            get_entries_from_cursor_use_case(
                &DynamoDbLedgerEntryRepository::from(app_state.dynamo_client),
                cursor,
                query_params.limit,
            )
            .await
        }
        (Some(_), _, _, _) => {
            return Err(JsonError::unprocessable_entity(
                "You can't provide a cursor and a range of dates or order".into(),
            )
            .into())
        }
        (None, Some(start_date), Some(end_date), order) => {
            get_entries_use_case(
                &DynamoDbLedgerEntryRepository::from(app_state.dynamo_client),
                &account_id,
                &start_date,
                &end_date,
                query_params.limit,
                &order.unwrap_or(Order::Desc),
            )
            .await
        }
        (None, _, _, _) => {
            return Err(JsonError::unprocessable_entity(
                "You need to provide both the `start_date` and the `end_date`".into(),
            )
            .into())
        }
    };
    match result {
        Ok((balances, cursor)) => Ok(Json(GetEntriesLedgerResponse {
            entries: balances.into_iter().map(|entry| entry.into()).collect(),
            cursor: cursor.map(|cursor| cursor.encode()).transpose()?,
        })),
        Err(GetBalanceError::NotFound(account_id)) => Err(JsonError::not_found(
            format!("Account {} not found", account_id).into(),
        )),
        Err(e) => Err(anyhow::Error::from(e).into()),
    }
}

#[derive(Deserialize)]
pub struct GetEntriesParams {
    limit: u8,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
    cursor: Option<String>,
    order: Option<Order>,
}

#[derive(Serialize)]
pub struct GetEntriesLedgerResponse {
    entries: Vec<LedgerResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
}
