use crate::cli::ConfigureArgs;
use crate::config::{expand_path, load_keypair};
use crate::crypto::ConfidentialKeys;
use anyhow::{anyhow, Context, Result};
use solana_client::nonblocking::rpc_client::RpcClient as NonblockingRpcClient;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account::{
    get_associated_token_address_with_program_id,
    instruction::create_associated_token_account,
};
use spl_token_2022::{
    extension::ExtensionType,
    instruction::reallocate,
};
use spl_token_client::{
    client::{ProgramRpcClient, ProgramRpcClientSendTransaction},
    token::Token,
};
use std::sync::Arc;

pub async fn execute(args: ConfigureArgs) -> Result<()> {
    let owner = load_keypair(&expand_path(&args.owner))?;
    let fee_payer = match &args.fee_payer {
        Some(path) => load_keypair(&expand_path(path))?,
        None => owner.insecure_clone(),
    };

    println!("Configuring confidential transfer account...");
    println!("  Mint: {}", args.mint);
    println!("  Owner: {}", owner.pubkey());

    let owner_keys = ConfidentialKeys::derive_from_keypair(&owner)?;

    let rpc = RpcClient::new_with_commitment(args.rpc.clone(), CommitmentConfig::confirmed());

    let ata = get_associated_token_address_with_program_id(
        &owner.pubkey(),
        &args.mint,
        &spl_token_2022::id(),
    );

    // Check if ATA exists
    let ata_exists = rpc.get_account(&ata).is_ok();

    if !ata_exists {
        // Create the ATA
        println!("Creating token account...");
        let create_ata_ix = create_associated_token_account(
            &fee_payer.pubkey(),
            &owner.pubkey(),
            &args.mint,
            &spl_token_2022::id(),
        );

        let recent_blockhash = rpc.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&fee_payer.pubkey()),
            &[&fee_payer],
            recent_blockhash,
        );

        rpc.send_and_confirm_transaction_with_spinner(&tx)
            .context("Failed to create token account")?;
    } else {
        println!("Token account already exists");
    }

    // Reallocate for confidential transfer extension
    println!("Reallocating for confidential transfer...");
    let reallocate_ix = reallocate(
        &spl_token_2022::id(),
        &ata,
        &fee_payer.pubkey(),
        &owner.pubkey(),
        &[&owner.pubkey()],
        &[ExtensionType::ConfidentialTransferAccount],
    )?;

    let recent_blockhash = rpc.get_latest_blockhash()?;
    let signers: Vec<&Keypair> = if fee_payer.pubkey() == owner.pubkey() {
        vec![&owner]
    } else {
        vec![&fee_payer, &owner]
    };
    let tx = Transaction::new_signed_with_payer(
        &[reallocate_ix],
        Some(&fee_payer.pubkey()),
        &signers,
        recent_blockhash,
    );

    rpc.send_and_confirm_transaction_with_spinner(&tx)
        .context("Failed to reallocate token account for confidential transfer")?;

    // Use Token client to configure the account - it handles proof generation
    println!("Configuring confidential transfer...");
    let rpc_client = Arc::new(NonblockingRpcClient::new_with_commitment(
        args.rpc.clone(),
        CommitmentConfig::confirmed(),
    ));

    let program_client = Arc::new(ProgramRpcClient::new(
        rpc_client.clone(),
        ProgramRpcClientSendTransaction,
    ));

    let token = Token::new(
        program_client,
        &spl_token_2022::id(),
        &args.mint,
        None,
        Arc::new(fee_payer.insecure_clone()),
    );

    token
        .confidential_transfer_configure_token_account(
            &ata,
            &owner.pubkey(),
            None, // context state account
            None, // maximum pending balance credit counter
            &owner_keys.elgamal_keypair,
            &owner_keys.aes_key,
            &[&owner],
        )
        .await
        .map_err(|e| anyhow!("Failed to configure confidential transfer account: {}", e))?;

    println!("\nConfiguration complete.");
    println!("Token account: {}", ata);
    println!("Owner: {}", owner.pubkey());

    Ok(())
}
