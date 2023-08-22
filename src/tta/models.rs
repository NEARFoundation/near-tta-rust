use near_primitives::types::AccountId;
use near_sdk::json_types::U128;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct ReportRow {
    pub date: String,
    pub account_id: String,
    pub method_name: String,
    pub block_timestamp: u128,
    pub from_account: String,
    pub block_height: u128,
    pub args: String,
    pub transaction_hash: String,
    pub amount_transferred: f64,
    pub currency_transferred: String,
    pub ft_amount_out: Option<f64>,
    pub ft_currency_out: Option<String>,
    pub ft_amount_in: Option<f64>,
    pub ft_currency_in: Option<String>,
    pub to_account: String,
    pub amount_staked: f64,
    pub onchain_balance: Option<f64>,
}

// Define the extension trait
pub trait FloatExt {
    fn to_5dp_string(&self) -> String;
}

// Implement the extension trait for f64
impl FloatExt for f64 {
    fn to_5dp_string(&self) -> String {
        format!("{:.5}", self)
    }
}

impl ReportRow {
    pub fn get_vec_headers() -> Vec<String> {
        vec![
            "date".to_string(),
            "account_id".to_string(),
            "method_name".to_string(),
            "block_timestamp".to_string(),
            "from_account".to_string(),
            "block_height".to_string(),
            "args".to_string(),
            "transaction_hash".to_string(),
            "amount_transferred".to_string(),
            "currency_transferred".to_string(),
            "ft_amount_out".to_string(),
            "ft_currency_out".to_string(),
            "ft_amount_in".to_string(),
            "ft_currency_in".to_string(),
            "to_account".to_string(),
            "amount_staked".to_string(),
            "onchain_balance".to_string(),
        ]
    }

    pub fn to_vec(&self) -> Vec<String> {
        vec![
            self.date.clone(),
            self.account_id.clone(),
            self.method_name.clone(),
            self.block_timestamp.to_string(),
            self.from_account.clone(),
            self.block_height.to_string(),
            self.args.clone(),
            self.transaction_hash.clone(),
            self.amount_transferred.to_5dp_string(),
            self.currency_transferred.clone(),
            self.ft_amount_out
                .map_or(String::new(), |v| v.to_5dp_string()),
            self.ft_currency_out.clone().unwrap_or_default(),
            self.ft_amount_in
                .map_or(String::new(), |v| v.to_5dp_string()),
            self.ft_currency_in.clone().unwrap_or_default(),
            self.to_account.clone(),
            self.amount_staked.to_5dp_string(),
            self.onchain_balance
                .map_or(String::new(), |v| v.to_5dp_string()),
        ]
    }
}

#[derive(Debug, Clone)]
pub struct FtAmounts {
    pub ft_amount_out: Option<f64>,
    pub ft_currency_out: Option<String>,
    pub ft_amount_in: Option<f64>,
    pub ft_currency_in: Option<String>,
    pub from_account: String,
    pub to_account: String,
}

#[derive(Debug, PartialEq)]
pub enum MethodName {
    FtTransfer,
    FtTransferCall,
    Withdraw,
    NearDeposit,
    NearWithdraw,
    Mint,
    Unsupported,
}

impl From<&str> for MethodName {
    fn from(s: &str) -> Self {
        match s {
            "ft_transfer" => MethodName::FtTransfer,
            "ft_transfer_call" => MethodName::FtTransferCall,
            "withdraw" => MethodName::Withdraw,
            "near_deposit" => MethodName::NearDeposit,
            "near_withdraw" => MethodName::NearWithdraw,
            "mint" => MethodName::Mint,
            _ => MethodName::Unsupported,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FtTransfer {
    pub receiver_id: AccountId,
    pub amount: U128,
    pub memo: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FtTransferCall {
    pub receiver_id: AccountId,
    pub amount: U128,
    pub memo: Option<String>,
    pub msg: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Swap {
    pub token_in: String,
    pub amount_in: U128,
    pub token_out: String,
    pub min_amount_out: U128,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct WithdrawFromBridge {
    pub amount: U128,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RainbowBridgeMint {
    pub account_id: AccountId,
    pub amount: U128,
}
