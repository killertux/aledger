use std::fmt::Display;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Ord, PartialOrd, Eq, Hash, Clone)]
pub struct AccountId(Uuid);

impl AccountId {
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<AccountId> for String {
    fn from(value: AccountId) -> String {
        value.0.into()
    }
}

impl Display for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
