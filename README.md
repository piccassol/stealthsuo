<p align="center">
  <img src="assets/logo.png" alt="Stealthsuo" width="600">
</p>

<p align="center">
  <strong>Private token launches on Solana using Token2022 confidential transfers</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/ZK-ElGamal-blueviolet?style=flat-square" alt="ZK ElGamal">
  <img src="https://img.shields.io/badge/Solana-Token2022-14F195?style=flat-square&logo=solana" alt="Token2022">
  <img src="https://img.shields.io/badge/Encryption-AES--128-orange?style=flat-square" alt="AES-128">
  <img src="https://img.shields.io/badge/Proofs-Range%20%7C%20Equality%20%7C%20Ciphertext-red?style=flat-square" alt="ZK Proofs">
  <img src="https://img.shields.io/badge/Privacy-E2E-black?style=flat-square" alt="E2E Privacy">
  <img src="https://img.shields.io/badge/Built%20by-Tetsuo-white?style=flat-square" alt="Tetsuo">
</p>

<p align="center">
  <a href="#installation">Install</a> •
  <a href="#usage">Usage</a> •
  <a href="#commands">Commands</a> •
  <a href="#how-it-works">How It Works</a>
</p>

---

## What is Stealthsuo?

Stealthsuo is a CLI tool for launching tokens with hidden supplies and private distributions on Solana. It uses Token2022's confidential transfer extensions with ElGamal encryption and zero-knowledge proofs to keep balances encrypted on-chain.

**Use cases:**
- Stealth token launches without exposing allocation strategy
- Private airdrops to communities
- Confidential treasury distributions
- Any scenario where you don't want the world watching your cap table

## Installation

```bash
cargo install stealth-launch
```

Or build from source:

```bash
git clone https://github.com/piccassol/stealthsuo
cd stealthsuo
cargo build --release
```

## Network

Stealthsuo requires the **zk-edge** testnet which has Token2022 confidential transfers enabled with ZK ElGamal support.

```bash
# Default RPC (already configured)
https://zk-edge.surfnet.dev:8899

# Get testnet SOL
solana airdrop 5 --url https://zk-edge.surfnet.dev:8899
```

> **Note:** Mainnet Token2022 does not yet have ZK ElGamal enabled. This tool targets the zk-edge network for the Solana Privacy Hackathon 2026.

## Usage

### Quick Start

```bash
# 1. Create a confidential mint with hidden supply
stealth-launch create \
  --name "Shadow Token" \
  --symbol "SHDW" \
  --supply 1000000 \
  --decimals 6 \
  --keypair authority.json

# 2. Configure recipient accounts for confidential transfers
stealth-launch configure \
  --mint <MINT_ADDRESS> \
  --owner recipient.json

# 3. Distribute tokens privately
stealth-launch distribute \
  --mint <MINT_ADDRESS> \
  --recipients recipients.csv \
  --keypair authority.json

# 4. Check encrypted balances
stealth-launch balance \
  --mint <MINT_ADDRESS> \
  --wallet <WALLET_ADDRESS> \
  --keypair owner.json
```

## Commands

### `create`

Creates a new token mint with confidential transfer extensions enabled. Mints the initial supply and immediately deposits it into the authority's confidential balance.

```bash
stealth-launch create \
  --name "Token Name" \
  --symbol "TKN" \
  --supply 1000000 \
  --decimals 6 \
  --keypair authority.json \
  --rpc https://zk-edge.surfnet.dev:8899
```

**What happens:**
1. Creates mint with `ConfidentialTransferMint` extension
2. Creates authority's token account with `ConfidentialTransferAccount` extension
3. Mints supply to authority's public balance
4. Deposits to confidential pending balance (encrypted)
5. Applies pending balance to available balance

After this, the supply exists but is **encrypted on-chain**. External observers cannot see the amount.

### `configure`

Sets up a recipient's token account for confidential transfers. Must be run before they can receive private transfers.

```bash
stealth-launch configure \
  --mint <MINT_ADDRESS> \
  --owner recipient.json \
  --rpc https://zk-edge.surfnet.dev:8899
```

**What happens:**
1. Creates Associated Token Account if needed
2. Reallocates account for `ConfidentialTransferAccount` extension
3. Generates ElGamal keypair from owner signature
4. Generates AES key for decryption
5. Configures account with pubkey proof

### `distribute`

Transfers tokens privately to multiple recipients using zero-knowledge proofs.

```bash
stealth-launch distribute \
  --mint <MINT_ADDRESS> \
  --recipients recipients.csv \
  --keypair authority.json \
  --rpc https://zk-edge.surfnet.dev:8899
```

**CSV format:**
```csv
wallet,amount
5abc...xyz,100000
7def...uvw,50000
```

**What happens per recipient:**
1. Generates transfer proof data (equality, ciphertext validity, range proofs)
2. Creates 3 proof context state accounts (split mode for large proofs)
3. Executes confidential transfer instruction
4. Closes proof accounts to reclaim rent

Each transfer requires ~7 transactions due to proof size limits.

### `balance`

Decrypts and displays confidential balances for a wallet.

```bash
stealth-launch balance \
  --mint <MINT_ADDRESS> \
  --wallet <WALLET_ADDRESS> \
  --keypair owner.json \
  --rpc https://zk-edge.surfnet.dev:8899
```

**Output:**
```
Balances for 5abc...xyz
  Public:                0
  Pending Confidential:  100000
  Available Confidential: 0
```

> Recipients receive tokens in "pending" state. They must call `apply-pending-balance` to move funds to available before spending.

## How It Works

### Cryptographic Primitives

| Component | Purpose |
|-----------|---------|
| **ElGamal Encryption** | Encrypts balance amounts on-chain |
| **AES-128-GCM** | Derives deterministic keys from owner signatures |
| **Range Proofs** | Proves transfer amount is valid without revealing it |
| **Equality Proofs** | Proves ciphertext consistency |
| **Ciphertext Validity Proofs** | Proves encrypted values are well-formed |

### Transaction Flow

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│   Create    │────▶│   Configure  │────▶│  Distribute │
│    Mint     │     │   Accounts   │     │   Tokens    │
└─────────────┘     └──────────────┘     └─────────────┘
      │                    │                    │
      ▼                    ▼                    ▼
 Encrypted            ElGamal +            ZK Proofs
  Supply              AES Keys            (3 accounts)
```

### Why zk-edge?

Mainnet Token2022 has confidential transfers but **ZK ElGamal proofs are disabled** pending security audits. The zk-edge network runs a modified validator with full ZK support enabled.

```toml
# Cargo.toml dependencies that work with zk-edge
solana-sdk = "3.0.0"
spl-token-2022 = "10.0.0"
spl-token-client = "0.18.0"
```

## Example Session

```bash
$ stealth-launch create --name "Stealth" --symbol "STL" --supply 1000000 --decimals 6 --keypair auth.json

Creating confidential mint...
Mint: Fgv44rXnoavaPvYyf5sXGMopNTZa3QYByCAUvmQ9v8h9
Authority ATA: 7UX2i7SucgLMQcfZ75s3VXmZZY4YRUyJN9X1RgfMoDUi
Minted 1000000 tokens (public)
Deposited to confidential balance
Applied pending balance

Supply is now hidden. External observers see encrypted ciphertext only.

$ stealth-launch configure --mint Fgv44... --owner recipient1.json

Configuring account for confidential transfers...
ATA: 9abc...
ElGamal pubkey generated
Account configured for confidential transfers

$ stealth-launch distribute --mint Fgv44... --recipients dist.csv --keypair auth.json

Distributing to 2 recipients...

[1/2] 5xyz... - 100000 tokens
  Creating equality proof account...
  Creating ciphertext validity proof account...
  Creating range proof account...
  Executing confidential transfer...
  Closing proof accounts...
  Done.

[2/2] 7uvw... - 150000 tokens
  ...
  Done.

Distribution complete. 250000 tokens transferred privately.

$ stealth-launch balance --mint Fgv44... --wallet 5xyz... --keypair recipient1.json

Balances for 5xyz...
  Public:                0
  Pending Confidential:  100000
  Available Confidential: 0
```

## Limitations

- **zk-edge only** - Mainnet doesn't have ZK proofs enabled yet
- **7 txs per transfer** - Proof accounts require separate transactions
- **Pending state** - Recipients must apply pending balance before spending
- **No UI** - CLI only (for now)

## Built For

<p align="center">
  <img src="https://img.shields.io/badge/Solana-Privacy%20Hackathon%202026-14F195?style=for-the-badge&logo=solana" alt="Privacy Hackathon">
</p>

Stealthsuo targets:
- **Track 1: Private Payments** - Confidential token transfers
- **Anoncoin Bounty** - Privacy tooling for memecoin launches

## License

MIT

---

<p align="center">
  Built by <a href="https://github.com/tetsuo-ai">Tetsuo</a> 
</p>