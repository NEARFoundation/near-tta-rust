use std::collections::HashSet;

use anyhow::Result;
use governor::{clock, state, RateLimiter};
use hyper::{Body, Response};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub type RateLim = RateLimiter<
    state::NotKeyed,
    state::InMemoryState,
    clock::QuantaClock,
    governor::middleware::NoOpMiddleware<clock::QuantaInstant>,
>;

// Extract accounts,
// returns: account, is lockup, master account
pub fn get_accounts_and_lockups(accounts: &str) -> HashSet<(String, Option<String>)> {
    let mut accounts: HashSet<(String, Option<String>)> = accounts
        .split(',')
        .map(String::from)
        .filter(|account| account != "near" && account != "system")
        .map(|account| (account, None))
        .collect();

    for a in accounts.clone() {
        if a.0.ends_with(".lockup.near") {
            continue;
        }
        let lockup_account = get_associated_lockup(&a.0, "near");
        accounts.insert((lockup_account, Some(a.0.clone())));
    }

    accounts
}

// Consolidate results and return a Response
pub fn results_to_response<T: Serialize>(results: Vec<T>) -> Result<Response<Body>, csv::Error> {
    let mut wtr = csv::Writer::from_writer(Vec::new());
    for row in results {
        wtr.serialize(row)?;
    }
    wtr.flush()?;
    Ok(Response::builder()
        .header("Content-Type", "text/csv")
        .body(Body::from(wtr.into_inner().unwrap()))
        .unwrap())
}

pub fn get_associated_lockup(account_id: &str, master_account_id: &str) -> String {
    format!(
        "{}.lockup.{}",
        &sha256(account_id)[0..40],
        master_account_id
    )
}

fn sha256(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}
