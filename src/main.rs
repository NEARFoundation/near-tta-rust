use anyhow::Result;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use chrono::DateTime;
use dotenvy::dotenv;
use hyper::StatusCode;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use std::{collections::HashSet, net::SocketAddr};
use tracing::*;
use tracing_subscriber::FmtSubscriber;
use tta::{SqlClient, TTA};
pub mod tta;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting up");

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(env!("DATABASE_URL"))
        .await?;

    let sql_client = SqlClient::new(pool);

    // Start services
    let tta_service = TTA::new(sql_client);

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
) -> Result<(), AppError> {
    let start_date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(&params.start_date)
        .unwrap()
        .into();
    let end_date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(&params.end_date)
        .unwrap()
        .into();
    let accounts: HashSet<String> = params.accounts.split(',').map(String::from).collect();

    tta_service
        .get_txns_report(
            start_date.timestamp_nanos() as u128,
            end_date.timestamp_nanos() as u128,
            accounts,
        )
        .await?;

    Ok(())
}

// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}