use crate::cli::BalanceArgs;
use crate::config::{create_rpc_client, expand_path, load_keypair};
use crate::crypto::ConfidentialKeys;
use anyhow::{anyhow, Context, Result};
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token_2022::extension::{
    confidential_transfer::ConfidentialTransferAccount,
    BaseStateWithExtensions, StateWithExtensions,
};

pub async fn execute(args: BalanceArgs) -> Result<()> {
    let keypair = load_keypair(&expand_path(&args.keypair))?;
    let rpc = create_rpc_client(&args.rpc);

    let owner_keys = ConfidentialKeys::derive_from_keypair(&keypair)?;

    let ata = get_associated_token_address_with_program_id(
        &args.wallet,
        &args.mint,
        &spl_token_2022::id(),
    );

    let account_data = rpc
        .get_account(&ata)
        .with_context(|| format!("Token account not found for wallet {}", args.wallet))?;

    let account_state =
        StateWithExtensions::<spl_token_2022::state::Account>::unpack(&account_data.data)
            .context("Failed to unpack token account")?;

    let ct_account = account_state
        .get_extension::<ConfidentialTransferAccount>()
        .context("Confidential transfer extension not found on account")?;

    let decryptable_balance = ct_account.decryptable_available_balance.try_into()
        .map_err(|_| anyhow!("Invalid decryptable balance ciphertext"))?;

    let available_balance = owner_keys
        .aes_key
        .decrypt(&decryptable_balance)
        .ok_or_else(|| anyhow!("Failed to decrypt balance - you may not be the owner"))?;

    let pending_lo = ct_account.pending_balance_lo.try_into()
        .map_err(|_| anyhow!("Invalid pending balance lo ciphertext"))?;
    let pending_hi = ct_account.pending_balance_hi.try_into()
        .map_err(|_| anyhow!("Invalid pending balance hi ciphertext"))?;

    let pending_lo_decrypted = owner_keys.elgamal_keypair.secret().decrypt_u32(&pending_lo);
    let pending_hi_decrypted = owner_keys.elgamal_keypair.secret().decrypt_u32(&pending_hi);

    let pending_balance = match (pending_lo_decrypted, pending_hi_decrypted) {
        (Some(lo), Some(hi)) => Some((hi as u64) << 16 | lo as u64),
        _ => None,
    };

    println!("Confidential Balance for {}", args.wallet);
    println!("  Mint: {}", args.mint);
    println!("  Token Account: {}", ata);
    println!("  Available Balance: {}", available_balance);

    if let Some(pending) = pending_balance {
        if pending > 0 {
            println!("  Pending Balance: {}", pending);
            println!("  (Use apply-pending-balance to make pending balance available)");
        }
    }

    let public_balance = account_state.base.amount;
    if public_balance > 0 {
        println!("  Public Balance: {} (not confidential)", public_balance);
    }

    Ok(())
}
