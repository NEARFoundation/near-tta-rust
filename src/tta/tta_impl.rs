use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
    vec,
};

use anyhow::{bail, Context, Result};

use futures_util::future::join_all;
use near_sdk::ONE_NEAR;

use crate::{tta::utils::get_associated_lockup, TxnsReportWithMetadata};
use base64::{engine::general_purpose, Engine as _};
use chrono::{NaiveDateTime, Utc};

use num_traits::cast::ToPrimitive;
use tokio::sync::{
    mpsc::{channel, Sender},
    Semaphore,
};

use tracing::{debug, error, info, instrument};

use super::{
    ft_metadata::{FtMetadata, FtService},
    models::{
        FtAmounts, FtTransfer, FtTransferCall, MethodName, RainbowBridgeMint, ReportRow,
        WithdrawFromBridge,
    },
    sql::{
        models::{TaArgs, Transaction},
        sql_queries::SqlClient,
    },
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TransactionType {
    Incoming,
    FtIncoming,
    Outgoing,
}

impl TransactionType {
    async fn get_transaction(
        self,
        client: &SqlClient,
        accounts: HashSet<String>,
        start_date: u128,
        end_date: u128,
        tx: Sender<Transaction>,
    ) -> Result<()> {
        match self {
            TransactionType::Incoming => {
                client
                    .get_incoming_txns(accounts, start_date, end_date, tx)
                    .await
            }
            TransactionType::FtIncoming => {
                client
                    .get_ft_incoming_txns(accounts, start_date, end_date, tx)
                    .await
            }
            TransactionType::Outgoing => {
                client
                    .get_outgoing_txns(accounts, start_date, end_date, tx)
                    .await
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct TTA {
    sql_client: SqlClient,
    ft_service: FtService,
    semaphore: Arc<Semaphore>,
}

impl TTA {
    pub fn new(sql_client: SqlClient, ft_service: FtService, semaphore: Arc<Semaphore>) -> Self {
        Self {
            sql_client,
            ft_service,
            semaphore,
        }
    }

    #[instrument(skip(self, start_date, end_date, accounts))]
    pub(crate) async fn get_txns_report(
        &self,
        start_date: u128,
        end_date: u128,
        accounts: HashSet<String>,
        include_balances: bool,
        metadata: Arc<RwLock<TxnsReportWithMetadata>>,
    ) -> Result<Vec<ReportRow>> {
        info!(?start_date, ?end_date, ?accounts, "Got request");

        let mut join_handles = vec![];
        let mut report = vec![];
        let started_at = Utc::now();

        for acc in &accounts {
            let t = self;
            let mut wallets_for_account = HashSet::new();
            let lockup = get_associated_lockup(acc, "near");
            info!(?acc, ?lockup, "Got lockup");
            wallets_for_account.insert(acc.clone());
            wallets_for_account.insert(lockup);

            let task_incoming = tokio::spawn({
                info!(
                    "Acquiring semaphore, remaining: {:?}",
                    self.semaphore.available_permits()
                );
                let s = self.semaphore.clone().acquire_owned().await?;
                info!(
                    "Acquired, remaining: {:?}",
                    self.semaphore.available_permits()
                );
                let wallets_for_account = wallets_for_account.clone();
                let t = t.clone();
                let for_account = acc.clone();
                let metadata = metadata.clone();

                async move {
                    let _s = s;
                    t.handle_txns(
                        TransactionType::Incoming,
                        for_account,
                        wallets_for_account,
                        start_date,
                        end_date,
                        include_balances,
                        metadata,
                    )
                    .await
                }
            });

            let task_ft_incoming = tokio::spawn({
                info!(
                    "Acquiring semaphore, remaining: {:?}",
                    self.semaphore.available_permits()
                );
                let s = self.semaphore.clone().acquire_owned().await?;
                info!(
                    "Acquired, remaining: {:?}",
                    self.semaphore.available_permits()
                );
                let wallets_for_account = wallets_for_account.clone();
                let t = t.clone();
                let for_account = acc.clone();
                let metadata = metadata.clone();

                async move {
                    let _s = s;
                    t.handle_txns(
                        TransactionType::FtIncoming,
                        for_account,
                        wallets_for_account,
                        start_date,
                        end_date,
                        include_balances,
                        metadata,
                    )
                    .await
                }
            });

            let task_outgoing = tokio::spawn({
                info!(
                    "Acquiring semaphore, remaining: {:?}",
                    self.semaphore.available_permits()
                );
                let s = self.semaphore.clone().acquire_owned().await?;
                info!(
                    "Acquired, remaining: {:?}",
                    self.semaphore.available_permits()
                );
                let wallets_for_account = wallets_for_account.clone();
                let t = t.clone();
                let a = acc.clone();
                let metadata = metadata.clone();

                async move {
                    let _s = s;

                    t.handle_txns(
                        TransactionType::Outgoing,
                        a,
                        wallets_for_account,
                        start_date,
                        end_date,
                        include_balances,
                        metadata,
                    )
                    .await
                }
            });

            join_handles.push(task_incoming);
            join_handles.push(task_ft_incoming);
            join_handles.push(task_outgoing);
        }

        // Wait for threads to be over.
        for ele in join_handles {
            match ele.await {
                Ok(res) => match res {
                    Ok(partial_report) => {
                        let mut p = vec![];
                        // Apply filtering
                        for ele in partial_report {
                            if let Some(ele) = assert_moves_token(ele) {
                                p.push(ele)
                            }
                        }
                        report.extend(p);
                    }
                    Err(e) => {
                        error!(?e, "Error in returned value from thread");
                    }
                },
                Err(e) => {
                    error!(?e, "Error joining threads");
                }
            }
        }

        // sort the report by account_id and block_timestamp
        report.sort_by(|a, b| {
            a.account_id
                .cmp(&b.account_id)
                .then(a.block_timestamp.cmp(&b.block_timestamp))
        });

        let ended_at = Utc::now();

        info!(
            "It took: {:?}, got {} txns",
            ended_at - started_at,
            report.len()
        );

        Ok(report)
    }

    async fn handle_txns(
        self,
        txn_type: TransactionType,
        for_account: String,
        accounts: HashSet<String>,
        start_date: u128,
        end_date: u128,
        include_balances: bool,
        metadata: Arc<RwLock<TxnsReportWithMetadata>>,
    ) -> Result<Vec<ReportRow>> {
        let mut report: Vec<ReportRow> = vec![];
        let (tx, mut rx) = channel(100);

        let t = self.clone();
        tokio::spawn({
            let a = accounts.clone();
            async move {
                txn_type
                    .get_transaction(&t.sql_client, a, start_date, end_date, tx)
                    .await
                    .unwrap();
            }
        });

        let mut rows_handle = vec![];
        while let Some(txn) = rx.recv().await {
            let t2: TTA = self.clone();
            let for_account = for_account.clone();
            let metadata = metadata.clone();
            let row = tokio::spawn(async move {
                if txn.ara_action_kind != "FUNCTION_CALL" && txn.ara_action_kind != "TRANSFER" {
                    return Ok(None);
                }

                let txn_args = decode_args(&txn)?;

                // Skipping gas refunds
                if get_near_transferred(&txn_args) < 0.5
                    && txn.ara_receipt_predecessor_account_id == "system"
                {
                    return Ok(None);
                }

                let ft_amounts = match t2
                    .get_ft_amounts(
                        txn_type != TransactionType::Outgoing,
                        txn.clone(),
                        txn_args.clone(),
                    )
                    .await
                {
                    Ok(ft_amounts) => ft_amounts,
                    Err(e) => bail!("Error getting ft amounts: {:?}", e),
                };

                let (ft_amount_out, ft_currency_out, ft_amount_in, ft_currency_in, to_account) =
                    ft_amounts
                        .as_ref()
                        .map(|ft_amounts| {
                            (
                                ft_amounts.ft_amount_out,
                                ft_amounts.ft_currency_out.clone(),
                                ft_amounts.ft_amount_in,
                                ft_amounts.ft_currency_in.clone(),
                                ft_amounts.to_account.clone(),
                            )
                        })
                        .unwrap_or((None, None, None, None, txn.r_receiver_account_id.clone()));

                let multiplier = if txn_type == TransactionType::Outgoing {
                    -1.0
                } else {
                    1.0
                };

                let mut onchain_balance = None;
                let mut onchain_balance_token = None;
                if include_balances {
                    if ft_amount_in.is_some() || ft_amount_out.is_some() {
                        debug!("Getting onchain balance for {}", for_account);
                        let ft_service = t2.ft_service.clone();
                        onchain_balance = Some(
                            ft_service
                                .assert_ft_balance(
                                    &txn.r_receiver_account_id,
                                    &for_account,
                                    txn.b_block_height
                                        .to_u64()
                                        .expect("Block height too large to fit in u128"),
                                )
                                .await?,
                        );
                        onchain_balance_token = Some(
                            ft_service
                                .assert_ft_metadata(&txn.r_receiver_account_id)
                                .await?
                                .symbol,
                        );
                    } else {
                        // It's a NEAR transfer
                        let near = t2
                            .ft_service
                            .get_near_balance(
                                &for_account,
                                txn.b_block_height
                                    .to_u64()
                                    .expect("Block height too large to fit in u64"),
                            )
                            .await?;
                        if let Some(near) = near {
                            onchain_balance = Some(near.0);
                            onchain_balance_token = Some("NEAR".to_string());
                        }
                    }
                }

                let data = metadata
                    .read()
                    .unwrap()
                    .metadata
                    .get(&for_account)
                    .and_then(|m| m.get(&txn.t_transaction_hash).cloned());

                Ok(Some(ReportRow {
                    account_id: for_account.clone(),
                    date: get_transaction_date(&txn),
                    method_name: get_method_name(&txn, &txn_args),
                    block_timestamp: txn.b_block_timestamp.to_u128().unwrap(),
                    from_account: txn.ara_receipt_predecessor_account_id.clone(),
                    block_height: txn.b_block_height.to_u128().unwrap(),
                    args: decode_transaction_args(&txn_args),
                    transaction_hash: txn.t_transaction_hash.clone(),
                    amount_transferred: get_near_transferred(&txn_args) * multiplier,
                    currency_transferred: "NEAR".to_string(),
                    ft_amount_out,
                    ft_currency_out,
                    ft_amount_in,
                    ft_currency_in,
                    to_account,
                    amount_staked: 0.0,
                    onchain_balance,
                    onchain_balance_token,
                    metadata: data,
                }))
            });
            rows_handle.push(row);
        }

        join_all(rows_handle)
            .await
            .iter()
            .for_each(|row| match row {
                Ok(r) => match r {
                    Ok(row) => {
                        if let Some(row) = row {
                            report.push(row.clone())
                        }
                    }
                    Err(err) => error!(?err, "Error getting row"),
                },
                Err(err) => error!(?err, "Error joining rows"),
            });

        Ok(report)
    }

    async fn get_ft_amounts(
        &self,
        is_incoming: bool,
        txn: Transaction,
        txn_args: TaArgs,
    ) -> Result<Option<FtAmounts>> {
        let method_name = txn_args
            .method_name
            .as_deref()
            .map(MethodName::from)
            .unwrap_or(MethodName::Unsupported);

        let function_call_args = decode_transaction_args(&txn_args);

        let res = match method_name {
            MethodName::FtTransfer => {
                let metadata = self.get_metadata(&txn.r_receiver_account_id).await?;

                let ft_transfer_args = serde_json::from_str::<FtTransfer>(&function_call_args)
                    .context(format!("Invalid ft_transfer args {:?}", function_call_args))?;
                let amount = safe_divide_u128(ft_transfer_args.amount.0, metadata.decimals as u32);
                if is_incoming {
                    Some(FtAmounts {
                        ft_amount_out: None,
                        ft_currency_out: None,
                        ft_amount_in: Some(amount),
                        ft_currency_in: Some(metadata.symbol),
                        from_account: txn.ara_receipt_predecessor_account_id.clone(),
                        to_account: ft_transfer_args.receiver_id.to_string(),
                    })
                } else {
                    Some(FtAmounts {
                        ft_amount_out: Some(amount),
                        ft_currency_out: Some(metadata.symbol),
                        ft_amount_in: None,
                        ft_currency_in: None,
                        from_account: txn.ara_receipt_predecessor_account_id.clone(),
                        to_account: ft_transfer_args.receiver_id.to_string(),
                    })
                }
            }
            MethodName::FtTransferCall => {
                let metadata = self.get_metadata(&txn.r_receiver_account_id).await?;
                let ft_transfer_args = serde_json::from_str::<FtTransferCall>(&function_call_args)
                    .context(format!("Invalid ft_transfer args {:?}", function_call_args))?;
                let amount = safe_divide_u128(ft_transfer_args.amount.0, metadata.decimals as u32);

                // No need to handle incoming. it comes as ft_transfer in case of swap.
                Some(FtAmounts {
                    ft_amount_out: Some(amount),
                    ft_currency_out: Some(metadata.symbol),
                    ft_amount_in: None,
                    ft_currency_in: None,
                    from_account: txn.ara_receipt_predecessor_account_id,
                    to_account: ft_transfer_args.receiver_id.to_string(),
                })
            }
            MethodName::Withdraw => {
                if txn.r_receiver_account_id.ends_with(".factory.bridge.near") {
                    let metadata = self.get_metadata(&txn.r_receiver_account_id).await?;
                    let withdraw_args =
                        serde_json::from_str::<WithdrawFromBridge>(&function_call_args)
                            .context(format!("Invalid withdraw args {:?}", function_call_args))?;
                    let amount = safe_divide_u128(withdraw_args.amount.0, metadata.decimals as u32);

                    Some(FtAmounts {
                        ft_amount_out: Some(amount),
                        ft_currency_out: Some(metadata.symbol),
                        ft_amount_in: None,
                        ft_currency_in: None,
                        from_account: txn.ara_receipt_predecessor_account_id.clone(),
                        to_account: txn.ara_receipt_predecessor_account_id.clone(),
                    })
                } else {
                    None
                }
            }
            MethodName::NearDeposit => {
                let metadata = self.get_metadata(&txn.r_receiver_account_id).await?;
                let deposit = get_near_transferred(&txn_args);
                Some(FtAmounts {
                    ft_amount_out: None,
                    ft_currency_out: None,
                    ft_amount_in: Some(deposit),
                    ft_currency_in: Some(metadata.symbol),
                    from_account: txn.ara_receipt_predecessor_account_id.clone(),
                    to_account: txn.ara_receipt_predecessor_account_id.clone(),
                })
            }
            MethodName::NearWithdraw => {
                let metadata = self.get_metadata(&txn.r_receiver_account_id).await?;
                let withdraw_args = serde_json::from_str::<WithdrawFromBridge>(&function_call_args)
                    .context(format!("Invalid withdraw args {:?}", function_call_args))?;
                let amount = safe_divide_u128(withdraw_args.amount.0, metadata.decimals as u32);

                Some(FtAmounts {
                    ft_amount_out: Some(amount),
                    ft_currency_out: Some(metadata.symbol),
                    ft_amount_in: None,
                    ft_currency_in: None,
                    from_account: txn.ara_receipt_predecessor_account_id.clone(),
                    to_account: txn.ara_receipt_predecessor_account_id.to_string(),
                })
            }
            MethodName::Mint => {
                let metadata = self.get_metadata(&txn.r_receiver_account_id).await?;

                let bridge_mint_args =
                    serde_json::from_str::<RainbowBridgeMint>(&function_call_args).context(
                        format!("Invalid bridge_mint_args args {:?}", function_call_args),
                    )?;
                let amount = safe_divide_u128(bridge_mint_args.amount.0, metadata.decimals as u32);
                if is_incoming {
                    Some(FtAmounts {
                        ft_amount_out: None,
                        ft_currency_out: None,
                        ft_amount_in: Some(amount),
                        ft_currency_in: Some(metadata.symbol),
                        from_account: txn.ara_receipt_predecessor_account_id.clone(),
                        to_account: bridge_mint_args.account_id.to_string(),
                    })
                } else {
                    error!("Minting should always comes from the bridge");
                    None
                }
            }
            MethodName::Unsupported => None,
        };

        Ok(res)
    }

    async fn get_metadata(&self, token_id: &String) -> Result<FtMetadata> {
        let ft_service = self.ft_service.clone();
        let metadata = match ft_service.assert_ft_metadata(token_id.as_str()).await {
            Ok(metadata) => metadata,
            Err(e) => bail!(
                "Failed to get ft_metadata for token_id: {:?}, err: {:?}",
                token_id,
                e
            ),
        };

        Ok(metadata)
    }
}

fn get_near_transferred(txn_args: &TaArgs) -> f64 {
    txn_args
        .deposit
        .as_ref()
        .map_or(Some(0.0), |deposit_str| {
            let deposit: u128 = match deposit_str.parse() {
                Ok(deposit) => deposit,
                Err(e) => panic!("Invalid deposit amount: {:?}, err: {:?}", deposit_str, e),
            };

            let nears = deposit / ONE_NEAR; // integer division
            let remainder = deposit % ONE_NEAR; // remainder

            // Convert the nears and remainder to f64
            let amount = nears as f64 + (remainder as f64 / ONE_NEAR as f64);

            // filter out small amounts
            (amount >= 0.0001).then_some(amount)
        })
        .unwrap_or(0.0)
}

pub fn safe_divide_u128(a: u128, decimals: u32) -> f64 {
    let divisor = 10u128.pow(decimals);
    (a / divisor) as f64 + (a % divisor) as f64 / divisor as f64
}

fn decode_args(txn: &Transaction) -> Result<TaArgs> {
    match serde_json::from_value::<TaArgs>(txn.clone().ara_args) {
        Ok(args) => Ok(args),
        Err(e) => bail!("Invalid args {:?}, err: {:?}", txn.ara_args, e),
    }
}

fn decode_transaction_args(txn_args: &TaArgs) -> String {
    match txn_args.args_base64.as_ref() {
        Some(base64_string) => general_purpose::STANDARD
            .decode(base64_string)
            .map(|decoded: Vec<u8>| {
                let mut args = String::new();
                for byte in decoded {
                    args.push(byte as char);
                }
                args
            })
            .unwrap_or_else(|_| String::new()),
        None => "{}".to_string(),
    }
}

fn get_method_name(txn: &Transaction, txn_args: &TaArgs) -> String {
    if txn.ara_action_kind != "FUNCTION_CALL" {
        txn.ara_action_kind.clone()
    } else {
        match &txn_args.method_name {
            Some(method_name) => method_name.clone(),
            None => {
                error!("No method name {:?}", txn_args);
                "".to_string()
            }
        }
    }
}

fn get_transaction_date(txn: &Transaction) -> String {
    let nanoseconds = txn
        .b_block_timestamp
        .to_u128()
        .expect("Timestamp too large to fit in u128");
    let seconds = (nanoseconds / 1_000_000_000) as i64;
    let date = NaiveDateTime::from_timestamp_opt(seconds, 0)
        .expect("Invalid timestamp")
        .date();
    date.format("%B %d, %Y").to_string()
}

fn assert_moves_token(row: ReportRow) -> Option<ReportRow> {
    if row.amount_transferred == 0.000000
        && row.ft_amount_out.is_none()
        && row.ft_amount_in.is_none()
        && row.amount_staked == 0.0
    {
        None
    } else {
        Some(row)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::DateTime;
    use near_jsonrpc_client::{JsonRpcClient, NEAR_MAINNET_ARCHIVAL_RPC_URL};
    use sqlx::postgres::PgPoolOptions;

    use super::*;

    async fn setup() -> Result<(SqlClient, FtService, TTA)> {
        let pool = PgPoolOptions::new()
            .max_connections(30)
            .connect(env!("DATABASE_URL"))
            .await?;

        let sql_client = SqlClient::new(pool);
        let near_client = JsonRpcClient::connect(NEAR_MAINNET_ARCHIVAL_RPC_URL);
        let ft_service = FtService::new(near_client);
        let semaphore = Arc::new(Semaphore::new(30));
        let tta_service = TTA::new(sql_client.clone(), ft_service.clone(), semaphore);

        Ok((sql_client, ft_service, tta_service))
    }

    #[tokio::test]
    async fn tta() -> Result<()> {
        let (_, _, tta_service) = setup().await?;

        let start_date = DateTime::parse_from_rfc3339("2022-01-01T00:00:00Z")
            .unwrap()
            .timestamp_nanos() as u128;
        let end_date = DateTime::parse_from_rfc3339("2022-02-01T00:00:00Z")
            .unwrap()
            .timestamp_nanos() as u128;
        let accounts: HashSet<String> = "nf-payments.near,nf-payments2.near"
            .split(',')
            .map(String::from)
            .collect();
        let include_balances = false;

        let mut accounts_metadata = HashMap::new();
        let mut account_txns = HashMap::new();

        account_txns.insert(
            "51VVGwLAFX6K62jB84E6qVHdF4GbhEMB2CoZJ9ZziiEt".to_string(),
            "unit test".to_string(),
        );

        accounts_metadata.insert("nf-payments.near".to_string(), account_txns);

        let metadata_struct = Arc::new(RwLock::new(TxnsReportWithMetadata {
            metadata: accounts_metadata,
        }));

        let res = tta_service
            .get_txns_report(
                start_date,
                end_date,
                accounts,
                include_balances,
                metadata_struct,
            )
            .await
            .unwrap();

        assert!(!res.is_empty());

        for row in res {
            if row.transaction_hash == "51VVGwLAFX6K62jB84E6qVHdF4GbhEMB2CoZJ9ZziiEt" {
                assert_eq!(row.metadata, Some("unit test".to_string()));
            } else {
                assert_eq!(row.metadata, None);
            }
        }
        Ok(())
    }
}
