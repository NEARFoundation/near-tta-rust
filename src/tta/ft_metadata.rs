use anyhow::{bail, Result};
use governor::{Quota, RateLimiter};
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
};
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

#[derive(Debug)]
pub struct FtService {
    pub ft_metadata_cache: HashMap<String, FtMetadata>,
    pub ft_balances_cache: LruCache<CompositeKey, f64>,
    pub near_client: JsonRpcClient,
    pub archival_rate_limiter: RateLimiter<
        governor::state::NotKeyed,
        governor::state::InMemoryState,
        governor::clock::QuantaClock,
        governor::middleware::NoOpMiddleware<governor::clock::QuantaInstant>,
    >,
}

impl FtService {
    pub fn new(near_client: JsonRpcClient) -> Self {
        FtService {
            ft_metadata_cache: HashMap::new(),
            ft_balances_cache: LruCache::new(NonZeroUsize::new(1_000_000).unwrap()),
            near_client,
            archival_rate_limiter: RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(9u32).unwrap(),
            )),
        }
    }

    pub async fn assert_ft_metadata(&mut self, ft_token_id: &str) -> Result<FtMetadata> {
        if !self.ft_metadata_cache.contains_key(ft_token_id) {
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

            self.ft_metadata_cache.insert(ft_token_id.to_string(), v);
        }

        match self.ft_metadata_cache.get(ft_token_id) {
            Some(v) => Ok(v.clone()),
            None => bail!("ft_metadata not found"),
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn assert_ft_balance(
        &mut self,
        token_id: &String,
        account_id: &String,
        block_id: u64,
    ) -> Result<f64> {
        if self.ft_balances_cache.contains(&CompositeKey {
            block_id,
            account_id: account_id.clone(),
            token_id: token_id.clone(),
        }) {
            return Ok(*self
                .ft_balances_cache
                .get(&CompositeKey {
                    block_id,
                    account_id: account_id.clone(),
                    token_id: token_id.clone(),
                })
                .unwrap());
        }

        self.archival_rate_limiter.until_ready().await;

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

        let amount: String = serde_json::from_slice(&result)?;
        let amount = amount.parse::<u128>()?;
        let amount = safe_divide_u128(amount, metadata.decimals as u32);

        self.ft_balances_cache.put(
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
