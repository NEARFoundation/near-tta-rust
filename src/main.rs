use csv::Writer;
use hyper::Body;
use kitwallet::KitWallet;
use near_primitives::types::AccountId;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_loki::url::Url;
use tta::models::ReportRow;

use axum::{
    body,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    routing::post,
    Json, Router,
};

use chrono::DateTime;
use dotenvy::dotenv;

use futures_util::future::join_all;
use near_jsonrpc_client::JsonRpcClient;
use serde::{Deserialize, Serialize};
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
use tta_rust::{get_accounts_and_lockups, results_to_response};

use crate::tta::{ft_metadata::FtService, sql::sql_queries::SqlClient, tta_impl::safe_divide_u128};

pub mod kitwallet;
pub mod lockup;
pub mod tta;

const POOL_SIZE: u32 = 500;
const SEMAPHORE_SIZE: usize = 50;

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
        .max_connections(POOL_SIZE)
        .connect(env!("DATABASE_URL"))
        .await?;

    let sql_client = SqlClient::new(pool);
    // let archival_near_client = JsonRpcClient::connect("http://beta.rpc.mainnet.near.org");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60 * 5))
        .build()?;
    let archival_near_client =
        JsonRpcClient::with(client).connect("http://beta.rpc.mainnet.near.org");
    // let near_client = JsonRpcClient::connect(NEAR_MAINNET_RPC_URL);
    let ft_service = FtService::new(archival_near_client);
    let kitwallet = KitWallet::new();
    let semaphore = Arc::new(Semaphore::new(SEMAPHORE_SIZE));

    let tta_service = TTA::new(sql_client.clone(), ft_service.clone(), semaphore);

    let trace = TraceLayer::new_for_http();
    let cors = CorsLayer::new().allow_methods(Any).allow_origin(Any);
    let middleware = ServiceBuilder::new().layer(trace).layer(cors);

    Ok(Router::new()
        .route("/tta", post(get_txns_report))
        .route("/tta", get(get_txns_report))
        .with_state(tta_service)
        .route("/likelyBlockId", get(get_closest_block_id))
        .with_state(sql_client.clone())
        .route("/balances", get(get_balances))
        .route("/balances", post(get_balances))
        .with_state((sql_client.clone(), ft_service.clone(), kitwallet.clone()))
        .route("/balancesfull", post(get_balances_full))
        .with_state((sql_client.clone(), ft_service.clone(), kitwallet))
        .route("/staking", get(get_staking_report))
        .route("/staking", post(get_staking_report))
        .with_state((sql_client.clone(), ft_service.clone()))
        .route("/lockup", get(get_lockup_balances))
        .route("/lockup", post(get_lockup_balances))
        .with_state((sql_client, ft_service))
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
        .map(|s| String::from(s.trim()))
        .filter(|account| account != "near" && account != "system" && !account.is_empty())
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

#[derive(Debug, Deserialize)]
struct ClosestBlockIdParams {
    pub date: String,
}

async fn get_closest_block_id(
    Query(params): Query<ClosestBlockIdParams>,
    State(sql_client): State<SqlClient>,
) -> Result<Response<Body>, AppError> {
    let date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(&params.date).unwrap().into();
    let nanos = date.timestamp_nanos() as u128;
    let d = sql_client.get_closest_block_id(nanos).await?;
    Ok(Response::new(Body::from(d.to_string())))
}

#[derive(Debug, Deserialize)]
struct GetBalances {
    pub start_date: String,
    pub end_date: String,
    pub accounts: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetBalancesBody {
    pub accounts: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
struct GetBalancesResultRow {
    pub account: String,
    pub start_date: String,
    pub end_date: String,
    pub start_block_id: u128,
    pub end_block_id: u128,
    pub token_id: String,
    pub symbol: String,
    pub lockup_of: Option<String>,
    pub start_balance: Option<f64>,
    pub end_balance: Option<f64>,
}

async fn get_balances(
    Query(params): Query<GetBalances>,
    State((sql_client, ft_service, kitwallet)): State<(SqlClient, FtService, KitWallet)>,
    body: Option<Json<GetBalancesBody>>,
) -> Result<Response<Body>, AppError> {
    let start_date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(&params.start_date)
        .unwrap()
        .into();
    let end_date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(&params.end_date)
        .unwrap()
        .into();
    let start_nanos = start_date.timestamp_nanos() as u128;
    let end_nanos = end_date.timestamp_nanos() as u128;

    let start_block_id = sql_client.get_closest_block_id(start_nanos).await?;
    let end_block_id = sql_client.get_closest_block_id(end_nanos).await?;
    let a = match body {
        Some(body) => body.accounts.join(","),
        None => params.accounts.unwrap_or("".to_string()),
    };

    let accounts = get_accounts_and_lockups(&a);
    let mut f = vec![];

    for (a, b) in accounts.clone() {
        f.push(a.clone());
        if let Some(b) = b {
            f.push(b.clone())
        };
    }

    kitwallet.get_likely_tokens_for_accounts(f).await?;

    let mut handles = vec![];

    for (account, lockup_of) in accounts {
        let ft_service = ft_service.clone();
        let start_block_id = start_block_id;
        let end_block_id = end_block_id;
        let start_date = start_date;
        let end_date = end_date;
        let kitwallet = kitwallet.clone();

        let handle = spawn(async move {
            info!(
                "Getting balances for {}, dates: start {} end {}",
                account, start_date, end_date
            );
            let mut rows: Vec<GetBalancesResultRow> = vec![];

            let likely_tokens = kitwallet.get_likely_tokens(account.clone()).await?;
            let token_handles: Vec<_> = likely_tokens
                .iter()
                .map(|token| {
                    let token = token.clone();
                    let account = account.clone();
                    let ft_service = ft_service.clone();
                    let lockup_of = lockup_of.clone();
                    async move {
                        let metadata = match ft_service.assert_ft_metadata(&token).await {
                            Ok(v) => v,
                            Err(e) => {
                                debug!("{}: {}", account, e);
                                return Err(e);
                            }
                        };
                        let start_balance = match ft_service
                            .assert_ft_balance(&token, &account, start_block_id as u64)
                            .await
                        {
                            Ok(v) => v,
                            Err(e) => {
                                debug!("{}: {}", account, e);
                                0.0
                            }
                        };
                        let end_balance = match ft_service
                            .assert_ft_balance(&token, &account, end_block_id as u64)
                            .await
                        {
                            Ok(v) => v,
                            Err(e) => {
                                debug!("{}: {}", account, e);
                                0.0
                            }
                        };
                        let record = GetBalancesResultRow {
                            account: account.clone(),
                            start_date: start_date.to_rfc3339(),
                            end_date: end_date.to_rfc3339(),
                            start_block_id,
                            end_block_id,
                            start_balance: Some(start_balance),
                            end_balance: Some(end_balance),
                            token_id: token.clone(),
                            symbol: metadata.symbol,
                            lockup_of,
                        };
                        Ok(record)
                    }
                })
                .collect();

            let token_results: Vec<_> = join_all(token_handles).await;
            for result in token_results {
                match result {
                    Ok(record) => rows.push(record),
                    Err(e) => {
                        debug!("Token fetch error: {:?}", e);
                    }
                }
            }

            let start_near_balance = match ft_service
                .get_near_balance(&account, start_block_id as u64)
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    debug!("{}: {}", account, e);
                    None
                }
            };
            let end_near_balance = match ft_service
                .get_near_balance(&account, end_block_id as u64)
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    debug!("{}: {}", account, e);
                    None
                }
            };

            let record = GetBalancesResultRow {
                account: account.clone(),
                start_date: start_date.to_rfc3339(),
                end_date: end_date.to_rfc3339(),
                start_block_id,
                end_block_id,
                start_balance: start_near_balance.map(|start| start.0),
                end_balance: end_near_balance.map(|end: (f64, f64)| end.0),
                token_id: "NEAR".to_string(),
                symbol: "NEAR".to_string(),
                lockup_of,
            };
            rows.push(record);

            anyhow::Ok(rows)
        });
        handles.push(handle);
    }

    let mut rows = vec![];
    join_all(handles).await.iter().for_each(|row| match row {
        Ok(result) => match result {
            Ok(res) => rows.extend(res.iter().cloned()),
            Err(e) => {
                println!("{:?}", e)
            }
        },
        Err(e) => {
            warn!("{:?}", e)
        }
    });

    let r = results_to_response(rows)?;
    Ok(r)
}

#[derive(Debug, Deserialize)]
struct GetBalancesFull {
    pub start_date: String,
    pub end_date: String,
    pub accounts: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
struct GetBalancesFullResultRow {
    pub account: String,
    pub date: String,
    pub block_id: u128,
    pub token_id: String,
    pub symbol: String,
    pub lockup_of: Option<String>,
    pub balance: Option<f64>,
}

#[tracing::instrument(skip(sql_client, ft_service, kitwallet))]
async fn get_balances_full(
    State((sql_client, ft_service, kitwallet)): State<(SqlClient, FtService, KitWallet)>,
    Json(params): Json<GetBalancesFull>,
) -> Result<Response<Body>, AppError> {
    let start_date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(&params.start_date)
        .unwrap()
        .into();
    let end_date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(&params.end_date)
        .unwrap()
        .into();
    let accounts = params.accounts.join(",");
    let accounts = get_accounts_and_lockups(accounts.as_str());
    let mut f = vec![];

    for (a, b) in &accounts {
        f.push(a.clone());
        if let Some(b) = b {
            f.push(b.clone())
        };
    }
    error!("test");

    let likely_tokens = kitwallet.get_likely_tokens_for_accounts(f).await?;

    // put all days between start and end in all_dates.
    let all_dates = {
        let mut dates = vec![];
        let mut date = start_date;
        while date <= end_date {
            dates.push(date);
            date += chrono::Duration::days(1);
        }
        dates
    };

    let block_ids = sql_client
        .get_closest_block_ids(
            all_dates
                .iter()
                .map(|d| d.timestamp_nanos() as u128)
                .collect(),
        )
        .await?;
    let mut handles = vec![];

    for (idx, date) in all_dates.iter().enumerate() {
        let date = *date;
        let idx = idx;
        let block_id = block_ids[idx];

        for (account, lockup_of) in &accounts {
            let ft_service = ft_service.clone();
            let likely_tokens = likely_tokens.get(account).unwrap().clone();
            let account = account.clone();
            let lockup_of = lockup_of.clone();

            // sleep 1 ms
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;

            let handle = spawn(async move {
                let mut rows: Vec<GetBalancesFullResultRow> = vec![];

                let token_handles: Vec<_> = likely_tokens
                    .iter()
                    .map(|token| {
                        let token = token.clone();
                        let account = account.clone();
                        let ft_service = ft_service.clone();
                        let lockup_of = lockup_of.clone();
                        async move {
                            let metadata = match ft_service.assert_ft_metadata(&token).await {
                                Ok(v) => v,
                                Err(e) => {
                                    debug!("{}: {}", account, e);
                                    return Err(e);
                                }
                            };
                            let balance = match ft_service
                                .assert_ft_balance(&token, &account, block_id as u64)
                                .await
                            {
                                Ok(v) => Some(v),
                                Err(e) => {
                                    debug!("{}: {}", account, e);
                                    None
                                }
                            };

                            let record = GetBalancesFullResultRow {
                                account: account.clone(),
                                date: date.to_rfc3339(),
                                token_id: token.clone(),
                                symbol: metadata.symbol,
                                lockup_of: lockup_of.clone(),
                                block_id,
                                balance,
                            };
                            Ok(record)
                        }
                    })
                    .collect();

                let token_results: Vec<_> = join_all(token_handles).await;
                for result in token_results {
                    match result {
                        Ok(record) => rows.push(record),
                        Err(e) => {
                            debug!("Token fetch error: {:?}", e);
                        }
                    }
                }

                let near_balance =
                    match ft_service.get_near_balance(&account, block_id as u64).await {
                        Ok(v) => v.map(|v| v.0),
                        Err(e) => {
                            error!("{}: {}", account, e);
                            None
                        }
                    };

                let record = GetBalancesFullResultRow {
                    account: account.clone(),
                    date: date.to_rfc3339(),
                    block_id,
                    balance: near_balance,
                    token_id: "NEAR".to_string(),
                    symbol: "NEAR".to_string(),
                    lockup_of: lockup_of.clone(),
                };
                rows.push(record);

                anyhow::Ok(rows)
            });
            handles.push(handle);
        }
    }

    let mut rows = vec![];
    join_all(handles).await.iter().for_each(|row| match row {
        Ok(result) => match result {
            Ok(res) => rows.extend(res.iter().cloned()),
            Err(e) => {
                error!("{:?}", e)
            }
        },
        Err(e) => {
            warn!("{:?}", e)
        }
    });

    let r = results_to_response(rows)?;
    Ok(r)
}

#[derive(Debug, Deserialize)]
struct DateAndAccounts {
    pub date: String,
    pub accounts: String,
}

#[derive(Debug, Serialize, Clone)]
struct StakingReportRow {
    pub account: String,
    pub staking_pool: String,
    pub amount_staked: f64,
    pub amount_unstaked: f64,
    pub ready_for_withdraw: bool,
    pub lockup_of: Option<String>,
    pub date: String,
    pub block_id: u128,
}

#[derive(Debug, Deserialize, Clone)]
struct StakingDeposit {
    pub deposit: String,
    pub validator_id: String,
}

async fn get_staking_report(
    params: Option<Query<DateAndAccounts>>,
    State((sql_client, ft_service)): State<(SqlClient, FtService)>,
    body: Option<Json<DateAndAccounts>>,
) -> Result<Response<Body>, AppError> {
    let params = match params {
        Some(params) => params.0,
        None => body.unwrap().0,
    };

    let date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(&params.date).unwrap().into();
    let start_nanos = date.timestamp_nanos() as u128;

    let block_id = sql_client.get_closest_block_id(start_nanos).await?;

    let accounts = get_accounts_and_lockups(&params.accounts);

    let client = reqwest::Client::new();
    let mut handles = vec![];

    for (account, master_account) in accounts {
        let client = client.clone();
        let ft_service = ft_service.clone();
        let block_id = block_id;

        let handle = spawn(async move {
            info!("Getting staking for {}", account);
            let mut rows: Vec<StakingReportRow> = vec![];

            let staking_deposits = client
                .get(format!(
                    "https://api.kitwallet.app/staking-deposits/{account}"
                ))
                .send()
                .await?
                .json::<Vec<StakingDeposit>>()
                .await?;
            info!(
                "Account {} staking deposits: {:?}",
                account, staking_deposits
            );

            let handles: Vec<_> = staking_deposits
                .iter()
                .map(|staking_deposit| {
                    let staking_deposit = staking_deposit.clone();
                    let account = account.clone();
                    let ft_service = ft_service.clone();
                    let master_account = master_account.clone();
                    async move {
                        let staking_details = match ft_service
                            .get_staking_details(
                                &staking_deposit.validator_id,
                                &account,
                                block_id as u64,
                            )
                            .await
                        {
                            Ok(v) => v,
                            Err(e) => {
                                debug!("{}: {}", account, e);
                                return Err(e);
                            }
                        };

                        if staking_details.0 == 0.0 && staking_details.1 == 0.0 {
                            return Ok(None);
                        }

                        let record = StakingReportRow {
                            account,
                            staking_pool: staking_deposit.validator_id.clone(),
                            amount_staked: staking_details.0,
                            amount_unstaked: staking_details.1,
                            ready_for_withdraw: staking_details.2,
                            lockup_of: master_account,
                            date: date.to_rfc3339(),
                            block_id,
                        };
                        Ok(Some(record))
                    }
                })
                .collect();

            let results: Vec<_> = join_all(handles).await;
            for result in results {
                match result {
                    Ok(record) => {
                        if let Some(record) = record {
                            rows.push(record)
                        }
                    }
                    Err(e) => {
                        error!("staking error: {:?}", e);
                    }
                }
            }

            anyhow::Ok(rows)
        });
        handles.push(handle);
    }

    let mut rows = vec![];
    join_all(handles).await.iter().for_each(|row| match row {
        Ok(result) => match result {
            Ok(res) => rows.extend(res.iter().cloned()),
            Err(e) => {
                println!("{:?}", e)
            }
        },
        Err(e) => {
            warn!("{:?}", e)
        }
    });

    let r = results_to_response(rows)?;
    Ok(r)
}

#[derive(Debug, Serialize, Clone)]
struct LockupBalanceRow {
    pub account: String,
    pub lockup_balance: Option<f64>,
    pub locked_amount: Option<f64>,
    pub liquid_amount: Option<f64>,
    pub lockup_of: Option<String>,
    pub date: String,
    pub block_id: u128,
}

async fn get_lockup_balances(
    params: Option<Query<DateAndAccounts>>,
    State((sql_client, ft_service)): State<(SqlClient, FtService)>,
    body: Option<Json<DateAndAccounts>>,
) -> Result<Response<Body>, AppError> {
    let params = match params {
        Some(params) => params.0,
        None => body.unwrap().0,
    };

    let date: DateTime<chrono::Utc> = DateTime::parse_from_rfc3339(&params.date).unwrap().into();
    let date_nanos = date.timestamp_nanos() as u128;
    let block_id = sql_client.get_closest_block_id(date_nanos).await?;
    let accounts = get_accounts_and_lockups(&params.accounts);
    let mut handles = vec![];

    for (account, master_account) in accounts {
        if master_account.is_none() {
            continue;
        }

        let ft_service = ft_service.clone();
        let account: AccountId = account.parse().unwrap();
        let block_id = block_id as u64;

        let handle = spawn(async move {
            info!("Getting lockup_balance for {}", account);

            let account = account.clone();
            let ft_service = ft_service.clone();
            let master_account = master_account.clone();

            let lockup =
                lockup::l::get_lockup_contract_state(&ft_service.near_client, &account, &block_id)
                    .await?;
            let timestamp = date.timestamp_nanos();

            // todo: address has_bug, get hash of contract
            let locked_amount = lockup.get_locked_amount(timestamp as u64, false);
            // let unlocked = lockup.get_unvested_amount(timestamp as u64, false);
            let locked_amount = safe_divide_u128(locked_amount.0, 24);
            let near_balance = ft_service.get_near_balance(&account, block_id).await?;

            info!("Account {} lockup balance: {:?}", account, near_balance);

            let record = LockupBalanceRow {
                account: account.to_string(),
                lockup_of: master_account,
                lockup_balance: near_balance.map(|v| v.0),
                locked_amount: Some(locked_amount),
                liquid_amount: near_balance.map(|v| v.0 - locked_amount),
                date: date.to_rfc3339(),
                block_id: block_id as u128,
            };

            anyhow::Ok(record)
        });
        handles.push(handle);
    }

    let mut rows = vec![];
    join_all(handles).await.iter().for_each(|row| match row {
        Ok(result) => match result {
            Ok(res) => rows.push(res.clone()),
            Err(e) => {
                println!("{:?}", e)
            }
        },
        Err(e) => {
            warn!("{:?}", e)
        }
    });

    let r = results_to_response(rows)?;
    Ok(r)
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
