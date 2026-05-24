use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Lang {
    Ru,
    En,
    Auto,
}

impl Default for Lang {
    fn default() -> Self {
        Lang::Auto
    }
}
