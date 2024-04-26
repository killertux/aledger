use super::LedgerResponse;
use crate::{
    controller::JsonError,
    domain::{entity::AccountId, gateway::GetBalanceError, use_case::get_balance_use_case},
    gateway::ledger_entry_repository::DynamoDbLedgerEntryRepository,
    AppState,
};
use axum::{
    debug_handler,
    extract::{Path, State},
    Json,
};

#[debug_handler]
pub async fn get_balance(
    State(app_state): State<AppState>,
    Path(account_id): Path<AccountId>,
) -> Result<Json<LedgerResponse>, JsonError<'static>> {
    match get_balance_use_case(
        &DynamoDbLedgerEntryRepository::from(app_state.dynamo_client),
        &account_id,
    )
    .await
    {
        Ok(balance) => Ok(Json(balance.into())),
        Err(GetBalanceError::NotFound(account_id)) => Err(JsonError::not_found(
            format!("Account {} not found", account_id).into(),
        )),
        Err(e) => Err(anyhow::Error::from(e).into()),
    }
}
