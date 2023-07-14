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
    pub ft_amount_out: f64,
    pub ft_currency_out: String,
    pub ft_amount_in: f64,
    pub ft_currency_in: String,
    pub to_account: String,
    pub amount_staked: f64,
    pub onchain_usdc_balance: f64,
    pub onchain_usdt_balance: f64,
}

impl ReportRow {
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
            self.amount_transferred.to_string(),
            self.currency_transferred.clone(),
            self.ft_amount_out.to_string(),
            self.ft_currency_out.clone(),
            self.ft_amount_in.to_string(),
            self.ft_currency_in.clone(),
            self.to_account.clone(),
            self.amount_staked.to_string(),
            self.onchain_usdc_balance.to_string(),
            self.onchain_usdt_balance.to_string(),
        ]
    }
}
