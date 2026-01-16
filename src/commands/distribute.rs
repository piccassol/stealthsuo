use crate::cli::DistributeArgs;
use crate::config::{expand_path, load_keypair};
use crate::crypto::ConfidentialKeys;
use anyhow::{anyhow, Context, Result};
use csv::Reader;
use serde::Deserialize;
use solana_client::nonblocking::rpc_client::RpcClient as NonblockingRpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use spl_token_2022::solana_zk_sdk::encryption::{
    auth_encryption::AeCiphertext,
    elgamal::ElGamalPubkey,
};
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token_2022::extension::{
    confidential_transfer::{ConfidentialTransferAccount, ConfidentialTransferMint},
    BaseStateWithExtensions, StateWithExtensions,
};
use spl_token_client::{
    client::{ProgramRpcClient, ProgramRpcClientSendTransaction},
    token::{ProofAccountWithCiphertext, Token},
};
use spl_token_confidential_transfer_proof_generation::transfer::transfer_split_proof_data;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct Recipient {
    wallet: String,
    amount: u64,
}

pub async fn execute(args: DistributeArgs) -> Result<()> {
    let keypair = load_keypair(&expand_path(&args.keypair))?;

    let rpc_client = Arc::new(NonblockingRpcClient::new_with_commitment(
        args.rpc.clone(),
        CommitmentConfig::confirmed(),
    ));

    let program_client = Arc::new(ProgramRpcClient::new(
        rpc_client.clone(),
        ProgramRpcClientSendTransaction,
    ));

    let recipients = parse_recipients(&expand_path(&args.recipients))?;
    println!("Distributing tokens to {} recipients...", recipients.len());

    let authority_keys = ConfidentialKeys::derive_from_keypair(&keypair)?;

    let source_ata = get_associated_token_address_with_program_id(
        &keypair.pubkey(),
        &args.mint,
        &spl_token_2022::id(),
    );

    // Get current balance
    let source_account_data = rpc_client.get_account(&source_ata).await?;
    let source_state =
        StateWithExtensions::<spl_token_2022::state::Account>::unpack(&source_account_data.data)?;
    let source_ct = source_state.get_extension::<ConfidentialTransferAccount>()?;

    let source_decryptable: AeCiphertext = source_ct.decryptable_available_balance.try_into()
        .map_err(|_| anyhow!("Invalid source decryptable balance"))?;
    let mut current_balance = authority_keys.aes_key
        .decrypt(&source_decryptable)
        .ok_or_else(|| anyhow!("Failed to decrypt source balance"))?;

    println!("Current confidential balance: {}", current_balance);

    // Get decimals and auditor pubkey from the mint
    let mint_account_data = rpc_client.get_account(&args.mint).await?;
    let mint_state = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_account_data.data)?;
    let decimals = mint_state.base.decimals;
    let ct_mint = mint_state.get_extension::<ConfidentialTransferMint>()?;
    let auditor_elgamal_pubkey: Option<ElGamalPubkey> = Option::<spl_token_2022::solana_zk_sdk::encryption::pod::elgamal::PodElGamalPubkey>::from(ct_mint.auditor_elgamal_pubkey)
        .and_then(|p| ElGamalPubkey::try_from(p).ok());

    let token = Token::new(
        program_client.clone(),
        &spl_token_2022::id(),
        &args.mint,
        Some(decimals),
        Arc::new(keypair.insecure_clone()),
    );

    for (i, recipient) in recipients.iter().enumerate() {
        let wallet = Pubkey::from_str(&recipient.wallet)
            .with_context(|| format!("Invalid wallet address: {}", recipient.wallet))?;

        if recipient.amount > current_balance {
            return Err(anyhow!(
                "Insufficient balance for recipient {}. Need {}, have {}",
                wallet,
                recipient.amount,
                current_balance
            ));
        }

        println!(
            "[{}/{}] Transferring {} to {}",
            i + 1,
            recipients.len(),
            recipient.amount,
            wallet
        );

        let dest_ata = get_associated_token_address_with_program_id(
            &wallet,
            &args.mint,
            &spl_token_2022::id(),
        );

        // Create destination ATA if needed
        if rpc_client.get_account(&dest_ata).await.is_err() {
            token.create_associated_token_account(&wallet).await
                .map_err(|e| anyhow!("Failed to create recipient token account: {}", e))?;
            println!("  Created token account for recipient");
        }

        // Get destination's ElGamal pubkey
        let dest_account_data = rpc_client.get_account(&dest_ata).await?;
        let dest_state = StateWithExtensions::<spl_token_2022::state::Account>::unpack(&dest_account_data.data)?;
        let dest_ct = dest_state.get_extension::<ConfidentialTransferAccount>()?;
        let dest_elgamal_pubkey: ElGamalPubkey = dest_ct.elgamal_pubkey.try_into()
            .map_err(|_| anyhow!("Invalid destination ElGamal pubkey"))?;

        // Get fresh source account state for proof generation
        let source_account_data = rpc_client.get_account(&source_ata).await?;
        let source_state = StateWithExtensions::<spl_token_2022::state::Account>::unpack(&source_account_data.data)?;
        let source_ct = source_state.get_extension::<ConfidentialTransferAccount>()?;

        let source_available_balance = source_ct.available_balance.try_into()
            .map_err(|_| anyhow!("Invalid source available balance"))?;
        let source_decryptable: AeCiphertext = source_ct.decryptable_available_balance.try_into()
            .map_err(|_| anyhow!("Invalid source decryptable balance"))?;

        // Generate split proof data
        println!("  Generating proofs...");
        let proof_data = transfer_split_proof_data(
            &source_available_balance,
            &source_decryptable,
            recipient.amount,
            &authority_keys.elgamal_keypair,
            &authority_keys.aes_key,
            &dest_elgamal_pubkey,
            auditor_elgamal_pubkey.as_ref(),
        ).map_err(|e| anyhow!("Failed to generate proof data: {:?}", e))?;

        // Create context state accounts for proofs (split mode for large proofs)
        let equality_proof_keypair = Keypair::new();
        let ciphertext_validity_proof_keypair = Keypair::new();
        let range_proof_keypair = Keypair::new();

        println!("  Creating proof context accounts...");

        // Create equality proof context state (small enough for single tx)
        token.confidential_transfer_create_context_state_account(
            &equality_proof_keypair.pubkey(),
            &keypair.pubkey(),
            &proof_data.equality_proof_data,
            false,
            &[&keypair, &equality_proof_keypair],
        ).await
        .map_err(|e| anyhow!("Failed to create equality proof account: {}", e))?;

        // Create ciphertext validity proof context state (use split for safety)
        token.confidential_transfer_create_context_state_account(
            &ciphertext_validity_proof_keypair.pubkey(),
            &keypair.pubkey(),
            &proof_data.ciphertext_validity_proof_data_with_ciphertext.proof_data,
            true, // split account creation and proof verification
            &[&keypair, &ciphertext_validity_proof_keypair],
        ).await
        .map_err(|e| anyhow!("Failed to create ciphertext validity proof account: {}", e))?;

        // Create range proof context state (split mode - range proofs are large)
        token.confidential_transfer_create_context_state_account(
            &range_proof_keypair.pubkey(),
            &keypair.pubkey(),
            &proof_data.range_proof_data,
            true, // split account creation and proof verification
            &[&keypair, &range_proof_keypair],
        ).await
        .map_err(|e| anyhow!("Failed to create range proof account: {}", e))?;

        // Execute transfer with proof accounts
        println!("  Executing transfer...");
        let ciphertext_validity_proof_with_ciphertext = ProofAccountWithCiphertext {
            context_state_account: ciphertext_validity_proof_keypair.pubkey(),
            ciphertext_lo: proof_data.ciphertext_validity_proof_data_with_ciphertext.ciphertext_lo.into(),
            ciphertext_hi: proof_data.ciphertext_validity_proof_data_with_ciphertext.ciphertext_hi.into(),
        };

        token.confidential_transfer_transfer(
            &source_ata,
            &dest_ata,
            &keypair.pubkey(),
            Some(&equality_proof_keypair.pubkey()),
            Some(&ciphertext_validity_proof_with_ciphertext),
            Some(&range_proof_keypair.pubkey()),
            recipient.amount,
            None, // account_info
            &authority_keys.elgamal_keypair,
            &authority_keys.aes_key,
            &dest_elgamal_pubkey,
            auditor_elgamal_pubkey.as_ref(),
            &[&keypair],
        ).await
        .map_err(|e| anyhow!("Failed to execute confidential transfer: {}", e))?;

        // Close context state accounts to recover rent
        println!("  Cleaning up proof accounts...");
        token.confidential_transfer_close_context_state_account(
            &equality_proof_keypair.pubkey(),
            &source_ata,
            &keypair.pubkey(),
            &[&keypair],
        ).await
        .map_err(|e| anyhow!("Failed to close equality proof account: {}", e))?;

        token.confidential_transfer_close_context_state_account(
            &ciphertext_validity_proof_keypair.pubkey(),
            &source_ata,
            &keypair.pubkey(),
            &[&keypair],
        ).await
        .map_err(|e| anyhow!("Failed to close ciphertext validity proof account: {}", e))?;

        token.confidential_transfer_close_context_state_account(
            &range_proof_keypair.pubkey(),
            &source_ata,
            &keypair.pubkey(),
            &[&keypair],
        ).await
        .map_err(|e| anyhow!("Failed to close range proof account: {}", e))?;

        current_balance -= recipient.amount;
        println!("  Transfer complete");
    }

    println!("\nDistribution complete.");
    println!("Remaining balance: {}", current_balance);
    Ok(())
}

fn parse_recipients(path: &std::path::Path) -> Result<Vec<Recipient>> {
    let mut reader = Reader::from_path(path)
        .with_context(|| format!("Failed to read CSV file: {}", path.display()))?;

    let mut recipients = Vec::new();
    for result in reader.deserialize() {
        let recipient: Recipient = result.context("Failed to parse recipient row")?;
        recipients.push(recipient);
    }

    if recipients.is_empty() {
        anyhow::bail!("No recipients found in CSV file");
    }

    Ok(recipients)
}
