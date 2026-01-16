use clap::{Parser, Subcommand};
use solana_sdk::pubkey::Pubkey;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "stealth-launch")]
#[command(about = "Private token creation on Solana using Token2022 confidential extensions")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new token with confidential transfer extensions
    Create(CreateArgs),
    /// Configure a wallet's token account for confidential transfers
    Configure(ConfigureArgs),
    /// Distribute tokens via confidential transfers
    Distribute(DistributeArgs),
    /// Check confidential balance for a wallet
    Balance(BalanceArgs),
}

#[derive(Parser)]
pub struct CreateArgs {
    /// Token name
    #[arg(long)]
    pub name: String,

    /// Token symbol
    #[arg(long)]
    pub symbol: String,

    /// Initial supply (hidden from public)
    #[arg(long)]
    pub supply: u64,

    /// Token decimals
    #[arg(long, default_value = "9")]
    pub decimals: u8,

    /// Optional auditor ElGamal pubkey for compliance
    #[arg(long)]
    pub auditor: Option<String>,

    /// Path to payer keypair
    #[arg(long, default_value = "~/.config/solana/id.json")]
    pub keypair: PathBuf,

    /// RPC endpoint
    #[arg(long, default_value = "https://zk-edge.surfnet.dev:8899")]
    pub rpc: String,
}

#[derive(Parser)]
pub struct ConfigureArgs {
    /// Mint address
    #[arg(long)]
    pub mint: Pubkey,

    /// Owner keypair (the wallet that will own the configured account)
    #[arg(long)]
    pub owner: PathBuf,

    /// Fee payer keypair (defaults to owner)
    #[arg(long)]
    pub fee_payer: Option<PathBuf>,

    /// RPC endpoint
    #[arg(long, default_value = "https://zk-edge.surfnet.dev:8899")]
    pub rpc: String,
}

#[derive(Parser)]
pub struct DistributeArgs {
    /// Mint address
    #[arg(long)]
    pub mint: Pubkey,

    /// CSV file with wallet,amount rows
    #[arg(long)]
    pub recipients: PathBuf,

    /// Payer/authority keypair
    #[arg(long, default_value = "~/.config/solana/id.json")]
    pub keypair: PathBuf,

    /// RPC endpoint
    #[arg(long, default_value = "https://zk-edge.surfnet.dev:8899")]
    pub rpc: String,
}

#[derive(Parser)]
pub struct BalanceArgs {
    /// Mint address
    #[arg(long)]
    pub mint: Pubkey,

    /// Wallet to check
    #[arg(long)]
    pub wallet: Pubkey,

    /// Owner keypair (needed to decrypt)
    #[arg(long, default_value = "~/.config/solana/id.json")]
    pub keypair: PathBuf,

    /// RPC endpoint
    #[arg(long, default_value = "https://zk-edge.surfnet.dev:8899")]
    pub rpc: String,
}
