use crate::domain::entity::LedgerBalanceName;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Conditional {
    GreaterThanOrEqualTo {
        balance: LedgerBalanceName,
        value: i128,
    },
}
