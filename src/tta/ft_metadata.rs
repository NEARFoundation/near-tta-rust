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
    views::{CallResult, QueryRequest},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
};
use tokio::sync::RwLock;
use tracing::{debug, warn};

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
        debug!("Getting ft_balance");

        let w = self.archival_rate_limiter.clone();
        let lock = w.write().await;
        debug!("Waiting for rate limiter");

        lock.until_ready().await;

        debug!("Rate limiter gave green light");

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
            warn!("Found ft_balance in cache");
            let w = self.ft_balances_cache.clone();
            let mut w = w.write().await;
            return Ok(*w
                .get(&CompositeKey {
                    block_id,
                    account_id: account_id.clone(),
                    token_id: token_id.clone(),
                })
                .unwrap());
        }

        let metadata = self.assert_ft_metadata(token_id).await.unwrap();

        let args = json!({ "account_id": account_id }).to_string().into_bytes();

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

        drop(lock);

        let amount: String = serde_json::from_slice(&result)?;
        let amount = amount.parse::<u128>()?;
        let amount = safe_divide_u128(amount, metadata.decimals as u32);

        debug!("Got ft_balance amount: {}", amount);

        let w = self.ft_balances_cache.clone();
        let mut w = w.write().await;
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

    let QueryResponseKind::CallResult(CallResult{result, ..}) = kind else {
      bail!("Unexpected response kind");
    };

    Ok(result)
}
