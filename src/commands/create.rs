use crate::cli::CreateArgs;
use crate::config::{create_rpc_client, expand_path, load_keypair};
use crate::crypto::{parse_elgamal_pubkey, ConfidentialKeys};
use anyhow::{anyhow, Context, Result};
use solana_client::nonblocking::rpc_client::RpcClient as NonblockingRpcClient;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
#[allow(deprecated)]
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;
use spl_associated_token_account::{
    get_associated_token_address_with_program_id,
    instruction::create_associated_token_account,
};
use spl_token_2022::{
    extension::{
        confidential_transfer::instruction::initialize_mint as init_ct_mint,
        ExtensionType,
    },
    instruction::{initialize_mint, reallocate},
    solana_zk_sdk::encryption::{
        elgamal::ElGamalPubkey,
        pod::elgamal::PodElGamalPubkey,
    },
    state::Mint,
};
use spl_token_client::{
    client::{ProgramRpcClient, ProgramRpcClientSendTransaction},
    token::Token,
};
use std::sync::Arc;

pub async fn execute(args: CreateArgs) -> Result<()> {
    let keypair = load_keypair(&expand_path(&args.keypair))?;
    let rpc = create_rpc_client(&args.rpc);

    println!("Creating confidential token mint...");
    println!("  Name: {}", args.name);
    println!("  Symbol: {}", args.symbol);
    println!("  Supply: {} (hidden)", args.supply);
    println!("  Decimals: {}", args.decimals);

    let auditor_elgamal_pubkey = match &args.auditor {
        Some(s) => Some(parse_elgamal_pubkey(s)?),
        None => None,
    };

    let mint_keypair = Keypair::new();
    let mint_pubkey = mint_keypair.pubkey();

    let authority_keys = ConfidentialKeys::derive_from_keypair(&keypair)?;

    create_confidential_mint(
        &rpc,
        &keypair,
        &mint_keypair,
        auditor_elgamal_pubkey.as_ref(),
        args.decimals,
    )?;

    println!("Mint created: {}", mint_pubkey);

    let ata = get_associated_token_address_with_program_id(
        &keypair.pubkey(),
        &mint_pubkey,
        &spl_token_2022::id(),
    );

    create_and_configure_ata(&args.rpc, &keypair, &mint_pubkey, &authority_keys).await?;
    println!("Token account created and configured: {}", ata);

    if args.supply > 0 {
        mint_and_deposit(
            &args.rpc,
            &keypair,
            &mint_pubkey,
            &ata,
            &authority_keys,
            args.supply,
            args.decimals,
        ).await?;
        println!("Minted and deposited {} tokens to confidential balance", args.supply);
    }

    println!("\nToken launch complete.");
    println!("Mint address: {}", mint_pubkey);
    println!("Authority: {}", keypair.pubkey());

    Ok(())
}

fn create_confidential_mint(
    rpc: &RpcClient,
    payer: &Keypair,
    mint_keypair: &Keypair,
    auditor_elgamal_pubkey: Option<&ElGamalPubkey>,
    decimals: u8,
) -> Result<()> {
    let extensions = vec![ExtensionType::ConfidentialTransferMint];

    let space = ExtensionType::try_calculate_account_len::<Mint>(&extensions)?;
    let rent = rpc.get_minimum_balance_for_rent_exemption(space)?;

    let create_account_ix = system_instruction::create_account(
        &payer.pubkey(),
        &mint_keypair.pubkey(),
        rent,
        space as u64,
        &spl_token_2022::id(),
    );

    let auditor_pod: Option<PodElGamalPubkey> = auditor_elgamal_pubkey.map(|p| (*p).into());

    let init_ct_ix = init_ct_mint(
        &spl_token_2022::id(),
        &mint_keypair.pubkey(),
        Some(payer.pubkey()),
        true,
        auditor_pod,
    )?;

    let init_mint_ix = initialize_mint(
        &spl_token_2022::id(),
        &mint_keypair.pubkey(),
        &payer.pubkey(),
        None,
        decimals,
    )?;

    let recent_blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[create_account_ix, init_ct_ix, init_mint_ix],
        Some(&payer.pubkey()),
        &[payer, mint_keypair],
        recent_blockhash,
    );

    rpc.send_and_confirm_transaction_with_spinner(&tx)
        .context("Failed to create mint")?;

    Ok(())
}

async fn create_and_configure_ata(
    rpc_url: &str,
    owner: &Keypair,
    mint: &Pubkey,
    owner_keys: &ConfidentialKeys,
) -> Result<()> {
    let rpc = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let ata = get_associated_token_address_with_program_id(
        &owner.pubkey(),
        mint,
        &spl_token_2022::id(),
    );

    // Create the ATA
    let create_ata_ix = create_associated_token_account(
        &owner.pubkey(),
        &owner.pubkey(),
        mint,
        &spl_token_2022::id(),
    );

    let recent_blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[create_ata_ix],
        Some(&owner.pubkey()),
        &[owner],
        recent_blockhash,
    );

    rpc.send_and_confirm_transaction_with_spinner(&tx)
        .context("Failed to create token account")?;

    // Reallocate for confidential transfer extension
    let reallocate_ix = reallocate(
        &spl_token_2022::id(),
        &ata,
        &owner.pubkey(),
        &owner.pubkey(),
        &[&owner.pubkey()],
        &[ExtensionType::ConfidentialTransferAccount],
    )?;

    let recent_blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[reallocate_ix],
        Some(&owner.pubkey()),
        &[owner],
        recent_blockhash,
    );

    rpc.send_and_confirm_transaction_with_spinner(&tx)
        .context("Failed to reallocate token account for confidential transfer")?;

    // Use Token client to configure the account - it handles proof generation
    let rpc_client = Arc::new(NonblockingRpcClient::new_with_commitment(
        rpc_url.to_string(),
        CommitmentConfig::confirmed(),
    ));

    let program_client = Arc::new(ProgramRpcClient::new(
        rpc_client.clone(),
        ProgramRpcClientSendTransaction,
    ));

    let token = Token::new(
        program_client,
        &spl_token_2022::id(),
        mint,
        None,
        Arc::new(owner.insecure_clone()),
    );

    token
        .confidential_transfer_configure_token_account(
            &ata,
            &owner.pubkey(),
            None, // context state account
            None, // maximum pending balance credit counter
            &owner_keys.elgamal_keypair,
            &owner_keys.aes_key,
            &[owner],
        )
        .await
        .map_err(|e| anyhow!("Failed to configure confidential transfer account: {}", e))?;

    Ok(())
}

async fn mint_and_deposit(
    rpc_url: &str,
    authority: &Keypair,
    mint: &Pubkey,
    destination: &Pubkey,
    authority_keys: &ConfidentialKeys,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    let rpc_client = Arc::new(NonblockingRpcClient::new_with_commitment(
        rpc_url.to_string(),
        CommitmentConfig::confirmed(),
    ));

    let program_client = Arc::new(ProgramRpcClient::new(
        rpc_client.clone(),
        ProgramRpcClientSendTransaction,
    ));

    let token = Token::new(
        program_client,
        &spl_token_2022::id(),
        mint,
        Some(decimals),
        Arc::new(authority.insecure_clone()),
    );

    token
        .mint_to(destination, &authority.pubkey(), amount, &[authority])
        .await
        .map_err(|e| anyhow!("Failed to mint tokens: {}", e))?;

    token
        .confidential_transfer_deposit(
            destination,
            &authority.pubkey(),
            amount,
            decimals,
            &[authority],
        )
        .await
        .map_err(|e| anyhow!("Failed to deposit to confidential balance: {}", e))?;

    token
        .confidential_transfer_apply_pending_balance(
            destination,
            &authority.pubkey(),
            None,
            authority_keys.elgamal_keypair.secret(),
            &authority_keys.aes_key,
            &[authority],
        )
        .await
        .map_err(|e| anyhow!("Failed to apply pending balance: {}", e))?;

    Ok(())
}
