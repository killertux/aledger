use anyhow::bail;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Hash, Clone)]
#[serde(try_from = "String")]
pub struct LedgerFieldName(String);

impl From<LedgerFieldName> for String {
    fn from(value: LedgerFieldName) -> String {
        value.0
    }
}

impl LedgerFieldName {
    pub fn new(field_name: String) -> anyhow::Result<Self> {
        if field_name.starts_with("balance_") {
            bail!("Ledger field cannot start with `balance_`")
        }
        Ok(Self(field_name))
    }
}

impl TryFrom<String> for LedgerFieldName {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        LedgerFieldName::new(value)
    }
}
