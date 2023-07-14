use anyhow::Result;
use near_jsonrpc_client::JsonRpcClient;
use near_jsonrpc_primitives::types::query::{QueryResponseKind, RpcQueryRequest, RpcQueryResponse};
use near_primitives::{
    types::{
        AccountId, BlockReference,
        Finality::{self, Final},
        FunctionArgs,
    },
    views::{CallResult, QueryRequest},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tracing::info;

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

#[derive(Debug, Clone)]
pub struct FtMetadataCache {
    pub ft_metadata_cache: HashMap<String, FtMetadata>,
    pub near_client: JsonRpcClient,
}

impl FtMetadataCache {
    pub fn new(near_client: JsonRpcClient) -> Self {
        FtMetadataCache {
            ft_metadata_cache: HashMap::new(),
            near_client,
        }
    }

    pub async fn assert_ft_metadata(&mut self, ft_token_id: &str) -> Result<FtMetadata> {
        if !self.ft_metadata_cache.contains_key(ft_token_id) {
            let args = json!({}).to_string().into_bytes();

            let result = view_function_call(
                &self.near_client,
                QueryRequest::CallFunction {
                    account_id: ft_token_id.parse().unwrap(),
                    method_name: "ft_metadata".to_string(),
                    args: FunctionArgs::from(args),
                },
            )
            .await?;

            let v = serde_json::from_slice(&result)?;

            self.ft_metadata_cache.insert(ft_token_id.to_string(), v);
        }

        match self.ft_metadata_cache.get(ft_token_id) {
            Some(v) => Ok(v.clone()),
            None => anyhow::bail!("ft_metadata not found"),
        }
    }
}

pub async fn view_function_call(
    client: &JsonRpcClient,
    request: QueryRequest,
) -> anyhow::Result<Vec<u8>> {
    let RpcQueryResponse { kind, .. } = client
        .call(RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request,
        })
        .await?;

    let QueryResponseKind::CallResult(CallResult{result, ..}) = kind else {
      anyhow::bail!("Unexpected response kind");
    };

    Ok(result)
}
