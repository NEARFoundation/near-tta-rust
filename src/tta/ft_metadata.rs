use anyhow::{bail, Result};
use governor::{clock, state, Quota, RateLimiter};
use lru::LruCache;
use near_jsonrpc_client::JsonRpcClient;
use near_jsonrpc_primitives::types::query::{QueryResponseKind, RpcQueryRequest, RpcQueryResponse};
use near_primitives::{
    types::{
        BlockId::Height,
        BlockReference,
        Finality::{self},
        FunctionArgs,
    },
    views::{AccountView, CallResult, QueryRequest},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
};
use tokio::{join, sync::RwLock};
use tracing::{debug, info};

use std::hash::{Hash, Hasher};

use crate::tta::tta_impl::safe_divide_u128;

#[derive(Debug, Clone)]
pub struct CompositeKey {
    block_id: u64,
    account_id: String,
    token_id: String,
}

impl PartialEq for CompositeKey {
    fn eq(&self, other: &Self) -> bool {
        self.block_id == other.block_id
            && self.account_id == other.account_id
            && self.token_id == other.token_id
    }
}

impl Eq for CompositeKey {}

impl Hash for CompositeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.block_id.hash(state);
        self.account_id.hash(state);
        self.token_id.hash(state);
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FtMetadata {
    pub spec: String,
    pub name: String,
    pub symbol: String,
    pub icon: Option<String>,
    pub reference: Option<String>,
    pub reference_hash: Option<String>,
    pub decimals: u8,
}

type RateLim = RateLimiter<
    state::NotKeyed,
    state::InMemoryState,
    clock::QuantaClock,
    governor::middleware::NoOpMiddleware<clock::QuantaInstant>,
>;

#[derive(Debug, Clone)]
pub struct FtService {
    pub ft_metadata_cache: Arc<RwLock<HashMap<String, FtMetadata>>>,
    pub ft_balances_cache: Arc<RwLock<LruCache<CompositeKey, f64>>>,
    pub near_client: JsonRpcClient,
    pub archival_rate_limiter: Arc<RwLock<RateLim>>,
}

impl FtService {
    pub fn new(near_client: JsonRpcClient) -> Self {
        FtService {
            ft_metadata_cache: Arc::new(RwLock::new(HashMap::new())),
            ft_balances_cache: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(1_000_000).unwrap(),
            ))),
            near_client,
            archival_rate_limiter: Arc::new(RwLock::new(RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(5u32).unwrap(),
            )))),
        }
    }

    pub async fn assert_ft_metadata(&self, ft_token_id: &str) -> Result<FtMetadata> {
        if !self
            .ft_metadata_cache
            .clone()
            .read()
            .await
            .contains_key(ft_token_id)
        {
            self.archival_rate_limiter.write().await.until_ready().await;
            let args = json!({}).to_string().into_bytes();
            let result = match view_function_call(
                &self.near_client,
                QueryRequest::CallFunction {
                    account_id: ft_token_id.parse().unwrap(),
                    method_name: "ft_metadata".to_string(),
                    args: FunctionArgs::from(args),
                },
                BlockReference::Finality(Finality::Final),
            )
            .await
            {
                Ok(v) => v,
                Err(e) => {
                    bail!(
                        "Error getting ft_metadata for ft_token_id: {}, error: {:?}",
                        ft_token_id,
                        e
                    );
                }
            };

            let v = serde_json::from_slice(&result)?;
            let e = self.ft_metadata_cache.clone();
            let mut w = e.write().await;
            w.insert(ft_token_id.to_string(), v);
        }

        match self.ft_metadata_cache.read().await.get(ft_token_id) {
            Some(v) => Ok(v.clone()),
            None => bail!("ft_metadata not found"),
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn assert_ft_balance(
        &self,
        token_id: &String,
        account_id: &String,
        block_id: u64,
    ) -> Result<f64> {
        if self
            .ft_balances_cache
            .clone()
            .read()
            .await
            .contains(&CompositeKey {
                block_id,
                account_id: account_id.clone(),
                token_id: token_id.clone(),
            })
        {
            debug!("Found ft_balance in cache");
            let mut w = self.ft_balances_cache.write().await;
            return Ok(*w
                .get(&CompositeKey {
                    block_id,
                    account_id: account_id.clone(),
                    token_id: token_id.clone(),
                })
                .unwrap());
        }
        let metadata = self.assert_ft_metadata(token_id).await.unwrap();

        self.archival_rate_limiter.write().await.until_ready().await;
        let args = json!({ "account_id": account_id }).to_string().into_bytes();
        info!("Calling ft_balance_of");
        let result = match view_function_call(
            &self.near_client,
            QueryRequest::CallFunction {
                account_id: token_id.parse().unwrap(),
                method_name: "ft_balance_of".to_string(),
                args: FunctionArgs::from(args),
            },
            BlockReference::BlockId(Height(block_id)),
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                bail!(
                    "Error assert_ft_balance for token_id: {}, error: {:?}",
                    token_id,
                    e
                );
            }
        };

        let amount: String = serde_json::from_slice(&result)?;
        let amount = amount.parse::<u128>()?;
        let amount = safe_divide_u128(amount, metadata.decimals as u32);

        debug!("Got ft_balance amount: {}", amount);
        let mut w = self.ft_balances_cache.write().await;
        w.put(
            CompositeKey {
                block_id,
                account_id: account_id.clone(),
                token_id: token_id.clone(),
            },
            amount,
        );

        Ok(amount)
    }

    pub async fn get_near_balance(&self, account_id: &str, block_id: u64) -> Result<(f64, f64)> {
        self.archival_rate_limiter.write().await.until_ready().await;
        let RpcQueryResponse { kind, .. } = match self
            .near_client
            .call(RpcQueryRequest {
                request: QueryRequest::ViewAccount {
                    account_id: account_id.parse().unwrap(),
                },
                block_reference: BlockReference::BlockId(Height(block_id)),
            })
            .await
        {
            Ok(v) => v,
            Err(e) => {
                bail!("Error calling ViewAccount: {:?}", e);
            }
        };
        let view = match kind {
            QueryResponseKind::ViewAccount(view) => view,
            _ => {
                bail!("Received unexpected kind: {:?}", kind);
            }
        };

        let amount = safe_divide_u128(view.amount, 24);
        let locked = safe_divide_u128(view.locked, 24);

        Ok((amount, locked))
    }

    pub async fn get_staking_details(
        &self,
        staking_pool: &str,
        account_id: &str,
        block_id: u64,
    ) -> Result<(f64, f64, bool)> {
        let args = json!({ "account_id": account_id }).to_string().into_bytes();

        let unstaked_balance_future = self.get_unstaked_balance(staking_pool, &args, block_id);
        let staked_balance_future = self.get_staked_balance(staking_pool, &args, block_id);
        let unstaked_balance_available_future =
            self.is_unstaked_balance_available(staking_pool, &args, block_id);

        let (unstaked_balance, staked_balance, unstaked_balance_available) = join!(
            unstaked_balance_future,
            staked_balance_future,
            unstaked_balance_available_future
        );

        Ok((
            safe_divide_u128(staked_balance?, 24),
            safe_divide_u128(unstaked_balance?, 24),
            unstaked_balance_available?,
        ))
    }

    async fn get_unstaked_balance(
        &self,
        staking_pool: &str,
        args: &[u8],
        block_id: u64,
    ) -> Result<u128> {
        self.archival_rate_limiter.write().await.until_ready().await;
        let result = view_function_call(
            &self.near_client,
            QueryRequest::CallFunction {
                account_id: staking_pool.parse()?,
                method_name: "get_account_unstaked_balance".to_string(),
                args: FunctionArgs::from(args.to_vec()),
            },
            BlockReference::BlockId(Height(block_id)),
        )
        .await;

        match result {
            Ok(v) => Ok(serde_json::from_slice::<String>(&v)?.parse::<u128>()?),
            Err(e) => {
                bail!(
                    "Error getting staking details for staking pool: {}, error: {:?}",
                    staking_pool,
                    e
                );
            }
        }
    }

    async fn get_staked_balance(
        &self,
        staking_pool: &str,
        args: &[u8],
        block_id: u64,
    ) -> Result<u128> {
        self.archival_rate_limiter.write().await.until_ready().await;
        let result = view_function_call(
            &self.near_client,
            QueryRequest::CallFunction {
                account_id: staking_pool.parse()?,
                method_name: "get_account_staked_balance".to_string(),
                args: FunctionArgs::from(args.to_vec()),
            },
            BlockReference::BlockId(Height(block_id)),
        )
        .await;

        match result {
            Ok(v) => Ok(serde_json::from_slice::<String>(&v)?.parse::<u128>()?),
            Err(e) => {
                bail!(
                    "Error getting staking details for staking pool: {}, error: {:?}",
                    staking_pool,
                    e
                );
            }
        }
    }

    async fn is_unstaked_balance_available(
        &self,
        staking_pool: &str,
        args: &[u8],
        block_id: u64,
    ) -> Result<bool> {
        self.archival_rate_limiter.write().await.until_ready().await;
        let result = view_function_call(
            &self.near_client,
            QueryRequest::CallFunction {
                account_id: staking_pool.parse()?,
                method_name: "is_account_unstaked_balance_available".to_string(),
                args: FunctionArgs::from(args.to_vec()),
            },
            BlockReference::BlockId(Height(block_id)),
        )
        .await;

        match result {
            Ok(v) => Ok(serde_json::from_slice::<bool>(&v)?),
            Err(e) => {
                bail!(
                    "Error getting staking details for staking pool: {}, error: {:?}",
                    staking_pool,
                    e
                );
            }
        }
    }

    pub async fn get_locked_amount(&self, lockup: &str, block_id: u64) -> Result<u128> {
        self.archival_rate_limiter.write().await.until_ready().await;
        let args = json!({}).to_string().into_bytes();
        let result = view_function_call(
            &self.near_client,
            QueryRequest::CallFunction {
                account_id: lockup.parse()?,
                method_name: "get_locked_amount".to_string(),
                args: FunctionArgs::from(args.to_vec()),
            },
            BlockReference::BlockId(Height(block_id)),
        )
        .await;

        match result {
            Ok(v) => Ok(serde_json::from_slice::<String>(&v)?.parse::<u128>()?),
            Err(e) => {
                bail!(
                    "Error getting locked amount for lockup: {}, error: {:?}",
                    lockup,
                    e
                );
            }
        }
    }

    pub async fn get_liquid_owners_balance(&self, lockup: &str, block_id: u64) -> Result<u128> {
        self.archival_rate_limiter.write().await.until_ready().await;
        let args = json!({}).to_string().into_bytes();
        let result = view_function_call(
            &self.near_client,
            QueryRequest::CallFunction {
                account_id: lockup.parse()?,
                method_name: "get_liquid_owners_balance".to_string(),
                args: FunctionArgs::from(args.to_vec()),
            },
            BlockReference::BlockId(Height(block_id)),
        )
        .await;

        match result {
            Ok(v) => Ok(serde_json::from_slice::<String>(&v)?.parse::<u128>()?),
            Err(e) => {
                bail!(
                    "Error get_liquid_owners_balance for lockup: {}, error: {:?}",
                    lockup,
                    e
                );
            }
        }
    }
}

pub async fn view_function_call(
    client: &JsonRpcClient,
    request: QueryRequest,
    block_reference: BlockReference,
) -> anyhow::Result<Vec<u8>> {
    let RpcQueryResponse { kind, .. } = match client
        .call(RpcQueryRequest {
            block_reference,
            request: request.clone(),
        })
        .await
    {
        Ok(v) => v,
        Err(e) => {
            bail!(
                "Error calling view_function_call: {:?}, request: {:?}",
                e,
                request
            );
        }
    };

    match kind {
        QueryResponseKind::CallResult(CallResult { result, .. }) => Ok(result),
        _ => {
            eprintln!("Received unexpected kind: {:?}", kind); // <-- Add this line
            bail!("Unexpected response kind");
        }
    }
}
