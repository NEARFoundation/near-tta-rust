use anyhow::Result;
use axum::{response::IntoResponse, Router};
use csv::Writer;
use hyper::{Body, Response};
use tta::models::ReportRow;

use axum::{
    extract::{Query, State},
    routing::get,
};
use chrono::DateTime;
use dotenvy::dotenv;

use near_jsonrpc_client::{JsonRpcClient, NEAR_MAINNET_ARCHIVAL_RPC_URL};
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use std::{collections::HashSet, sync::Arc};
use tokio::sync::Mutex;
use tracing::*;
use tracing_subscriber::FmtSubscriber;
use tta::tta_impl::TTA;

use crate::tta::{ft_metadata::FtMetadataCache, sql::sql_queries::SqlClient};

pub mod tta;

#[tokio::main]
async fn main() -> Result<()> {
    info!("Starting up");

    match dotenv() {
        Ok(_) => info!("Loaded .env file"),
        Err(e) => warn!("Failed to load .env file: {}", e),
    }

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(env!("DATABASE_URL"))
        .await?;

    let sql_client = SqlClient::new(pool);
    let near_client = JsonRpcClient::connect(NEAR_MAINNET_ARCHIVAL_RPC_URL);
    let ft_metadata_cache = Arc::new(Mutex::new(FtMetadataCache::new(near_client)));

    // Start services
    let tta_service = TTA::new(sql_client, ft_metadata_cache.clone());

    let app = Router::new()
        .route("/tta", get(get_txns_report))
        .with_state(tta_service);

    let ip = env!("IP");
    let port = env!("PORT");
    let address = format!("{ip}:{port}");
    info!("Binding server to {address}");

    axum::Server::bind(&address.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    info!("Closing server on {address}");
    Ok(())
}

// HTTP layer

#[derive(Debug, Deserialize)]
struct TxnsReportParams {
    pub start_date: String,
    pub end_date: String,
    pub accounts: String,
}

async fn get_txns_report(
    Query(params): Query<TxnsReportParams>,
    State(tta_service): State<TTA>,
) -> Result<impl IntoResponse, Response<Body>> {
    let start_date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(&params.start_date)
        .unwrap()
        .into();
    let end_date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(&params.end_date)
        .unwrap()
        .into();
    let accounts: HashSet<String> = params.accounts.split(',').map(String::from).collect();

    let csv_data = tta_service
        .get_txns_report(
            start_date.timestamp_nanos() as u128,
            end_date.timestamp_nanos() as u128,
            accounts,
        )
        .await
        .unwrap();

    // Create a Writer with a Vec<u8> as the underlying writer
    let mut wtr = Writer::from_writer(Vec::new());

    // Write the headers
    wtr.write_record(&ReportRow::get_vec_headers()).unwrap();

    // Write each row
    for row in csv_data {
        let record: Vec<String> = row.to_vec();
        wtr.write_record(&record).unwrap();
    }

    // Get the CSV data
    let csv_data = wtr.into_inner().unwrap();

    // Create a response with the CSV data
    let response = Response::builder()
        .header("Content-Type", "text/csv")
        .header("Content-Disposition", "attachment; filename=data.csv")
        .body(Body::from(csv_data))
        .unwrap();

    Ok(response)
}
