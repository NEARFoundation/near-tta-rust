use std::{
    cmp,
    collections::{self, HashSet},
    fs::File,
    sync::Arc,
    vec,
};

use anyhow::{bail, Context, Result};
use near_jsonrpc_client::JsonRpcClient;

use crate::tta::utils::get_associated_lockup;
use base64::{engine::general_purpose, Engine as _};
use chrono::{NaiveDateTime, Utc};
use csv::WriterBuilder;
use num_traits::{cast::ToPrimitive, Float, Pow};
use tokio::sync::{mpsc::channel, Mutex};
use tracing::{debug, error, info, instrument};

use super::{
    ft_metadata::{FtMetadata, FtMetadataCache},
    models::{FtAmounts, MethodName, ReportRow},
    sql::{
        models::{FtTransfer, TaArgs, Transaction, WithdrawFromBridge},
        sql_queries::SqlClient,
    },
};

struct TokenAndMetadata {
    token_id: String,
    metadata: FtMetadata,
}

#[derive(Debug, Clone)]
pub struct TTA {
    sql_client: SqlClient,
    near_client: JsonRpcClient,
    ft_metadata_cache: Arc<Mutex<FtMetadataCache>>,
}

impl TTA {
    pub fn new(
        sql_client: SqlClient,
        near_client: JsonRpcClient,
        ft_metadata_cache: Arc<Mutex<FtMetadataCache>>,
    ) -> Self {
        Self {
            sql_client,
            near_client,
            ft_metadata_cache,
        }
    }

    #[instrument(skip(self, start_date, end_date, accounts))]
    pub(crate) async fn get_txns_report(
        &self,
        start_date: u128,
        end_date: u128,
        accounts: HashSet<String>,
    ) -> anyhow::Result<()> {
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
                let wallets_for_account = wallets_for_account.clone();
                let t = t.clone();
                let a = acc.clone();
                async move {
                    match t
                        .handle_incoming_txns(a, wallets_for_account, start_date, end_date)
                        .await
                    {
                        Ok(txns) => Ok(txns),
                        Err(e) => Err(e),
                    }
                }
            });

            let task_ft_incoming = tokio::spawn({
                let wallets_for_account = wallets_for_account.clone();
                let t = t.clone();
                let a = acc.clone();

                async move {
                    match t
                        .handle_ft_incoming_txns(a, wallets_for_account, start_date, end_date)
                        .await
                    {
                        Ok(txns) => Ok(txns),
                        Err(e) => Err(e),
                    }
                }
            });

            let task_outgoing = tokio::spawn({
                let wallets_for_account = wallets_for_account.clone();
                let t = t.clone();
                let a = acc.clone();

                async move {
                    match t
                        .handle_outgoing_txns(a, wallets_for_account, start_date, end_date)
                        .await
                    {
                        Ok(txns) => Ok(txns),
                        Err(e) => Err(e),
                    }
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
                        // Aply filtering
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

        let file = File::create("report.csv")?;
        let mut writer = WriterBuilder::new().from_writer(file);

        writer.write_record(ReportRow::get_vec_headers())?;
        for record in report {
            writer.write_record(record.to_vec())?;
        }

        writer.flush()?;

        info!("Done");

        Ok(())
    }

    // handle_incoming_txns handles incoming transactions to the given accounts.
    async fn handle_incoming_txns(
        self,
        for_account: String,
        accounts: HashSet<String>,
        start_date: u128,
        end_date: u128,
    ) -> Result<Vec<ReportRow>> {
        let mut report = vec![];
        let (tx, mut rx) = channel(100);

        let t = self.clone();
        tokio::spawn({
            let a = accounts.clone();
            async move {
                t.sql_client
                    .get_incoming_txns(a, start_date, end_date, tx)
                    .await
                    .unwrap();
            }
        });

        while let Some(txn) = rx.recv().await {
            if txn.ara_action_kind != "FUNCTION_CALL" && txn.ara_action_kind != "TRANSFER" {
                continue;
            }

            let txn_args = decode_args(txn.clone())?;
            let ft_amounts = self
                .get_ft_amounts(true, txn.clone(), txn_args.clone())
                .await?;

            let (ft_amount_out, ft_currency_out, ft_amount_in, ft_currency_in, to_account) =
                ft_amounts
                    .as_ref()
                    .map(|ft_amounts| {
                        (
                            ft_amounts.ft_amount_out.clone(),
                            ft_amounts.ft_currency_out.clone(),
                            ft_amounts.ft_amount_in.clone(),
                            ft_amounts.ft_currency_in.clone(),
                            ft_amounts.to_account.clone(),
                        )
                    })
                    .unwrap_or((None, None, None, None, txn.r_receiver_account_id.clone()));

            let row = ReportRow {
                account_id: for_account.clone(),
                date: get_transaction_date(&txn),
                method_name: get_method_name(&txn, &txn_args),
                block_timestamp: txn.b_block_timestamp.to_u128().unwrap(),
                from_account: txn.ara_receipt_predecessor_account_id.clone(),
                block_height: txn.b_block_height.to_u128().unwrap(),
                args: decode_transaction_args(&txn_args),
                transaction_hash: txn.t_transaction_hash.clone(),
                amount_transferred: get_near_transferred(&txn_args),
                currency_transferred: "NEAR".to_string(),
                ft_amount_out,
                ft_currency_out,
                ft_amount_in,
                ft_currency_in,
                to_account,
                amount_staked: 0.0,
                onchain_usdc_balance: 0.0,
                onchain_usdt_balance: 0.0,
            };
            report.push(row)
        }
        Ok(report)
    }

    // handle_ft+incoming_txns handles incoming fungible token transactions for the given accounts.
    async fn handle_ft_incoming_txns(
        self,
        for_account: String,
        accounts: HashSet<String>,
        start_date: u128,
        end_date: u128,
    ) -> Result<Vec<ReportRow>> {
        let mut report = vec![];
        let (tx, mut rx) = channel(100);

        let t = self.clone();
        tokio::spawn({
            let a = accounts.clone();
            async move {
                t.sql_client
                    .get_ft_incoming_txns(a, start_date, end_date, tx)
                    .await
                    .unwrap();
            }
        });

        while let Some(txn) = rx.recv().await {
            info!(?txn.ara_action_kind, "Got txn");

            if txn.ara_action_kind != "FUNCTION_CALL" && txn.ara_action_kind != "TRANSFER" {
                continue;
            }

            let txn_args = decode_args(txn.clone())?;
            let ft_amounts = self
                .get_ft_amounts(true, txn.clone(), txn_args.clone())
                .await?;

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

            let row = ReportRow {
                account_id: for_account.clone(),
                date: get_transaction_date(&txn),
                method_name: get_method_name(&txn, &txn_args),
                block_timestamp: txn.b_block_timestamp.to_u128().unwrap(),
                from_account: txn.ara_receipt_predecessor_account_id.clone(),
                block_height: txn.b_block_height.to_u128().unwrap(),
                args: decode_transaction_args(&txn_args),
                transaction_hash: txn.t_transaction_hash.clone(),
                amount_transferred: get_near_transferred(&txn_args),
                currency_transferred: "NEAR".to_string(),
                ft_amount_out,
                ft_currency_out,
                ft_amount_in,
                ft_currency_in,
                to_account,
                amount_staked: 0.0,
                onchain_usdc_balance: 0.0,
                onchain_usdt_balance: 0.0,
            };

            report.push(row)
        }
        Ok(report)
    }

    async fn handle_outgoing_txns(
        self,
        for_account: String,
        accounts: HashSet<String>,
        start_date: u128,
        end_date: u128,
    ) -> Result<Vec<ReportRow>> {
        let mut report = vec![];
        let (tx, mut rx) = channel(100);

        let t = self.clone();
        tokio::spawn({
            let a = accounts.clone();
            async move {
                t.sql_client
                    .get_outgoing_txns(a, start_date, end_date, tx)
                    .await
                    .unwrap();
            }
        });

        while let Some(txn) = rx.recv().await {
            if txn.ara_action_kind != "FUNCTION_CALL" && txn.ara_action_kind != "TRANSFER" {
                continue;
            }

            let txn_args = decode_args(txn.clone())?;
            let ft_amounts = self
                .get_ft_amounts(false, txn.clone(), txn_args.clone())
                .await?;

            let (ft_amount_out, ft_currency_out, ft_amount_in, ft_currency_in, to_account) =
                ft_amounts
                    .as_ref()
                    .map(|ft_amounts| {
                        (
                            ft_amounts.ft_amount_out.clone(),
                            ft_amounts.ft_currency_out.clone(),
                            ft_amounts.ft_amount_in.clone(),
                            ft_amounts.ft_currency_in.clone(),
                            ft_amounts.to_account.clone(),
                        )
                    })
                    .unwrap_or((None, None, None, None, txn.r_receiver_account_id.clone()));

            let row = ReportRow {
                account_id: for_account.clone(),
                date: get_transaction_date(&txn),
                method_name: get_method_name(&txn, &txn_args),
                block_timestamp: txn.b_block_timestamp.to_u128().unwrap(),
                from_account: txn.ara_receipt_predecessor_account_id.clone(),
                block_height: txn.b_block_height.to_u128().unwrap(),
                args: decode_transaction_args(&txn_args),
                transaction_hash: txn.t_transaction_hash.clone(),
                amount_transferred: get_near_transferred(&txn_args) * -1.0,
                currency_transferred: "NEAR".to_string(),
                ft_amount_out,
                ft_currency_out,
                ft_amount_in,
                ft_currency_in,
                to_account,
                amount_staked: 0.0,
                onchain_usdc_balance: 0.0,
                onchain_usdt_balance: 0.0,
            };

            report.push(row)
        }
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
                let token_n_metadata = self.get_token_and_metadata(&txn).await?;

                let ft_transfer_args = serde_json::from_str::<FtTransfer>(&function_call_args)
                    .context(format!("Invalid ft_transfer args {:?}", function_call_args))?;
                let amount = safe_divide_u128(
                    ft_transfer_args.amount.0,
                    token_n_metadata.metadata.decimals as u32,
                );
                if is_incoming {
                    Some(FtAmounts {
                        ft_amount_out: None,
                        ft_currency_out: None,
                        ft_amount_in: Some(amount),
                        ft_currency_in: Some(token_n_metadata.metadata.symbol),
                        from_account: txn.ara_receipt_predecessor_account_id.clone(),
                        to_account: ft_transfer_args.receiver_id.to_string(),
                    })
                } else {
                    Some(FtAmounts {
                        ft_amount_out: Some(amount),
                        ft_currency_out: Some(token_n_metadata.metadata.symbol),
                        ft_amount_in: None,
                        ft_currency_in: None,
                        from_account: txn.ara_receipt_predecessor_account_id.clone(),
                        to_account: ft_transfer_args.receiver_id.to_string(),
                    })
                }
            }
            MethodName::FtTransferCall => {
                // Handle "ft_transfer_call" here...
                // todo!("ft_transfer_call")
                None
            }
            MethodName::Swap => {
                // Handle "swap" here...
                // todo!("swap")
                None
            }
            MethodName::Withdraw => {
                if txn.r_receiver_account_id.ends_with(".factory.bridge.near") {
                    let token_n_metadata = self.get_token_and_metadata(&txn).await?;
                    let withdraw_args =
                        serde_json::from_str::<WithdrawFromBridge>(&function_call_args)
                            .context(format!("Invalid withdraw args {:?}", function_call_args))?;
                    let amount = safe_divide_u128(
                        withdraw_args.amount.0,
                        token_n_metadata.metadata.decimals as u32,
                    );

                    Some(FtAmounts {
                        ft_amount_out: Some(amount),
                        ft_currency_out: Some(token_n_metadata.metadata.symbol),
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
                let token_n_metadata = self.get_token_and_metadata(&txn).await?;
                let deposit = get_near_transferred(&txn_args);
                Some(FtAmounts {
                    ft_amount_out: None,
                    ft_currency_out: None,
                    ft_amount_in: Some(deposit),
                    ft_currency_in: Some(token_n_metadata.metadata.symbol.clone()),
                    from_account: txn.ara_receipt_predecessor_account_id.clone(),
                    to_account: txn.ara_receipt_predecessor_account_id.clone(),
                })
            }
            MethodName::NearWithdraw => {
                // -wnear -> +near. near might come with a transfer call, only minus wnear
                // needs further research
                // todo!("near_withdraw")
                None
            }
            MethodName::Unsupported => {
                // info!(
                //     "Unsupported method_name: {:?} and receiver: {:?}",
                //     method_name, txn.r_receiver_account_id
                // );
                None
            }
        };

        Ok(res)
    }

    async fn get_token_and_metadata(&self, txn: &Transaction) -> Result<TokenAndMetadata> {
        let token_id = txn.r_receiver_account_id.clone();

        let ft_metadata_cache = self.ft_metadata_cache.clone();
        let mut w = ft_metadata_cache.lock().await;
        let metadata = match w.assert_ft_metadata(token_id.as_str()).await {
            Ok(metadata) => metadata,
            Err(e) => bail!(
                "Failed to get ft_metadata for token_id: {:?}, txn: {:?}, err: {:?}",
                token_id,
                txn,
                e
            ),
        };

        Ok(TokenAndMetadata {
            token_id,
            metadata: metadata.clone(),
        })
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

            // Divide by 10^24 safely
            let amount = if deposit >= 10u128.pow(24) {
                deposit as f64 / 10f64.powi(24)
            } else {
                0.0
            };

            // filter out small amounts
            (amount >= 0.0001).then_some(amount)
        })
        .unwrap_or(0.0)
}

fn safe_divide_u128(a: u128, decimals: u32) -> f64 {
    let divisor = 10u128.pow(decimals);
    (a / divisor) as f64 + (a % divisor) as f64 / divisor as f64
}

fn decode_args(txn: Transaction) -> Result<TaArgs> {
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
    if row.amount_transferred == 0.0
        && row.ft_amount_out.is_none()
        && row.ft_amount_in.is_none()
        && row.amount_staked == 0.0
    {
        None
    } else {
        Some(row)
    }
}
