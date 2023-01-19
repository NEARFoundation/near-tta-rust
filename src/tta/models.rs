use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{types::Decimal, Type};

pub type Transactions = Vec<Transaction>;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    #[serde(rename = "t_transaction_hash")]
    pub t_transaction_hash: String,
    #[serde(rename = "t_included_in_block_hash")]
    pub t_included_in_block_hash: String,
    #[serde(rename = "t_included_in_chunk_hash")]
    pub t_included_in_chunk_hash: String,
    #[serde(rename = "t_index_in_chunk")]
    pub t_index_in_chunk: i32,
    #[serde(rename = "t_block_timestamp")]
    pub t_block_timestamp: Decimal,
    #[serde(rename = "t_signer_account_id")]
    pub t_signer_account_id: String,
    #[serde(rename = "t_signer_public_key")]
    pub t_signer_public_key: String,
    #[serde(rename = "t_nonce")]
    pub t_nonce: Decimal,
    #[serde(rename = "t_receiver_account_id")]
    pub t_receiver_account_id: String,
    #[serde(rename = "t_signature")]
    pub t_signature: String,
    #[serde(rename = "t_status")]
    pub t_status: String,
    #[serde(rename = "t_converted_into_receipt_id")]
    pub t_converted_into_receipt_id: String,
    #[serde(rename = "t_receipt_conversion_gas_burnt")]
    pub t_receipt_conversion_gas_burnt: std::option::Option<Decimal>,
    #[serde(rename = "t_receipt_conversion_tokens_burnt")]
    pub t_receipt_conversion_tokens_burnt: std::option::Option<Decimal>,
    #[serde(rename = "r_receipt_id")]
    pub r_receipt_id: String,
    #[serde(rename = "r_included_in_block_hash")]
    pub r_included_in_block_hash: String,
    #[serde(rename = "r_included_in_chunk_hash")]
    pub r_included_in_chunk_hash: String,
    #[serde(rename = "r_index_in_chunk")]
    pub r_index_in_chunk: i32,
    #[serde(rename = "r_included_in_block_timestamp")]
    pub r_included_in_block_timestamp: Decimal,
    #[serde(rename = "r_predecessor_account_id")]
    pub r_predecessor_account_id: String,
    #[serde(rename = "r_receiver_account_id")]
    pub r_receiver_account_id: String,
    #[serde(rename = "r_receipt_kind")]
    pub r_receipt_kind: String,
    #[serde(rename = "r_originated_from_transaction_hash")]
    pub r_originated_from_transaction_hash: String,
    #[serde(rename = "ta_transaction_hash")]
    pub ta_transaction_hash: String,
    #[serde(rename = "ta_index_in_transaction")]
    pub ta_index_in_transaction: i32,
    #[serde(rename = "ta_action_kind")]
    pub ta_action_kind: String,
    #[serde(rename = "ta_args")]
    pub ta_args: serde_json::Value,
    // pub ta_args: sqlx::types::Json<TaArgs>,
    #[serde(rename = "ara_receipt_id")]
    pub ara_receipt_id: String,
    #[serde(rename = "ara_index_in_action_receipt")]
    pub ara_index_in_action_receipt: i32,
    #[serde(rename = "ara_action_kind")]
    pub ara_action_kind: String,
    #[serde(rename = "ara_args")]
    pub ara_args: serde_json::Value,
    // pub ara_args: AraArgs,
    #[serde(rename = "ara_receipt_predecessor_account_id")]
    pub ara_receipt_predecessor_account_id: String,
    #[serde(rename = "ara_receipt_receiver_account_id")]
    pub ara_receipt_receiver_account_id: String,
    #[serde(rename = "ara_receipt_included_in_block_timestamp")]
    pub ara_receipt_included_in_block_timestamp: Decimal,
    #[serde(rename = "b_block_height")]
    pub b_block_height: Decimal,
    #[serde(rename = "b_block_hash")]
    pub b_block_hash: String,
    #[serde(rename = "b_prev_block_hash")]
    pub b_prev_block_hash: String,
    #[serde(rename = "b_block_timestamp")]
    pub b_block_timestamp: Decimal,
    #[serde(rename = "b_total_supply")]
    pub b_total_supply: Decimal,
    #[serde(rename = "b_gas_price")]
    pub b_gas_price: Decimal,
    #[serde(rename = "b_author_account_id")]
    pub b_author_account_id: String,
    #[serde(rename = "eo_receipt_id")]
    pub eo_receipt_id: String,
    #[serde(rename = "eo_executed_in_block_hash")]
    pub eo_executed_in_block_hash: String,
    #[serde(rename = "eo_executed_in_block_timestamp")]
    pub eo_executed_in_block_timestamp: Decimal,
    #[serde(rename = "eo_index_in_chunk")]
    pub eo_index_in_chunk: i32,
    #[serde(rename = "eo_gas_burnt")]
    pub eo_gas_burnt: Decimal,
    #[serde(rename = "eo_tokens_burnt")]
    pub eo_tokens_burnt: Decimal,
    #[serde(rename = "eo_executor_account_id")]
    pub eo_executor_account_id: String,
    #[serde(rename = "eo_status")]
    pub eo_status: String,
    #[serde(rename = "eo_shard_id")]
    pub eo_shard_id: Decimal,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaArgs {
    pub gas: Option<i64>,
    pub deposit: Option<String>,
    #[serde(rename = "args_json")]
    pub args_json: Option<ArgsJson>,
    #[serde(rename = "args_base64")]
    pub args_base64: Option<String>,
    #[serde(rename = "method_name")]
    pub method_name: Option<String>,
    #[serde(rename = "access_key")]
    pub access_key: Option<AccessKey>,
    #[serde(rename = "public_key")]
    pub public_key: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgsJson {
    #[serde(rename = "estimated_fee")]
    pub estimated_fee: Value,
    pub msg: Option<String>,
    pub amount: Value,
    #[serde(rename = "receiver_id")]
    pub receiver_id: Option<String>,
    #[serde(rename = "account_id")]
    pub account_id: Option<String>,
    #[serde(rename = "registration_only")]
    pub registration_only: Option<bool>,
    pub id: Option<i64>,
    pub action: Option<String>,
    pub proposal: Option<Proposal>,
    pub args: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "token_id")]
    pub token_id: Option<String>,
    pub unregister: Option<bool>,
    pub shares: Option<String>,
    #[serde(rename = "pool_id")]
    pub pool_id: Option<i64>,
    #[serde(rename = "min_amounts")]
    pub min_amounts: Option<Vec<String>>,
    pub amounts: Option<Vec<String>>,
    #[serde(rename = "min_shares")]
    pub min_shares: Option<String>,
    pub accounts: Option<Vec<Account>>,
    #[serde(default)]
    pub receivers: Vec<String>,
    #[serde(rename = "min_fee")]
    pub min_fee: Option<String>,
    #[serde(rename = "account_ids")]
    #[serde(default)]
    pub account_ids: Vec<String>,
    pub expected: Option<Expected>,
    #[serde(rename = "public_key")]
    pub public_key: Option<String>,
    #[serde(rename = "request_id")]
    pub request_id: Option<i64>,
    pub request: Option<Request>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub kind: Kind,
    pub description: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Kind {
    #[serde(rename = "RemoveMemberFromRole")]
    pub remove_member_from_role: Option<RemoveMemberFromRole>,
    #[serde(rename = "AddMemberToRole")]
    pub add_member_to_role: Option<AddMemberToRole>,
    #[serde(rename = "FunctionCall")]
    pub function_call: Option<FunctionCall>,
    #[serde(rename = "Transfer")]
    pub transfer: Option<Transfer>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveMemberFromRole {
    pub role: String,
    #[serde(rename = "member_id")]
    pub member_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMemberToRole {
    pub role: String,
    #[serde(rename = "member_id")]
    pub member_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCall {
    pub actions: Vec<Action>,
    #[serde(rename = "receiver_id")]
    pub receiver_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub gas: String,
    pub args: String,
    pub deposit: String,
    #[serde(rename = "method_name")]
    pub method_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    pub amount: String,
    #[serde(rename = "token_id")]
    pub token_id: String,
    #[serde(rename = "receiver_id")]
    pub receiver_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub amount: String,
    #[serde(rename = "account_id")]
    pub account_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Expected {
    pub decimals: i64,
    pub slippage: String,
    pub multiplier: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub actions: Vec<Action2>,
    #[serde(rename = "receiver_id")]
    pub receiver_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action2 {
    #[serde(rename = "type")]
    pub type_field: String,
    pub amount: Option<String>,
    pub gas: Value,
    pub args: Option<String>,
    pub deposit: Value,
    #[serde(rename = "method_name")]
    pub method_name: Option<String>,
    pub permission: Option<Permission>,
    #[serde(rename = "public_key")]
    pub public_key: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    pub allowance: Value,
    #[serde(rename = "receiver_id")]
    pub receiver_id: String,
    #[serde(rename = "method_names")]
    pub method_names: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessKey {
    pub nonce: i64,
    pub permission: Permission2,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission2 {
    #[serde(rename = "permission_kind")]
    pub permission_kind: String,
    #[serde(rename = "permission_details")]
    pub permission_details: Option<PermissionDetails>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDetails {
    pub allowance: String,
    #[serde(rename = "receiver_id")]
    pub receiver_id: String,
    #[serde(rename = "method_names")]
    pub method_names: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AraArgs {
    pub gas: Option<i64>,
    pub deposit: Option<String>,
    #[serde(rename = "args_json")]
    pub args_json: Option<ArgsJson2>,
    #[serde(rename = "args_base64")]
    pub args_base64: Option<String>,
    #[serde(rename = "method_name")]
    pub method_name: Option<String>,
    #[serde(rename = "access_key")]
    pub access_key: Option<AccessKey2>,
    #[serde(rename = "public_key")]
    pub public_key: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgsJson2 {
    #[serde(rename = "estimated_fee")]
    pub estimated_fee: Value,
    pub msg: Option<String>,
    pub amount: Value,
    #[serde(rename = "receiver_id")]
    pub receiver_id: Option<String>,
    #[serde(rename = "account_id")]
    pub account_id: Option<String>,
    #[serde(rename = "registration_only")]
    pub registration_only: Option<bool>,
    pub id: Option<i64>,
    pub action: Option<String>,
    pub proposal: Option<Proposal2>,
    pub args: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "token_id")]
    pub token_id: Option<String>,
    pub unregister: Option<bool>,
    pub shares: Option<String>,
    #[serde(rename = "pool_id")]
    pub pool_id: Option<i64>,
    #[serde(rename = "min_amounts")]
    pub min_amounts: Option<Vec<String>>,
    pub amounts: Option<Vec<String>>,
    #[serde(rename = "min_shares")]
    pub min_shares: Option<String>,
    pub accounts: Option<Vec<Account2>>,
    #[serde(default)]
    pub receivers: Vec<String>,
    #[serde(rename = "min_fee")]
    pub min_fee: Option<String>,
    #[serde(rename = "account_ids")]
    #[serde(default)]
    pub account_ids: Vec<String>,
    pub expected: Option<Expected2>,
    #[serde(rename = "public_key")]
    pub public_key: Option<String>,
    #[serde(rename = "request_id")]
    pub request_id: Option<i64>,
    pub request: Option<Request2>,
    #[serde(rename = "lockup_duration")]
    pub lockup_duration: Option<String>,
    #[serde(rename = "lockup_timestamp")]
    pub lockup_timestamp: Option<String>,
    #[serde(rename = "owner_account_id")]
    pub owner_account_id: Option<String>,
    #[serde(rename = "release_duration")]
    pub release_duration: Option<String>,
    #[serde(rename = "whitelist_account_id")]
    pub whitelist_account_id: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal2 {
    pub kind: Kind2,
    pub description: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Kind2 {
    #[serde(rename = "RemoveMemberFromRole")]
    pub remove_member_from_role: Option<RemoveMemberFromRole2>,
    #[serde(rename = "AddMemberToRole")]
    pub add_member_to_role: Option<AddMemberToRole2>,
    #[serde(rename = "FunctionCall")]
    pub function_call: Option<FunctionCall2>,
    #[serde(rename = "Transfer")]
    pub transfer: Option<Transfer2>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveMemberFromRole2 {
    pub role: String,
    #[serde(rename = "member_id")]
    pub member_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMemberToRole2 {
    pub role: String,
    #[serde(rename = "member_id")]
    pub member_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCall2 {
    pub actions: Vec<Action3>,
    #[serde(rename = "receiver_id")]
    pub receiver_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action3 {
    pub gas: String,
    pub args: String,
    pub deposit: String,
    #[serde(rename = "method_name")]
    pub method_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer2 {
    pub amount: String,
    #[serde(rename = "token_id")]
    pub token_id: String,
    #[serde(rename = "receiver_id")]
    pub receiver_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account2 {
    pub amount: String,
    #[serde(rename = "account_id")]
    pub account_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Expected2 {
    pub decimals: i64,
    pub slippage: String,
    pub multiplier: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request2 {
    pub actions: Vec<Action4>,
    #[serde(rename = "receiver_id")]
    pub receiver_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action4 {
    #[serde(rename = "type")]
    pub type_field: String,
    pub amount: Option<String>,
    pub gas: Value,
    pub args: Option<String>,
    pub deposit: Value,
    #[serde(rename = "method_name")]
    pub method_name: Option<String>,
    pub permission: Option<Permission3>,
    #[serde(rename = "public_key")]
    pub public_key: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission3 {
    pub allowance: Value,
    #[serde(rename = "receiver_id")]
    pub receiver_id: String,
    #[serde(rename = "method_names")]
    pub method_names: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessKey2 {
    pub nonce: i64,
    pub permission: Permission4,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission4 {
    #[serde(rename = "permission_kind")]
    pub permission_kind: String,
    #[serde(rename = "permission_details")]
    pub permission_details: Option<PermissionDetails2>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDetails2 {
    pub allowance: Option<String>,
    #[serde(rename = "receiver_id")]
    pub receiver_id: String,
    #[serde(rename = "method_names")]
    pub method_names: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
#[sqlx(rename_all = "snake_case", type_name = "execution_outcome_status")]
// #[serde(rename_all = "snake_case")]
pub enum ExecutionOutcomeStatus {
    #[default]
    Unknown,
    Failure,
    SuccessValue,
    SuccessReceiptId,
}
