use std::{
    cmp,
    collections::{self, HashSet},
    fs::File,
    io::Write,
    vec,
};

use anyhow::{bail, Result};

use csv::WriterBuilder;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::channel;
use tracing::{error, info, instrument};

use super::{sql_queries::SqlClient, Transaction, TtaError};

#[derive(Debug, Clone)]
pub struct TTA {
    sql_client: SqlClient,
}

impl TTA {
    pub fn new(sql_client: SqlClient) -> Self {
        Self { sql_client }
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
                    Ok(txns) => report.extend(txns),
                    Err(e) => {
                        info!(?e, "Got error");
                    }
                },
                Err(e) => {
                    info!(?e, "Got error");
                }
            }
        }

        let ended_at = chrono::Utc::now();

        info!("It took: {:?}", ended_at - started_at);
        info!("Got {} txns", report.len());

        let report_json = serde_json::to_string(&report[0..cmp::min(report.len(), 1000)])?;
        tokio::fs::write("report.json", report_json).await?;

        // After you've collected your transactions, open a file to write to
        let file = File::create("report.csv")?;

        // Create a CSV writer
        let mut writer = WriterBuilder::new().from_writer(file);

        for txn in report {
            // Assume txn is of type Transaction and has the necessary fields
            let record = [
                &txn.t_transaction_hash,
                &txn.t_signer_public_key,
                &txn.t_receiver_account_id,
            ];

            // Write record to CSV
            writer.write_record(record)?;
        }

        // Make sure to flush the writer to write any remaining bytes
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
    ) -> Result<Vec<Transaction>> {
        let mut txns: Vec<Transaction> = vec![];
        let (tx, mut rx) = channel(100);

        let t = self.clone();
        tokio::spawn(async move {
            t.sql_client
                .get_incoming_txns(accounts, start_date, end_date, tx)
                .await
                .unwrap();
        });

        while let Some(txn) = rx.recv().await {
            // info!("Got incoming txn: {:?}", txn);
            txns.push(txn)
        }

        Ok(txns)
    }

    // handle_ft+incoming_txns handles incoming fungible token transactions for the given accounts.
    async fn handle_ft_incoming_txns(
        self,
        accounts: HashSet<String>,
        start_date: u128,
        end_date: u128,
    ) -> Result<Vec<Transaction>> {
        let mut txns: Vec<Transaction> = vec![];
        let (tx, mut rx) = channel(100);

        let t = self.clone();
        tokio::spawn(async move {
            t.sql_client
                .get_ft_incoming_txns(accounts, start_date, end_date, tx)
                .await
                .unwrap();
        });

        while let Some(txn) = rx.recv().await {
            // info!("Got incoming txn: {:?}", txn);
            txns.push(txn)
        }

        Ok(txns)
    }

    async fn handle_outgoing_txns(
        self,
        accounts: HashSet<String>,
        start_date: u128,
        end_date: u128,
    ) -> Result<Vec<Transaction>> {
        let mut txns: Vec<Transaction> = vec![];
        let (tx, mut rx) = channel(100);

        let t = self.clone();
        tokio::spawn(async move {
            t.sql_client
                .get_outgoing_txns(accounts, start_date, end_date, tx)
                .await
                .unwrap();
        });

        while let Some(txn) = rx.recv().await {
            // info!("Got outgoing txn: {:?}", txn);
            txns.push(txn)
        }

        Ok(txns)
    }
}

pub fn get_associated_lockup(account_id: &str, master_account_id: &str) -> String {
    format!(
        "{}.lockup.{}",
        &sha256(account_id)[0..40],
        master_account_id
    )
}

fn sha256(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}
