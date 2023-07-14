use std::{
    cmp,
    collections::{self, HashSet},
    fs::File,
    sync::Arc,
    vec,
};

use anyhow::{bail, Result};
use near_jsonrpc_client::JsonRpcClient;

use crate::tta::utils::get_associated_lockup;
use base64::{engine::general_purpose, Engine as _};
use chrono::NaiveDateTime;
use csv::WriterBuilder;
use num_traits::cast::ToPrimitive;
use tokio::sync::{mpsc::channel, Mutex};
use tracing::{debug, error, info, instrument};

use super::{
    ft_metadata::FtMetadataCache,
    models::ReportRow,
    sql::{
        models::{FtTransfer, TaArgs, Transaction},
        sql_queries::SqlClient,
    },
};

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
        let started_at = chrono::Utc::now();

        for acc in accounts {
            let t = self.clone();
            let mut wallets_for_account = collections::HashSet::new();

            let lockup = get_associated_lockup(&acc, "near");
            info!(?acc, ?lockup, "Got lockup");
            wallets_for_account.insert(acc);
            wallets_for_account.insert(lockup);

            let w_2 = wallets_for_account.clone();
            let tta_2 = t.clone();

            let w_3 = wallets_for_account.clone();
            let tta_3 = t.clone();

            let task_incoming = tokio::spawn(async move {
                match t
                    .handle_incoming_txns(wallets_for_account, start_date, end_date)
                    .await
                {
                    Ok(txns) => Ok(txns),
                    Err(e) => Err(e),
                }
            });
            let task_ft_incoming = tokio::spawn(async move {
                match tta_2
                    .handle_ft_incoming_txns(w_2, start_date, end_date)
                    .await
                {
                    Ok(txns) => Ok(txns),
                    Err(e) => Err(e),
                }
            });
            let task_outgoing = tokio::spawn(async move {
                match tta_3.handle_outgoing_txns(w_3, start_date, end_date).await {
                    Ok(txns) => Ok(txns),
                    Err(e) => Err(e),
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
                    Ok(partial_report) => report.extend(partial_report),
                    Err(e) => {
                        error!(?e, "Got error");
                    }
                },
                Err(e) => {
                    error!(?e, "Got error");
                }
            }
        }

        let ended_at = chrono::Utc::now();

        info!("It took: {:?}", ended_at - started_at);
        info!("Got {} txns", report.len());

        let file = File::create("report.csv")?;
        let mut writer = WriterBuilder::new().from_writer(file);

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
        accounts: HashSet<String>,
        start_date: u128,
        end_date: u128,
    ) -> Result<Vec<ReportRow>> {
        let mut report = vec![];
        let (tx, mut rx) = channel(100);

        let t = self.clone();
        tokio::spawn(async move {
            t.sql_client
                .get_incoming_txns(accounts, start_date, end_date, tx)
                .await
                .unwrap();
        });

        while let Some(txn) = rx.recv().await {
            let txn_args = decode_args(txn.clone());
            let (ft_amount_in, ft_amount_out, ft_currency_in, ft_currency_out) =
                self.get_ft_amounts(txn.clone(), txn_args.clone()).await?;
            let row = ReportRow {
                account_id: "test".to_string(),
                date: get_transaction_date(txn.clone()),
                method_name: get_method_name(txn.clone(), txn_args.clone()),
                block_timestamp: txn.b_block_timestamp.to_u128().unwrap(),
                from_account: txn.ara_receipt_predecessor_account_id.clone(),
                block_height: txn.b_block_height.to_u128().unwrap(),
                args: decode_transaction_args(txn_args.clone()),
                transaction_hash: txn.t_transaction_hash.clone(),
                amount_transferred: get_near_transferred(txn_args.clone()) * -1.0,
                currency_transferred: "NEAR".to_string(),
                ft_amount_out,
                ft_currency_out,
                ft_amount_in,
                ft_currency_in,
                to_account: txn.t_receiver_account_id.clone(),
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
        accounts: HashSet<String>,
        start_date: u128,
        end_date: u128,
    ) -> Result<Vec<ReportRow>> {
        let mut report = vec![];
        let (tx, mut rx) = channel(100);

        let t = self.clone();
        tokio::spawn(async move {
            t.sql_client
                .get_ft_incoming_txns(accounts, start_date, end_date, tx)
                .await
                .unwrap();
        });

        while let Some(txn) = rx.recv().await {
            let txn_args = decode_args(txn.clone());
            let (ft_amount_in, ft_amount_out, ft_currency_in, ft_currency_out) =
                self.get_ft_amounts(txn.clone(), txn_args.clone()).await?;
            let row = ReportRow {
                account_id: "test".to_string(),
                date: get_transaction_date(txn.clone()),
                method_name: get_method_name(txn.clone(), txn_args.clone()),
                block_timestamp: txn.b_block_timestamp.to_u128().unwrap(),
                from_account: txn.ara_receipt_predecessor_account_id.clone(),
                block_height: txn.b_block_height.to_u128().unwrap(),
                args: decode_transaction_args(txn_args.clone()),
                transaction_hash: txn.t_transaction_hash.clone(),
                amount_transferred: get_near_transferred(txn_args.clone()) * -1.0,
                currency_transferred: "NEAR".to_string(),
                ft_amount_out,
                ft_currency_out,
                ft_amount_in,
                ft_currency_in,
                to_account: txn.t_receiver_account_id.clone(),
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
        accounts: HashSet<String>,
        start_date: u128,
        end_date: u128,
    ) -> Result<Vec<ReportRow>> {
        let mut report = vec![];
        let (tx, mut rx) = channel(100);

        let t = self.clone();
        tokio::spawn(async move {
            t.sql_client
                .get_outgoing_txns(accounts, start_date, end_date, tx)
                .await
                .unwrap();
        });

        while let Some(txn) = rx.recv().await {
            debug!("handle_outgoing_txns - Got txn: {:?}", txn);
            let txn_args = decode_args(txn.clone());
            let (ft_amount_in, ft_amount_out, ft_currency_in, ft_currency_out) =
                self.get_ft_amounts(txn.clone(), txn_args.clone()).await?;
            let row = ReportRow {
                account_id: "test".to_string(),
                date: get_transaction_date(txn.clone()),
                method_name: get_method_name(txn.clone(), txn_args.clone()),
                block_timestamp: txn.b_block_timestamp.to_u128().unwrap(),
                from_account: txn.ara_receipt_predecessor_account_id.clone(),
                block_height: txn.b_block_height.to_u128().unwrap(),
                args: decode_transaction_args(txn_args.clone()),
                transaction_hash: txn.t_transaction_hash.clone(),
                amount_transferred: get_near_transferred(txn_args.clone()) * -1.0,
                currency_transferred: "NEAR".to_string(),
                ft_amount_out,
                ft_currency_out,
                ft_amount_in,
                ft_currency_in,
                to_account: txn.t_receiver_account_id.clone(),
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
        txn: Transaction,
        txn_args: TaArgs,
    ) -> Result<(f64, f64, String, String)> {
        let method_name = match txn_args.clone().method_name {
            Some(method_name) => method_name,
            None => "".to_string(),
        };

        let token_id = txn.r_receiver_account_id;
        let mut ft_amount_in = 0.0;
        let mut ft_amount_out = 0.0;
        let mut ft_currency_in = "".to_string();
        let mut ft_currency_out = "".to_string();

        if method_name == "ft_transfer" {
            let ft_tranfer_args = decode_transaction_args(txn_args.clone());
            let args = serde_json::from_str::<FtTransfer>(&ft_tranfer_args);
            let args = match args {
                Ok(args) => args,
                Err(e) => bail!(
                    "Invalid ft_transfer args {:?}, err: {:?}",
                    ft_tranfer_args,
                    e
                ),
            };

            let ft_metadata_cache = self.ft_metadata_cache.clone();
            let mut w = ft_metadata_cache.lock().await;
            let e = w.assert_ft_metadata(token_id.as_str()).await?;

            // if args.receiver_id.to_string() == token_id {
            //     ft_amount_in = args.amount.0;
            //     ft_currency_in = args.token_id;
            // } else {
            //     ft_amount_out = args.amount;
            //     ft_currency_out = args.token_id;
            // }
        }

        Ok((ft_amount_in, ft_amount_out, ft_currency_in, ft_currency_out))
    }
}

fn get_near_transferred(txn_args: TaArgs) -> f64 {
    txn_args
        .deposit
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

fn decode_args(txn: Transaction) -> TaArgs {
    serde_json::from_value(txn.ara_args).expect("Invalid args")
}

fn decode_transaction_args(txn_args: TaArgs) -> String {
    if txn_args.args_base64.is_none() {
        return "{}".to_string();
    }

    general_purpose::STANDARD
        .decode(txn_args.args_base64.unwrap())
        .map(|decoded: Vec<u8>| {
            let mut args = String::new();
            for byte in decoded {
                args.push(byte as char);
            }
            args
        })
        .unwrap_or("".to_string())
}

fn get_method_name(txn: Transaction, txn_args: TaArgs) -> String {
    let t = txn;
    if t.ara_action_kind != "FUNCTION_CALL" {
        t.ara_action_kind
    } else if txn_args.method_name.is_none() {
        error!("No method name {:?}", txn_args);
        "".to_string()
    } else {
        txn_args.method_name.unwrap()
    }
}

fn get_transaction_date(txn: Transaction) -> String {
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
