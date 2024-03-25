use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FastNearFT {
    #[serde(rename = "account_id")]
    pub account_id: String,
    pub tokens: Vec<Token>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    #[serde(rename = "contract_id")]
    pub contract_id: String,
    #[serde(rename = "last_update_block_height")]
    pub last_update_block_height: Value,
}
