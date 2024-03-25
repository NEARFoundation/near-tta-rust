mod models;

use std::{collections::HashMap, num::NonZeroU32, sync::Arc};

use anyhow::bail;
use governor::{Quota, RateLimiter};
use tokio::sync::RwLock;
use tracing::{error, info};
use tta_rust::RateLim;

use crate::kitwallet::models::FastNearFT;

#[derive(Clone)]
pub struct KitWallet {
    rate_limiter: Arc<RwLock<RateLim>>,
    client: reqwest::Client,
    cache: Arc<RwLock<HashMap<String, (i64, Vec<String>)>>>,
}

impl Default for KitWallet {
    fn default() -> Self {
        Self::new()
    }
}

impl KitWallet {
    pub fn new() -> Self {
        Self {
            rate_limiter: Arc::new(RwLock::new(RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(4u32).unwrap(),
            )))),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // TODO(plg): expire the cache.
    pub async fn get_likely_tokens(&self, account: String) -> anyhow::Result<Vec<String>> {
        let cache_read = self.cache.read().await;

        if let Some(likely_tokens) = cache_read.get(&account) {
            // Check if the cache is expired
            if chrono::Utc::now().timestamp() - likely_tokens.0 < 60 {
                return Ok(likely_tokens.1.clone());
            }
        }

        drop(cache_read); // Release the read lock

        // Now, only here do we apply the rate limiter
        self.rate_limiter.read().await.until_ready().await;

        info!(
            "Account {} likely tokens not cached, fetching from API",
            account
        );
        // https://api.fastnear.com/v1/account/here.near/ft
        let likely_tokens = self
            .client
            .get(format!(
                "https://api.fastnear.com/v1/account/{}/ft",
                account
            ))
            .send()
            .await?
            .json::<FastNearFT>()
            .await?;

        // Insert the result into the cache
        let mut cache_write = self.cache.write().await;
        cache_write.insert(
            account.clone(),
            (
                chrono::Utc::now().timestamp(),
                likely_tokens
                    .tokens
                    .iter()
                    .map(|t| t.contract_id.clone())
                    .collect(),
            ),
        );

        Ok(cache_write.get(&account).unwrap().1.clone())
    }

    // get all in parallel
    pub async fn get_likely_tokens_for_accounts(
        &self,
        accounts: Vec<String>,
    ) -> anyhow::Result<HashMap<String, Vec<String>>> {
        let mut tasks = Vec::new();
        for account in accounts {
            let account = account.clone();
            let self_clone = self.clone();
            tasks.push(tokio::spawn(async move {
                let likely_tokens = match self_clone.get_likely_tokens(account.clone()).await {
                    Ok(likely_tokens) => likely_tokens,
                    Err(e) => {
                        error!(
                            "Error fetching likely tokens for account {}: {}",
                            account, e
                        );
                        bail!(
                            "Error fetching likely tokens for account {}: {}",
                            account,
                            e
                        )
                    }
                };
                anyhow::Ok((account, likely_tokens))
            }));
        }

        let mut likely_tokens_for_accounts = HashMap::new();
        for task in tasks {
            let (account, likely_tokens) = match task.await? {
                Ok(a) => a,
                Err(err) => {
                    error!("Error fetching likely tokens: {}", err);
                    continue;
                }
            };
            likely_tokens_for_accounts.insert(account, likely_tokens);
        }

        Ok(likely_tokens_for_accounts)
    }
}
