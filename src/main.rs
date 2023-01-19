use axum::{
    extract::{Query, State},
    routing::get,
    Router,
};
// use bb8::Pool;
// use bb8_postgres::PostgresConnectionManager;
use chrono::DateTime;
use hyper::StatusCode;
use sqlx::postgres::PgPoolOptions;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
};
// use tokio_postgres::NoTls;
use tracing::*;
use tracing_subscriber::FmtSubscriber;
use tta::{SqlClient, TTA};

pub mod tta;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("Starting up");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://labs_explorer:A05eC2a93B276b46C179@142.132.152.134/mainnet_node")
        .await?;

    let sql_client = SqlClient::new(pool);

    // Start services
    let tta_service = TTA::new(sql_client);

    let app = Router::new()
        .route("/tta", get(get_txns_report))
        .with_state(tta_service);

    info!("Binding server to 0.0.0.0:3000");
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    info!("Closing server on 0.0.0.0:3000");
    Ok(())
}

/// Utility function for mapping any error into a `500 Internal Server Error`
/// response.
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

// HTTP layer

async fn get_txns_report(
    Query(params): Query<HashMap<String, String>>,
    State(tta_service): State<TTA>,
) -> Result<(), (StatusCode, String)> {
    let start_date = params.get("start_date").unwrap();
    let end_date = params.get("end_date").unwrap();
    let accounts = params.get("accounts").unwrap();

    let start_date: DateTime<chrono::Utc> =
        DateTime::parse_from_rfc3339(start_date).unwrap().into();
    let end_date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(end_date).unwrap().into();
    let accounts: HashSet<String> = accounts.split(',').map(|s| s.to_string()).collect();

    match tta_service
        .get_txns_report(start_date, end_date, accounts)
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => Err(internal_error(e)),
    }
}
