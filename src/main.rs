use csv::Writer;
use hyper::Body;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_loki::url::Url;
use tta::models::ReportRow;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    routing::post,
    Json, Router,
};

use chrono::DateTime;
use dotenvy::dotenv;

use near_jsonrpc_client::{JsonRpcClient, NEAR_MAINNET_ARCHIVAL_RPC_URL};
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use std::{
    collections::{HashMap, HashSet},
    env,
    sync::{Arc, RwLock},
};
use tokio::{spawn, sync::Semaphore};
use tracing::*;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, FmtSubscriber};
use tta::tta_impl::TTA;

use crate::tta::{ft_metadata::FtService, sql::sql_queries::SqlClient};

pub mod tta;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    info!("Starting up");

    match dotenv() {
        Ok(_) => info!("Loaded .env file"),
        Err(e) => warn!("Failed to load .env file: {}", e),
    }

    init_tracing()?;

    let app = router().await?;

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

fn init_tracing() -> anyhow::Result<()> {
    // Check the environment variable
    let env = env::var("ENV").unwrap_or_else(|_| "production".to_string());

    let filter = match option_env!("LOG_LEVEL") {
        Some(level) => EnvFilter::new(level),
        None => EnvFilter::new("info"),
    };

    if env == "local" {
        // If we're in a local environment, just set a simple subscriber
        tracing::subscriber::set_global_default(
            FmtSubscriber::builder().with_env_filter(filter).finish(),
        )?;
    } else {
        // If we're not in a local environment, set up Loki logging
        let (layer, task) = tracing_loki::builder()
            .label("job", "tta")?
            .build_url(Url::parse("http://loki-33z9:3100")?)?;

        tracing::subscriber::set_global_default(
            FmtSubscriber::builder()
                .with_env_filter(filter)
                .finish()
                .with(layer),
        )?;

        spawn(task);
    }

    debug!("Tracing initialized.");

    Ok(())
}

async fn router() -> anyhow::Result<Router> {
    let pool = PgPoolOptions::new()
        .max_connections(30)
        .connect(env!("DATABASE_URL"))
        .await?;

    let sql_client = SqlClient::new(pool);
    let near_client = JsonRpcClient::connect(NEAR_MAINNET_ARCHIVAL_RPC_URL);
    let ft_service = FtService::new(near_client);
    let semaphore = Arc::new(Semaphore::new(30));

    let tta_service = TTA::new(sql_client, ft_service, semaphore);

    let trace = TraceLayer::new_for_http();
    let cors = CorsLayer::new().allow_methods(Any).allow_origin(Any);
    let middleware = ServiceBuilder::new().layer(trace).layer(cors);

    Ok(Router::new()
        .route("/tta", post(get_txns_report))
        .route("/tta", get(get_txns_report))
        .with_state(tta_service)
        .layer(middleware))
}

// HTTP layer
type AccountID = String;
type TransactionID = String;
type Metadata = HashMap<AccountID, HashMap<TransactionID, String>>;

#[derive(Debug, Deserialize)]
struct TxnsReportParams {
    pub start_date: String,
    pub end_date: String,
    pub accounts: String,
    pub include_balances: Option<bool>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct TxnsReportWithMetadata {
    pub metadata: Metadata,
}

async fn get_txns_report(
    Query(params): Query<TxnsReportParams>,
    State(tta_service): State<TTA>,
    metadata_body: Option<Json<TxnsReportWithMetadata>>,
) -> Result<Response<Body>, AppError> {
    let start_date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(&params.start_date)
        .unwrap()
        .into();
    let end_date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(&params.end_date)
        .unwrap()
        .into();

    let accounts: HashSet<String> = params
        .accounts
        .split(',')
        .map(String::from)
        .filter(|account| account != "near" && account != "system")
        .collect();

    let include_balances = params.include_balances.unwrap_or(false);

    let metadata = Arc::new(RwLock::new(metadata_body.unwrap_or_default().0));

    let csv_data = tta_service
        .get_txns_report(
            start_date.timestamp_nanos() as u128,
            end_date.timestamp_nanos() as u128,
            accounts,
            include_balances,
            metadata,
        )
        .await?;

    // Create a Writer with a Vec<u8> as the underlying writer
    let mut wtr = Writer::from_writer(Vec::new());

    // Write the headers
    wtr.write_record(&ReportRow::get_vec_headers())?;

    // Write each row
    for row in csv_data {
        let record: Vec<String> = row.to_vec();
        wtr.write_record(&record)?;
    }

    // Get the CSV data
    let csv_data = wtr.into_inner()?;

    // Create a response with the CSV data
    let response = Response::builder()
        .header("Content-Type", "text/csv")
        .header("Content-Disposition", "attachment; filename=data.csv")
        .body(Body::from(csv_data))?;

    Ok(response)
}

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test_helper::TestClient;
    use futures_util::future::join_all;

    #[tokio::test]
    async fn test_tta_router() {
        let router = router().await.unwrap();
        let client = TestClient::new(router);
        let res = client.get("/tta?start_date=2023-01-01T00:00:00Z&end_date=2023-02-01T00:00:00Z&accounts=nf-payments.near&include_balances=false").send().await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn loadtest_tta() {
        let router = router().await.unwrap();
        let request_url = "/tta?start_date=2023-01-01T00:00:00Z&end_date=2023-02-01T00:00:00Z&accounts=nf-payments.near&include_balances=false";

        let futures = (0..20)
            .map(|_| {
                let router = router.clone(); // Clone the router for each request
                tokio::spawn(async move {
                    let client = TestClient::new(router); // Create a new client for each request
                    let res = client.get(request_url).send().await;
                    assert_eq!(res.status(), StatusCode::OK);
                    res
                })
            })
            .collect::<Vec<_>>();

        // wait for all requests to complete
        let results: Vec<_> = join_all(futures).await.into_iter().collect();

        for result in results {
            match result {
                Ok(res) => {
                    assert_eq!(res.status(), StatusCode::OK);
                }
                Err(e) => {
                    eprintln!("Request error: {:?}", e);
                    panic!("Request failed");
                }
            }
        }
    }
}
