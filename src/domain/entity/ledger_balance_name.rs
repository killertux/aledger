use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::domain::entity::LedgerFieldName;

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Hash, Clone)]
pub struct LedgerBalanceName(String);

impl LedgerBalanceName {
    pub fn new(value: String) -> anyhow::Result<Self> {
        if !value.starts_with("balance_") {
            bail!("Ledger balance name must start with balance_");
        }
        Ok(Self(value))
    }
}

impl From<LedgerBalanceName> for String {
    fn from(value: LedgerBalanceName) -> String {
        value.0
    }
}

impl From<LedgerFieldName> for LedgerBalanceName {
    fn from(value: LedgerFieldName) -> Self {
        Self(format!("balance_{}", String::from(value)))
    }
}
