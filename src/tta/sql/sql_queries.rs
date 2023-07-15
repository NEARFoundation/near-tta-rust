use std::collections::{self};

use anyhow::Result;
use sqlx::{types::Decimal, Pool, Postgres};
use tokio::sync::mpsc::Sender;
use tokio_stream::StreamExt;
use tracing::{error, info, instrument};

use super::models::Transaction;

#[derive(Debug, Clone)]
pub struct SqlClient {
    pool: Pool<Postgres>,
}

impl SqlClient {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    #[instrument(skip(self, sender_txn))]
    pub async fn get_outgoing_txns(
        &self,
        accounts: collections::HashSet<String>,
        start_date: u128,
        end_date: u128,
        sender_txn: Sender<Transaction>,
    ) -> Result<()> {
        let accs: Vec<String> = accounts.into_iter().collect();
        let start_date_decimal = Decimal::from(start_date);
        let end_date_decimal = Decimal::from(end_date);

        let mut stream_txs = sqlx::query_as!(
            Transaction,
            r##"SELECT
                T.TRANSACTION_HASH as T_TRANSACTION_HASH,
                T.INCLUDED_IN_BLOCK_HASH as T_INCLUDED_IN_BLOCK_HASH,
                T.INCLUDED_IN_CHUNK_HASH as T_INCLUDED_IN_CHUNK_HASH,
                T.INDEX_IN_CHUNK as T_INDEX_IN_CHUNK,
                T.BLOCK_TIMESTAMP as T_BLOCK_TIMESTAMP,
                T.SIGNER_ACCOUNT_ID as T_SIGNER_ACCOUNT_ID,
                T.SIGNER_PUBLIC_KEY as T_SIGNER_PUBLIC_KEY,
                T.NONCE as T_NONCE,
                T.RECEIVER_ACCOUNT_ID as T_RECEIVER_ACCOUNT_ID,
                T.SIGNATURE as T_SIGNATURE,
                T.STATUS as "t_status: String",
                T.CONVERTED_INTO_RECEIPT_ID as T_CONVERTED_INTO_RECEIPT_ID,
                T.RECEIPT_CONVERSION_GAS_BURNT as T_RECEIPT_CONVERSION_GAS_BURNT,
                T.RECEIPT_CONVERSION_TOKENS_BURNT as T_RECEIPT_CONVERSION_TOKENS_BURNT,
                R.RECEIPT_ID as R_RECEIPT_ID,
                R.INCLUDED_IN_BLOCK_HASH as R_INCLUDED_IN_BLOCK_HASH,
                R.INCLUDED_IN_CHUNK_HASH as R_INCLUDED_IN_CHUNK_HASH,
                R.INDEX_IN_CHUNK as R_INDEX_IN_CHUNK,
                R.INCLUDED_IN_BLOCK_TIMESTAMP as R_INCLUDED_IN_BLOCK_TIMESTAMP,
                R.PREDECESSOR_ACCOUNT_ID as R_PREDECESSOR_ACCOUNT_ID,
                R.RECEIVER_ACCOUNT_ID as R_RECEIVER_ACCOUNT_ID,
                R.RECEIPT_KIND as "r_receipt_kind: String",
                R.ORIGINATED_FROM_TRANSACTION_HASH as R_ORIGINATED_FROM_TRANSACTION_HASH,
                ARA.RECEIPT_ID as ARA_RECEIPT_ID,
                ARA.INDEX_IN_ACTION_RECEIPT as ARA_INDEX_IN_ACTION_RECEIPT,
                ARA.ARGS as ARA_ARGS,
                ARA.RECEIPT_PREDECESSOR_ACCOUNT_ID as ARA_RECEIPT_PREDECESSOR_ACCOUNT_ID,
                ARA.RECEIPT_RECEIVER_ACCOUNT_ID as ARA_RECEIPT_RECEIVER_ACCOUNT_ID,
                ARA.RECEIPT_INCLUDED_IN_BLOCK_TIMESTAMP as ARA_RECEIPT_INCLUDED_IN_BLOCK_TIMESTAMP,
                ARA.ACTION_KIND as "ara_action_kind: String",
                B.BLOCK_HEIGHT as B_BLOCK_HEIGHT,
                B.BLOCK_HASH as B_BLOCK_HASH,
                B.PREV_BLOCK_HASH as B_PREV_BLOCK_HASH,
                B.BLOCK_TIMESTAMP as B_BLOCK_TIMESTAMP,
                B.GAS_PRICE as B_GAS_PRICE,
                B.AUTHOR_ACCOUNT_ID as B_AUTHOR_ACCOUNT_ID,
                EO.RECEIPT_ID as EO_RECEIPT_ID,
                EO.EXECUTED_IN_BLOCK_HASH  as EO_EXECUTED_IN_BLOCK_HASH ,
                EO.EXECUTED_IN_BLOCK_TIMESTAMP as EO_EXECUTED_IN_BLOCK_TIMESTAMP,
                EO.INDEX_IN_CHUNK as EO_INDEX_IN_CHUNK,
                EO.GAS_BURNT as EO_GAS_BURNT,
                EO.TOKENS_BURNT as EO_TOKENS_BURNT,
                EO.EXECUTOR_ACCOUNT_ID as EO_EXECUTOR_ACCOUNT_ID,
                EO.SHARD_ID as EO_SHARD_ID,
                EO.STATUS as "eo_status: String"
            FROM
                TRANSACTIONS T
                LEFT JOIN RECEIPTS R ON (T.CONVERTED_INTO_RECEIPT_ID = R.RECEIPT_ID
                        OR t.TRANSACTION_HASH = R.ORIGINATED_FROM_TRANSACTION_HASH)
                LEFT JOIN ACTION_RECEIPT_ACTIONS ARA ON ARA.RECEIPT_ID = R.RECEIPT_ID
                LEFT JOIN BLOCKS B ON B.BLOCK_HASH = R.INCLUDED_IN_BLOCK_HASH
                LEFT JOIN EXECUTION_OUTCOMES EO ON EO.RECEIPT_ID = R.RECEIPT_ID
            WHERE
                receipt_predecessor_account_id = ANY($1)
                AND EO.STATUS IN ('SUCCESS_RECEIPT_ID', 'SUCCESS_VALUE')
                and B.BLOCK_TIMESTAMP >= $2
                and B.BLOCK_TIMESTAMP < $3  
                AND NOT EXISTS (
                    SELECT 1
                    FROM RECEIPTS R2
                    JOIN EXECUTION_OUTCOMES EO2 ON EO2.RECEIPT_ID = R2.RECEIPT_ID
                    WHERE (T.CONVERTED_INTO_RECEIPT_ID = R2.RECEIPT_ID OR T.TRANSACTION_HASH = R2.ORIGINATED_FROM_TRANSACTION_HASH)
                    AND EO2.STATUS = 'FAILURE'
                );
            "##,
            &accs,
            &start_date_decimal,
            &end_date_decimal,
        )
        .fetch(&self.pool);

        let start = chrono::Utc::now();

        while let Some(txn) = stream_txs.next().await {
            match txn {
                Ok(txn) => {
                    if let Err(e) = sender_txn.send(txn).await {
                        error!("Error sending transaction: {}", e);
                    };
                }
                Err(e) => error!("Error getting transaction: {}", e),
            }
        }

        let end = chrono::Utc::now();
        info!(
            "Time taken to get outgoing transactions: {:?} for {:?}",
            end - start,
            accs
        );

        Ok(())
    }

    #[instrument(skip(self, sender_txn))]
    pub async fn get_incoming_txns(
        &self,
        accounts: collections::HashSet<String>,
        start_date: u128,
        end_date: u128,
        sender_txn: Sender<Transaction>,
    ) -> Result<()> {
        let accs: Vec<String> = accounts.into_iter().collect();
        let start_date_decimal = Decimal::from(start_date);
        let end_date_decimal = Decimal::from(end_date);

        let mut stream_txs = sqlx::query_as!(
            Transaction,
            r##"
            SELECT
                T.TRANSACTION_HASH as T_TRANSACTION_HASH,
                T.INCLUDED_IN_BLOCK_HASH as T_INCLUDED_IN_BLOCK_HASH,
                T.INCLUDED_IN_CHUNK_HASH as T_INCLUDED_IN_CHUNK_HASH,
                T.INDEX_IN_CHUNK as T_INDEX_IN_CHUNK,
                T.BLOCK_TIMESTAMP as T_BLOCK_TIMESTAMP,
                T.SIGNER_ACCOUNT_ID as T_SIGNER_ACCOUNT_ID,
                T.SIGNER_PUBLIC_KEY as T_SIGNER_PUBLIC_KEY,
                T.NONCE as T_NONCE,
                T.RECEIVER_ACCOUNT_ID as T_RECEIVER_ACCOUNT_ID,
                T.SIGNATURE as T_SIGNATURE,
                T.STATUS as "t_status: String",
                T.CONVERTED_INTO_RECEIPT_ID as T_CONVERTED_INTO_RECEIPT_ID,
                T.RECEIPT_CONVERSION_GAS_BURNT as T_RECEIPT_CONVERSION_GAS_BURNT,
                T.RECEIPT_CONVERSION_TOKENS_BURNT as T_RECEIPT_CONVERSION_TOKENS_BURNT,
                R.RECEIPT_ID as R_RECEIPT_ID,
                R.INCLUDED_IN_BLOCK_HASH as R_INCLUDED_IN_BLOCK_HASH,
                R.INCLUDED_IN_CHUNK_HASH as R_INCLUDED_IN_CHUNK_HASH,
                R.INDEX_IN_CHUNK as R_INDEX_IN_CHUNK,
                R.INCLUDED_IN_BLOCK_TIMESTAMP as R_INCLUDED_IN_BLOCK_TIMESTAMP,
                R.PREDECESSOR_ACCOUNT_ID as R_PREDECESSOR_ACCOUNT_ID,
                R.RECEIVER_ACCOUNT_ID as R_RECEIVER_ACCOUNT_ID,
                R.RECEIPT_KIND as "r_receipt_kind: String",
                R.ORIGINATED_FROM_TRANSACTION_HASH as R_ORIGINATED_FROM_TRANSACTION_HASH,
                ARA.RECEIPT_ID as ARA_RECEIPT_ID,
                ARA.INDEX_IN_ACTION_RECEIPT as ARA_INDEX_IN_ACTION_RECEIPT,
                ARA.ARGS as ARA_ARGS,
                ARA.RECEIPT_PREDECESSOR_ACCOUNT_ID as ARA_RECEIPT_PREDECESSOR_ACCOUNT_ID,
                ARA.RECEIPT_RECEIVER_ACCOUNT_ID as ARA_RECEIPT_RECEIVER_ACCOUNT_ID,
                ARA.RECEIPT_INCLUDED_IN_BLOCK_TIMESTAMP as ARA_RECEIPT_INCLUDED_IN_BLOCK_TIMESTAMP,
                ARA.ACTION_KIND as "ara_action_kind: String",
                B.BLOCK_HEIGHT as B_BLOCK_HEIGHT,
                B.BLOCK_HASH as B_BLOCK_HASH,
                B.PREV_BLOCK_HASH as B_PREV_BLOCK_HASH,
                B.BLOCK_TIMESTAMP as B_BLOCK_TIMESTAMP,
                B.GAS_PRICE as B_GAS_PRICE,
                B.AUTHOR_ACCOUNT_ID as B_AUTHOR_ACCOUNT_ID,
                EO.RECEIPT_ID as EO_RECEIPT_ID,
                EO.EXECUTED_IN_BLOCK_HASH  as EO_EXECUTED_IN_BLOCK_HASH ,
                EO.EXECUTED_IN_BLOCK_TIMESTAMP as EO_EXECUTED_IN_BLOCK_TIMESTAMP,
                EO.INDEX_IN_CHUNK as EO_INDEX_IN_CHUNK,
                EO.GAS_BURNT as EO_GAS_BURNT,
                EO.TOKENS_BURNT as EO_TOKENS_BURNT,
                EO.EXECUTOR_ACCOUNT_ID as EO_EXECUTOR_ACCOUNT_ID,
                EO.SHARD_ID as EO_SHARD_ID,
                EO.STATUS as "eo_status: String"
            FROM
                TRANSACTIONS T
                LEFT JOIN RECEIPTS R ON (T.CONVERTED_INTO_RECEIPT_ID = R.RECEIPT_ID
                        OR T.TRANSACTION_HASH = R.ORIGINATED_FROM_TRANSACTION_HASH)
                LEFT JOIN ACTION_RECEIPT_ACTIONS ARA ON ARA.RECEIPT_ID = R.RECEIPT_ID
                LEFT JOIN BLOCKS B ON B.BLOCK_HASH = R.INCLUDED_IN_BLOCK_HASH
                LEFT JOIN EXECUTION_OUTCOMES EO ON EO.RECEIPT_ID = R.RECEIPT_ID
            WHERE
                RECEIPT_RECEIVER_ACCOUNT_ID = ANY ($1)
                AND EO.STATUS IN ('SUCCESS_RECEIPT_ID', 'SUCCESS_VALUE')
                AND B.BLOCK_TIMESTAMP >= $2
                AND B.BLOCK_TIMESTAMP < $3
                AND NOT EXISTS (
                    SELECT 1
                    FROM RECEIPTS R2
                    JOIN EXECUTION_OUTCOMES EO2 ON EO2.RECEIPT_ID = R2.RECEIPT_ID
                    WHERE (T.CONVERTED_INTO_RECEIPT_ID = R2.RECEIPT_ID OR T.TRANSACTION_HASH = R2.ORIGINATED_FROM_TRANSACTION_HASH)
                    AND EO2.STATUS = 'FAILURE'
                );
            "##,
            &accs,
            &start_date_decimal,
            &end_date_decimal,
        )
        .fetch(&self.pool);

        let start = chrono::Utc::now();

        while let Some(txn) = stream_txs.next().await {
            match txn {
                Ok(txn) => {
                    if let Err(e) = sender_txn.send(txn).await {
                        error!("Error sending transaction: {}", e);
                    };
                }
                Err(e) => error!("Error getting transaction: {}", e),
            }
        }

        let end = chrono::Utc::now();
        info!(
            "Time taken to get incoming transactions: {:?} for {:?}",
            end - start,
            accs
        );

        Ok(())
    }

    #[instrument(skip(self, sender_txn))]
    pub async fn get_ft_incoming_txns(
        &self,
        accounts: collections::HashSet<String>,
        start_date: u128,
        end_date: u128,
        sender_txn: Sender<Transaction>,
    ) -> Result<()> {
        let accs: Vec<String> = accounts.into_iter().collect();
        let start_date_decimal = Decimal::from(start_date);
        let end_date_decimal = Decimal::from(end_date);

        let mut stream_txs = sqlx::query_as!(
            Transaction,
            r##"
            SELECT
                T.TRANSACTION_HASH as T_TRANSACTION_HASH,
                T.INCLUDED_IN_BLOCK_HASH as T_INCLUDED_IN_BLOCK_HASH,
                T.INCLUDED_IN_CHUNK_HASH as T_INCLUDED_IN_CHUNK_HASH,
                T.INDEX_IN_CHUNK as T_INDEX_IN_CHUNK,
                T.BLOCK_TIMESTAMP as T_BLOCK_TIMESTAMP,
                T.SIGNER_ACCOUNT_ID as T_SIGNER_ACCOUNT_ID,
                T.SIGNER_PUBLIC_KEY as T_SIGNER_PUBLIC_KEY,
                T.NONCE as T_NONCE,
                T.RECEIVER_ACCOUNT_ID as T_RECEIVER_ACCOUNT_ID,
                T.SIGNATURE as T_SIGNATURE,
                T.STATUS as "t_status: String",
                T.CONVERTED_INTO_RECEIPT_ID as T_CONVERTED_INTO_RECEIPT_ID,
                T.RECEIPT_CONVERSION_GAS_BURNT as T_RECEIPT_CONVERSION_GAS_BURNT,
                T.RECEIPT_CONVERSION_TOKENS_BURNT as T_RECEIPT_CONVERSION_TOKENS_BURNT,
                R.RECEIPT_ID as R_RECEIPT_ID,
                R.INCLUDED_IN_BLOCK_HASH as R_INCLUDED_IN_BLOCK_HASH,
                R.INCLUDED_IN_CHUNK_HASH as R_INCLUDED_IN_CHUNK_HASH,
                R.INDEX_IN_CHUNK as R_INDEX_IN_CHUNK,
                R.INCLUDED_IN_BLOCK_TIMESTAMP as R_INCLUDED_IN_BLOCK_TIMESTAMP,
                R.PREDECESSOR_ACCOUNT_ID as R_PREDECESSOR_ACCOUNT_ID,
                R.RECEIVER_ACCOUNT_ID as R_RECEIVER_ACCOUNT_ID,
                R.RECEIPT_KIND as "r_receipt_kind: String",
                R.ORIGINATED_FROM_TRANSACTION_HASH as R_ORIGINATED_FROM_TRANSACTION_HASH,
                ARA.RECEIPT_ID as ARA_RECEIPT_ID,
                ARA.INDEX_IN_ACTION_RECEIPT as ARA_INDEX_IN_ACTION_RECEIPT,
                ARA.ARGS as ARA_ARGS,
                ARA.RECEIPT_PREDECESSOR_ACCOUNT_ID as ARA_RECEIPT_PREDECESSOR_ACCOUNT_ID,
                ARA.RECEIPT_RECEIVER_ACCOUNT_ID as ARA_RECEIPT_RECEIVER_ACCOUNT_ID,
                ARA.RECEIPT_INCLUDED_IN_BLOCK_TIMESTAMP as ARA_RECEIPT_INCLUDED_IN_BLOCK_TIMESTAMP,
                ARA.ACTION_KIND as "ara_action_kind: String",
                B.BLOCK_HEIGHT as B_BLOCK_HEIGHT,
                B.BLOCK_HASH as B_BLOCK_HASH,
                B.PREV_BLOCK_HASH as B_PREV_BLOCK_HASH,
                B.BLOCK_TIMESTAMP as B_BLOCK_TIMESTAMP,
                B.GAS_PRICE as B_GAS_PRICE,
                B.AUTHOR_ACCOUNT_ID as B_AUTHOR_ACCOUNT_ID,
                EO.RECEIPT_ID as EO_RECEIPT_ID,
                EO.EXECUTED_IN_BLOCK_HASH  as EO_EXECUTED_IN_BLOCK_HASH ,
                EO.EXECUTED_IN_BLOCK_TIMESTAMP as EO_EXECUTED_IN_BLOCK_TIMESTAMP,
                EO.INDEX_IN_CHUNK as EO_INDEX_IN_CHUNK,
                EO.GAS_BURNT as EO_GAS_BURNT,
                EO.TOKENS_BURNT as EO_TOKENS_BURNT,
                EO.EXECUTOR_ACCOUNT_ID as EO_EXECUTOR_ACCOUNT_ID,
                EO.SHARD_ID as EO_SHARD_ID,
                EO.STATUS as "eo_status: String"
            FROM TRANSACTIONS t
                    LEFT JOIN RECEIPTS R ON (T.CONVERTED_INTO_RECEIPT_ID = R.RECEIPT_ID OR
                                                t.TRANSACTION_HASH = R.ORIGINATED_FROM_TRANSACTION_HASH)
                    LEFT JOIN ACTION_RECEIPT_ACTIONS ARA ON ARA.RECEIPT_ID = R.RECEIPT_ID
                    LEFT JOIN BLOCKS B ON B.BLOCK_HASH = R.INCLUDED_IN_BLOCK_HASH
                    LEFT JOIN EXECUTION_OUTCOMES EO ON EO.RECEIPT_ID = R.RECEIPT_ID
            WHERE eo.status IN ('SUCCESS_RECEIPT_ID', 'SUCCESS_VALUE')
                AND ARA.action_kind = 'FUNCTION_CALL'
                AND (ARA.args -> 'args_json' ->> 'receiver_id' = ANY($1) OR ARA.args -> 'args_json' ->> 'account_id' = ANY($1))
                AND B.BLOCK_TIMESTAMP >= $2
                AND B.BLOCK_TIMESTAMP < $3
                AND NOT EXISTS (
                    SELECT 1
                    FROM RECEIPTS R2
                    JOIN EXECUTION_OUTCOMES EO2 ON EO2.RECEIPT_ID = R2.RECEIPT_ID
                    WHERE (T.CONVERTED_INTO_RECEIPT_ID = R2.RECEIPT_ID OR T.TRANSACTION_HASH = R2.ORIGINATED_FROM_TRANSACTION_HASH)
                    AND EO2.STATUS = 'FAILURE'
            );
            "##,
            &accs,
            &start_date_decimal,
            &end_date_decimal,
        )
        .fetch(&self.pool);

        let start = chrono::Utc::now();

        while let Some(txn) = stream_txs.next().await {
            match txn {
                Ok(txn) => {
                    if let Err(e) = sender_txn.send(txn).await {
                        error!("Error sending transaction: {}", e);
                    };
                }
                Err(e) => error!("Error getting transaction: {}", e),
            }
        }

        let end = chrono::Utc::now();
        info!(
            "Time taken to get incoming FT transactions: {:?} for {:?}",
            end - start,
            accs
        );

        Ok(())
    }
}
