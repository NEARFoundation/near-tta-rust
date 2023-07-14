use sha2::{Digest, Sha256};

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
