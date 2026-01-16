use anyhow::{anyhow, Result};
use solana_sdk::signature::Keypair;
use spl_token_2022::solana_zk_sdk::encryption::{
    auth_encryption::AeKey,
    elgamal::{ElGamalKeypair, ElGamalPubkey},
};

pub struct ConfidentialKeys {
    pub elgamal_keypair: ElGamalKeypair,
    pub aes_key: AeKey,
}

impl ConfidentialKeys {
    pub fn derive_from_keypair(keypair: &Keypair) -> Result<Self> {
        let elgamal_keypair = ElGamalKeypair::new_from_signer(keypair, b"elgamal")
            .map_err(|e| anyhow!("Failed to derive ElGamal keypair: {}", e))?;

        let aes_key = AeKey::new_from_signer(keypair, b"aes")
            .map_err(|e| anyhow!("Failed to derive AES key: {}", e))?;

        Ok(Self {
            elgamal_keypair,
            aes_key,
        })
    }

    #[allow(dead_code)]
    pub fn elgamal_pubkey(&self) -> ElGamalPubkey {
        *self.elgamal_keypair.pubkey()
    }
}

pub fn parse_elgamal_pubkey(s: &str) -> Result<ElGamalPubkey> {
    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|_| anyhow!("Invalid base58 encoding for ElGamal pubkey"))?;

    if bytes.len() != 32 {
        return Err(anyhow!("ElGamal pubkey must be 32 bytes, got {}", bytes.len()));
    }

    ElGamalPubkey::try_from(bytes.as_slice())
        .map_err(|_| anyhow!("Invalid ElGamal pubkey bytes"))
}
