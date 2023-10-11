use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{types::Decimal, Type};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Transaction {
    #[serde(rename = "t_transaction_hash", default)]
    pub t_transaction_hash: String,
    #[serde(rename = "t_included_in_block_hash", default)]
    pub t_included_in_block_hash: String,
    #[serde(rename = "t_included_in_chunk_hash", default)]
    pub t_included_in_chunk_hash: String,
    #[serde(rename = "t_index_in_chunk", default)]
    pub t_index_in_chunk: i32,
    #[serde(rename = "t_block_timestamp", default)]
    pub t_block_timestamp: Decimal,
    #[serde(rename = "t_signer_account_id", default)]
    pub t_signer_account_id: String,
    #[serde(rename = "t_signer_public_key", default)]
    pub t_signer_public_key: String,
    #[serde(rename = "t_nonce", default)]
    pub t_nonce: Decimal,
    #[serde(rename = "t_receiver_account_id", default)]
    pub t_receiver_account_id: String,
    #[serde(rename = "t_signature", default)]
    pub t_signature: String,
    #[serde(rename = "t_status", default)]
    pub t_status: String,
    #[serde(rename = "t_converted_into_receipt_id", default)]
    pub t_converted_into_receipt_id: String,
    #[serde(rename = "t_receipt_conversion_gas_burnt", default)]
    pub t_receipt_conversion_gas_burnt: std::option::Option<Decimal>,
    #[serde(rename = "t_receipt_conversion_tokens_burnt", default)]
    pub t_receipt_conversion_tokens_burnt: std::option::Option<Decimal>,
    #[serde(rename = "r_receipt_id", default)]
    pub r_receipt_id: String,
    #[serde(rename = "r_included_in_block_hash", default)]
    pub r_included_in_block_hash: String,
    #[serde(rename = "r_included_in_chunk_hash", default)]
    pub r_included_in_chunk_hash: String,
    #[serde(rename = "r_index_in_chunk", default)]
    pub r_index_in_chunk: i32,
    #[serde(rename = "r_included_in_block_timestamp", default)]
    pub r_included_in_block_timestamp: Decimal,
    #[serde(rename = "r_predecessor_account_id", default)]
    pub r_predecessor_account_id: String,
    #[serde(rename = "r_receiver_account_id", default)]
    pub r_receiver_account_id: String,
    #[serde(rename = "r_receipt_kind", default)]
    pub r_receipt_kind: String,
    #[serde(rename = "r_originated_from_transaction_hash", default)]
    pub r_originated_from_transaction_hash: String,
    // #[serde(rename = "ta_transaction_hash", default)]
    // pub ta_transaction_hash: String,
    // #[serde(rename = "ta_index_in_transaction", default)]
    // pub ta_index_in_transaction: i32,
    // #[serde(rename = "ta_action_kind", default)]
    // pub ta_action_kind: String,
    // #[serde(rename = "ta_args", default)]
    // pub ta_args: serde_json::Value,
    // pub ta_args: sqlx::types::Json<TaArgs>,
    #[serde(rename = "ara_receipt_id", default)]
    pub ara_receipt_id: String,
    #[serde(rename = "ara_index_in_action_receipt", default)]
    pub ara_index_in_action_receipt: i32,
    #[serde(rename = "ara_action_kind", default)]
    pub ara_action_kind: String,
    #[serde(rename = "ara_args", default)]
    pub ara_args: serde_json::Value,
    // pub ara_args: AraArgs,
    #[serde(rename = "ara_receipt_predecessor_account_id", default)]
    pub ara_receipt_predecessor_account_id: String,
    #[serde(rename = "ara_receipt_receiver_account_id", default)]
    pub ara_receipt_receiver_account_id: String,
    #[serde(rename = "ara_receipt_included_in_block_timestamp", default)]
    pub ara_receipt_included_in_block_timestamp: Decimal,
    #[serde(rename = "b_block_height", default)]
    pub b_block_height: Decimal,
    #[serde(rename = "b_block_hash", default)]
    pub b_block_hash: String,
    #[serde(rename = "b_prev_block_hash", default)]
    pub b_prev_block_hash: String,
    #[serde(rename = "b_block_timestamp", default)]
    pub b_block_timestamp: Decimal,
    // #[serde(rename = "b_total_supply", default)]
    // pub b_total_supply: Decimal,
    #[serde(rename = "b_gas_price", default)]
    pub b_gas_price: Decimal,
    #[serde(rename = "b_author_account_id", default)]
    pub b_author_account_id: String,
    #[serde(rename = "eo_receipt_id", default)]
    pub eo_receipt_id: String,
    #[serde(rename = "eo_executed_in_block_hash", default)]
    pub eo_executed_in_block_hash: String,
    #[serde(rename = "eo_executed_in_block_timestamp", default)]
    pub eo_executed_in_block_timestamp: Decimal,
    #[serde(rename = "eo_index_in_chunk", default)]
    pub eo_index_in_chunk: i32,
    #[serde(rename = "eo_gas_burnt", default)]
    pub eo_gas_burnt: Decimal,
    #[serde(rename = "eo_tokens_burnt", default)]
    pub eo_tokens_burnt: Decimal,
    #[serde(rename = "eo_executor_account_id", default)]
    pub eo_executor_account_id: String,
    #[serde(rename = "eo_status", default)]
    pub eo_status: String,
    #[serde(rename = "eo_shard_id", default)]
    pub eo_shard_id: Decimal,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TaArgs {
    pub gas: Option<i64>,
    pub deposit: Option<String>,
    #[serde(rename = "args_json", default)]
    pub args_json: Option<ArgsJson>,
    #[serde(rename = "args_base64", default)]
    pub args_base64: Option<String>,
    #[serde(rename = "method_name", default)]
    pub method_name: Option<String>,
    #[serde(rename = "access_key", default)]
    pub access_key: Option<AccessKey>,
    #[serde(rename = "public_key", default)]
    pub public_key: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArgsJson {
    #[serde(rename = "estimated_fee", default)]
    pub estimated_fee: Value,
    pub msg: Option<String>,
    pub amount: Value,
    #[serde(rename = "receiver_id", default)]
    pub receiver_id: Option<String>,
    #[serde(rename = "account_id", default)]
    pub account_id: Option<String>,
    #[serde(rename = "registration_only", default)]
    pub registration_only: Option<bool>,
    pub id: Option<i64>,
    pub action: Option<String>,
    pub proposal: Option<Proposal>,
    pub args: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "token_id", default)]
    pub token_id: Option<String>,
    pub unregister: Option<bool>,
    pub shares: Option<String>,
    #[serde(rename = "pool_id", default)]
    pub pool_id: Option<i64>,
    #[serde(rename = "min_amounts", default)]
    pub min_amounts: Option<Vec<String>>,
    pub amounts: Option<Vec<String>>,
    #[serde(rename = "min_shares", default)]
    pub min_shares: Option<String>,
    pub accounts: Option<Vec<Account>>,
    #[serde(default)]
    pub receivers: Vec<String>,
    #[serde(rename = "min_fee", default)]
    pub min_fee: Option<String>,
    #[serde(rename = "account_ids", default)]
    pub account_ids: Vec<String>,
    pub expected: Option<Expected>,
    #[serde(rename = "public_key", default)]
    pub public_key: Option<String>,
    #[serde(rename = "request_id", default)]
    pub request_id: Option<i64>,
    pub request: Option<Request>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Proposal {
    pub kind: Kind,
    pub description: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Kind {
    #[serde(rename = "RemoveMemberFromRole", default)]
    pub remove_member_from_role: Option<RemoveMemberFromRole>,
    #[serde(rename = "AddMemberToRole", default)]
    pub add_member_to_role: Option<AddMemberToRole>,
    #[serde(rename = "FunctionCall", default)]
    pub function_call: Option<FunctionCall>,
    #[serde(rename = "Transfer", default)]
    pub transfer: Option<Transfer>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoveMemberFromRole {
    pub role: String,
    #[serde(rename = "member_id", default)]
    pub member_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AddMemberToRole {
    pub role: String,
    #[serde(rename = "member_id", default)]
    pub member_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FunctionCall {
    pub actions: Vec<Action>,
    #[serde(rename = "receiver_id", default)]
    pub receiver_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Action {
    pub gas: String,
    pub args: String,
    pub deposit: String,
    #[serde(rename = "method_name", default)]
    pub method_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Transfer {
    pub amount: String,
    #[serde(rename = "token_id", default)]
    pub token_id: String,
    #[serde(rename = "receiver_id", default)]
    pub receiver_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Account {
    pub amount: String,
    #[serde(rename = "account_id", default)]
    pub account_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Expected {
    pub decimals: i64,
    pub slippage: String,
    pub multiplier: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Request {
    pub actions: Vec<Action2>,
    #[serde(rename = "receiver_id", default)]
    pub receiver_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Action2 {
    #[serde(rename = "type", default)]
    pub type_field: String,
    pub amount: Option<String>,
    pub gas: Value,
    pub args: Option<String>,
    pub deposit: Value,
    #[serde(rename = "method_name", default)]
    pub method_name: Option<String>,
    pub permission: Option<Permission>,
    #[serde(rename = "public_key", default)]
    pub public_key: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Permission {
    pub allowance: Value,
    #[serde(rename = "receiver_id", default)]
    pub receiver_id: String,
    #[serde(rename = "method_names", default)]
    pub method_names: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AccessKey {
    pub nonce: i64,
    pub permission: Permission2,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Permission2 {
    #[serde(rename = "permission_kind", default)]
    pub permission_kind: String,
    #[serde(rename = "permission_details", default)]
    pub permission_details: Option<PermissionDetails>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PermissionDetails {
    pub allowance: String,
    #[serde(rename = "receiver_id", default)]
    pub receiver_id: String,
    #[serde(rename = "method_names", default)]
    pub method_names: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AraArgs {
    pub gas: Option<i64>,
    pub deposit: Option<String>,
    #[serde(rename = "args_json", default)]
    pub args_json: Option<FunctionCallParameters>,
    #[serde(rename = "args_base64", default)]
    pub args_base64: Option<String>,
    #[serde(rename = "method_name", default)]
    pub method_name: Option<String>,
    #[serde(rename = "access_key", default)]
    pub access_key: Option<AccessKey2>,
    #[serde(rename = "public_key", default)]
    pub public_key: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FunctionCallParameters {
    #[serde(rename = "estimated_fee", default)]
    pub estimated_fee: Option<Value>,
    pub msg: Option<String>,
    pub amount: Option<Value>,
    #[serde(rename = "receiver_id", default)]
    pub receiver_id: Option<String>,
    #[serde(rename = "account_id", default)]
    pub account_id: Option<String>,
    #[serde(rename = "registration_only", default)]
    pub registration_only: Option<bool>,
    pub id: Option<i64>,
    pub action: Option<String>,
    pub proposal: Option<Proposal2>,
    pub args: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "token_id", default)]
    pub token_id: Option<String>,
    pub unregister: Option<bool>,
    pub shares: Option<String>,
    #[serde(rename = "pool_id", default)]
    pub pool_id: Option<i64>,
    #[serde(rename = "min_amounts", default)]
    pub min_amounts: Option<Vec<String>>,
    pub amounts: Option<Vec<String>>,
    #[serde(rename = "min_shares", default)]
    pub min_shares: Option<String>,
    pub accounts: Option<Vec<Account2>>,
    #[serde(default)]
    pub receivers: Vec<String>,
    #[serde(rename = "min_fee", default)]
    pub min_fee: Option<String>,
    #[serde(rename = "account_ids", default)]
    pub account_ids: Vec<String>,
    pub expected: Option<Expected2>,
    #[serde(rename = "public_key", default)]
    pub public_key: Option<String>,
    #[serde(rename = "request_id", default)]
    pub request_id: Option<i64>,
    #[serde(rename = "request", default)]
    pub request: Option<MultiSigRequest>,
    #[serde(rename = "lockup_duration", default)]
    pub lockup_duration: Option<String>,
    #[serde(rename = "lockup_timestamp", default)]
    pub lockup_timestamp: Option<String>,
    #[serde(rename = "owner_account_id", default)]
    pub owner_account_id: Option<String>,
    #[serde(rename = "release_duration", default)]
    pub release_duration: Option<String>,
    #[serde(rename = "whitelist_account_id", default)]
    pub whitelist_account_id: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Proposal2 {
    pub kind: Kind2,
    pub description: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Kind2 {
    #[serde(rename = "RemoveMemberFromRole", default)]
    pub remove_member_from_role: Option<RemoveMemberFromRole2>,
    #[serde(rename = "AddMemberToRole", default)]
    pub add_member_to_role: Option<AddMemberToRole2>,
    #[serde(rename = "FunctionCall", default)]
    pub function_call: Option<FunctionCall2>,
    #[serde(rename = "Transfer", default)]
    pub transfer: Option<Transfer2>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoveMemberFromRole2 {
    pub role: String,
    #[serde(rename = "member_id", default)]
    pub member_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AddMemberToRole2 {
    pub role: String,
    #[serde(rename = "member_id", default)]
    pub member_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FunctionCall2 {
    pub actions: Vec<Action3>,
    #[serde(rename = "receiver_id", default)]
    pub receiver_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Action3 {
    pub gas: String,
    pub args: String,
    pub deposit: String,
    #[serde(rename = "method_name", default)]
    pub method_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Transfer2 {
    pub amount: String,
    #[serde(rename = "token_id", default)]
    pub token_id: String,
    #[serde(rename = "receiver_id", default)]
    pub receiver_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Account2 {
    pub amount: String,
    #[serde(rename = "account_id", default)]
    pub account_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Expected2 {
    pub decimals: i64,
    pub slippage: String,
    pub multiplier: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MultiSigRequest {
    pub actions: Vec<Action4>,
    #[serde(rename = "receiver_id", default)]
    pub receiver_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Action4 {
    #[serde(rename = "type", default)]
    pub type_field: Option<String>,
    pub amount: Option<String>,
    pub gas: Option<Value>,
    pub args: Option<String>,
    pub deposit: Option<Value>,
    #[serde(rename = "method_name", default)]
    pub method_name: Option<String>,
    pub permission: Option<Permission3>,
    #[serde(rename = "public_key", default)]
    pub public_key: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Permission3 {
    pub allowance: Value,
    #[serde(rename = "receiver_id", default)]
    pub receiver_id: String,
    #[serde(rename = "method_names", default)]
    pub method_names: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AccessKey2 {
    pub nonce: i64,
    pub permission: Permission4,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Permission4 {
    #[serde(rename = "permission_kind", default)]
    pub permission_kind: String,
    #[serde(rename = "permission_details", default)]
    pub permission_details: Option<PermissionDetails2>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PermissionDetails2 {
    pub allowance: Option<String>,
    #[serde(rename = "receiver_id", default)]
    pub receiver_id: String,
    #[serde(rename = "method_names", default)]
    pub method_names: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
#[sqlx(rename_all = "snake_case", type_name = "execution_outcome_status")]
// #[serde(rename_all = "snake_case", default)]
pub enum ExecutionOutcomeStatus {
    #[default]
    Unknown,
    Failure,
    SuccessValue,
    SuccessReceiptId,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BlockId {
    #[serde(rename = "block_ud", default)]
    pub block_height: Decimal,
}
