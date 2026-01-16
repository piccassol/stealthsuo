use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum StealthLaunchError {
    #[error("Failed to load keypair from {path}: {source}")]
    KeypairLoad {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Invalid keypair format")]
    InvalidKeypair,

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("Token account not found for {wallet}")]
    TokenAccountNotFound { wallet: String },

    #[error("Confidential transfer not configured on this mint")]
    ConfidentialTransferNotConfigured,

    #[error("Failed to decrypt balance: {0}")]
    DecryptionFailed(String),

    #[error("Invalid recipient CSV: {0}")]
    InvalidRecipientCsv(String),

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Invalid auditor pubkey: {0}")]
    InvalidAuditorPubkey(String),
}
