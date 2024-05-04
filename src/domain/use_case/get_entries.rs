use anyhow::anyhow;
use chrono::{DateTime, Utc};

use crate::domain::entity::AccountId;
use crate::domain::entity::Cursor;
use crate::domain::entity::EntryWithBalance;
use crate::domain::entity::Order;
use crate::domain::gateway::{GetBalanceError, LedgerEntryRepository};

pub async fn get_entries_use_case(
    repository: &impl LedgerEntryRepository,
    account_id: &AccountId,
    start_date: &DateTime<Utc>,
    end_date: &DateTime<Utc>,
    limit: u8,
    order: &Order,
) -> Result<(Vec<EntryWithBalance>, Option<Cursor>), GetBalanceError> {
    repository
        .get_entries(account_id, start_date, end_date, limit, order, None)
        .await
}

pub async fn get_entries_from_cursor_use_case(
    repository: &impl LedgerEntryRepository,
    cursor: Cursor,
    limit: u8,
) -> Result<(Vec<EntryWithBalance>, Option<Cursor>), GetBalanceError> {
    let Cursor::FromEntriesQuery {
        start_date,
        end_date,
        order,
        account_id,
        sequence,
    } = cursor
    else {
        return Err(GetBalanceError::Other(anyhow!("Invalid cursor")));
    };
    repository
        .get_entries(
            &account_id,
            &start_date,
            &end_date,
            limit,
            &order,
            Some(sequence),
        )
        .await
}
