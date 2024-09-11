use serde::{Deserialize, Serialize};

pub use account_id::AccountId;
pub use conditional::Conditional;
pub use cursor::{Cursor, EntryToContinue};
#[cfg(test)]
pub use entry::test::{EntryBuilder, EntryWithBalanceBuilder};
pub use entry::{Entry, EntryId, EntryStatus, EntryWithBalance, EntryWithConditionals};
pub use ledger_balance_name::LedgerBalanceName;
pub use ledger_field_name::LedgerFieldName;

mod account_id;
mod conditional;
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
