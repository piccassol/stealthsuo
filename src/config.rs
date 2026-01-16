use anyhow::{Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::signature::Keypair;
use std::fs;
use std::path::Path;

pub fn load_keypair(path: &Path) -> Result<Keypair> {
    let path = expand_tilde(path);
    let data = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read keypair from {}", path.display()))?;

    let bytes: Vec<u8> = serde_json::from_str(&data)
        .with_context(|| "Invalid keypair format - expected JSON array of bytes")?;

    Keypair::try_from(bytes.as_slice()).map_err(|_| anyhow::anyhow!("Invalid keypair bytes"))
}

pub fn create_rpc_client(url: &str) -> RpcClient {
    RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed())
}

fn expand_tilde(path: &Path) -> std::path::PathBuf {
    if path.starts_with("~") {
        if let Some(home) = dirs::home_dir() {
            return home.join(path.strip_prefix("~").unwrap());
        }
    }
    path.to_path_buf()
}

pub fn expand_path(path: &Path) -> std::path::PathBuf {
    expand_tilde(path)
}
