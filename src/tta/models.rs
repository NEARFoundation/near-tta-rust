use chrono::DateTime;

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
    pub onchain_usdc_balance: f64,
    pub onchain_usdt_balance: f64,
}

// Define the extension trait
pub trait FloatExt {
    fn to_2dp_string(&self) -> String;
}

// Implement the extension trait for f64
impl FloatExt for f64 {
    fn to_2dp_string(&self) -> String {
        format!("{:.2}", self)
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
            "onchain_usdc_balance".to_string(),
            "onchain_usdt_balance".to_string(),
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
            self.amount_transferred.to_2dp_string(),
            self.currency_transferred.clone(),
            self.ft_amount_out
                .map_or(String::new(), |v| v.to_2dp_string()),
            self.ft_currency_out.clone().unwrap_or_default(),
            self.ft_amount_in
                .map_or(String::new(), |v| v.to_2dp_string()),
            self.ft_currency_in.clone().unwrap_or_default(),
            self.to_account.clone(),
            self.amount_staked.to_2dp_string(),
            self.onchain_usdc_balance.to_2dp_string(),
            self.onchain_usdt_balance.to_2dp_string(),
        ]
    }
}

#[derive(Debug)]
pub struct FtAmounts {
    pub ft_amount_out: Option<f64>,
    pub ft_currency_out: Option<String>,
    pub ft_amount_in: Option<f64>,
    pub ft_currency_in: Option<String>,
    pub from_account: String,
    pub to_account: String,
}
