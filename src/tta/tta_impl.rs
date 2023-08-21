use std::{collections::HashSet, sync::Arc, vec};

use anyhow::{bail, Context, Result};

use near_sdk::ONE_NEAR;

use crate::tta::utils::get_associated_lockup;
use base64::{engine::general_purpose, Engine as _};
use chrono::{NaiveDateTime, Utc};

use num_traits::cast::ToPrimitive;
use tokio::sync::{
    mpsc::{channel, Sender},
    Mutex, Semaphore,
};

use tracing::{error, info, instrument, warn};

use super::{
    ft_metadata::{FtMetadata, FtMetadataCache},
    models::{
        FtAmounts, FtTransfer, FtTransferCall, MethodName, RainbowBridgeMint, ReportRow, Swap,
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
    ft_metadata_cache: Arc<Mutex<FtMetadataCache>>,
    semaphore: Arc<Semaphore>,
}

impl TTA {
    pub fn new(
        sql_client: SqlClient,
        ft_metadata_cache: Arc<Mutex<FtMetadataCache>>,
        semaphore: Arc<Semaphore>,
    ) -> Self {
        Self {
            sql_client,
            ft_metadata_cache,
            semaphore,
        }
    }

    #[instrument(skip(self, start_date, end_date, accounts))]
    pub(crate) async fn get_txns_report(
        &self,
        start_date: u128,
        end_date: u128,
        accounts: HashSet<String>,
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
                async move {
                    let _s = s;
                    t.handle_txns(
                        TransactionType::Incoming,
                        for_account,
                        wallets_for_account,
                        start_date,
                        end_date,
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
                async move {
                    let _s = s;
                    t.handle_txns(
                        TransactionType::FtIncoming,
                        for_account,
                        wallets_for_account,
                        start_date,
                        end_date,
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
                async move {
                    let _s = s;

                    t.handle_txns(
                        TransactionType::Outgoing,
                        a,
                        wallets_for_account,
                        start_date,
                        end_date,
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

        Ok(report)
    }

    async fn handle_txns(
        self,
        txn_type: TransactionType,
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
                txn_type
                    .get_transaction(&t.sql_client, a, start_date, end_date, tx)
                    .await
                    .unwrap();
            }
        });

        while let Some(txn) = rx.recv().await {
            if txn.ara_action_kind != "FUNCTION_CALL" && txn.ara_action_kind != "TRANSFER" {
                continue;
            }

            let txn_args = decode_args(&txn)?;

            // Skipping gas refunds
            if get_near_transferred(&txn_args) < 0.5
                && txn.ara_receipt_predecessor_account_id == "system"
            {
                continue;
            }

            let ft_amounts = match self
                .get_ft_amounts(
                    txn_type != TransactionType::Outgoing,
                    txn.clone(),
                    txn_args.clone(),
                )
                .await
            {
                Ok(ft_amounts) => ft_amounts,
                Err(e) => {
                    error!(?e, "Error getting ft amounts");
                    continue;
                }
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

            let multiplier = if accounts.contains(txn.r_predecessor_account_id.as_str()) {
                -1.0
            } else {
                1.0
            };

            let row = ReportRow {
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
            MethodName::Swap => {
                let withdraw_args = serde_json::from_str::<Swap>(&function_call_args)
                    .context(format!("Invalid withdraw args {:?}", function_call_args))?;

                let metadata_out = self.get_metadata(&withdraw_args.token_out).await?;
                let metadata_in = self.get_metadata(&withdraw_args.token_in).await?;

                let amount_out =
                    safe_divide_u128(withdraw_args.min_amount_out.0, metadata_in.decimals as u32);
                let amount_in =
                    safe_divide_u128(withdraw_args.amount_in.0, metadata_in.decimals as u32);

                Some(FtAmounts {
                    ft_amount_out: Some(amount_out),
                    ft_currency_out: Some(metadata_out.symbol),
                    ft_amount_in: Some(amount_in),
                    ft_currency_in: Some(metadata_in.symbol),
                    from_account: txn.ara_receipt_predecessor_account_id.clone(),
                    to_account: txn.ara_receipt_predecessor_account_id.clone(),
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
        let ft_metadata_cache = self.ft_metadata_cache.clone();
        let mut w = ft_metadata_cache.lock().await;
        let metadata = match w.assert_ft_metadata(token_id.as_str()).await {
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

fn safe_divide_u128(a: u128, decimals: u32) -> f64 {
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
