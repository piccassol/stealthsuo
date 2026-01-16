mod cli;
mod commands;
mod config;
mod crypto;
mod error;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create(args) => commands::create::execute(args).await,
        Commands::Configure(args) => commands::configure::execute(args).await,
        Commands::Distribute(args) => commands::distribute::execute(args).await,
        Commands::Balance(args) => commands::balance::execute(args).await,
    }
}
