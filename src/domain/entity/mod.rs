use serde::{Deserialize, Serialize};

pub use account_id::AccountId;
pub use cursor::Cursor;
pub use entry::{Entry, EntryId, EntryStatus, EntryWithBalance};
pub use ledger_balance_name::LedgerBalanceName;
pub use ledger_field_name::LedgerFieldName;

mod account_id;
mod cursor;
mod entry;
mod ledger_balance_name;
mod ledger_field_name;

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Clone)]
pub enum Order {
    Asc,
    Desc,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct DeleteEntryRequest {
    pub account_id: AccountId,
    pub entry_id: EntryId,
}
