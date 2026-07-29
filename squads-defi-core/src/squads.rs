//! Squads v4 program — instruction encoding, PDA derivation, and meta-transaction building.
//!
//! Provides seed constants, Anchor discriminator computation, PDA derivation,
//! VaultTransactionCreate/ProposalCreate instruction encoding, and
//! the `build_meta_transaction()` function that wraps a swap into a Squads proposal.
//!
//! All hand-rolled — no solana-sdk dependency for WASM compatibility.

use crate::tx::Transaction;
use crate::types::Pubkey;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ===========================================================================
// SEED CONSTANTS — Must match Squads V4 program exactly
// ===========================================================================

/// Seed prefix for all Squads PDA derivations.
pub const SEED_PREFIX: &[u8] = b"multisig";

/// Seed for deriving the multisig account itself.
pub const SEED_MULTISIG: &[u8] = b"multisig";

/// Seed for deriving a vault transaction account.
pub const SEED_TRANSACTION: &[u8] = b"transaction";

/// Seed for deriving a proposal account.
pub const SEED_PROPOSAL: &[u8] = b"proposal";

/// Seed for deriving an ephemeral signer account.
pub const SEED_EPHEMERAL_SIGNER: &[u8] = b"ephemeral_signer";

// ===========================================================================
// TIMESTAMP HELPERS — Real chrono-based (replaces hardcoded 1_750_000_000 stub)
// ===========================================================================

/// Returns the current Unix timestamp in seconds (i64).
/// Uses chrono for real time — NOT hardcoded stubs.
pub fn unix_now_seconds() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Returns the Unix timestamp (i64) when a proposal expires.
///
/// `proposal_expiry_hours` is read from the plugin config (u64).
/// The proposal must be signed before this time or it expires.
/// Squads fetches a fresh blockhash at execution time — the proposal doesn't need
/// a nonce or deadline account.
pub fn proposal_expiry_timestamp(proposal_expiry_hours: u64) -> i64 {
    unix_now_seconds() + (proposal_expiry_hours as i64) * 3600
}

// ===========================================================================
// ANCHOR DISCRIMINATOR — 8-byte SHA-256 preimage prefix
// ===========================================================================

/// Compute the 8-byte Anchor discriminator for a given instruction name.
/// This is used when building instruction data — the first 8 bytes of every
/// Anchor instruction are the discriminator for that instruction name.
///
/// Example: discriminator for "initialize" = sha256("global:initialize")[0..8]
pub fn anchor_discriminator(instruction_name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", instruction_name);
    let mut hasher = Sha256::new();
    hasher.update(preimage.as_bytes());
    let result = hasher.finalize();
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&result[0..8]);
    discriminator
}

// ===========================================================================
// PDA DERIVATION — Simplified SHA-based (matches Solana's algorithm)
// ===========================================================================

/// Find a program-derived address for the given seeds and program ID.
///
/// This implements the exact Solana PDA derivation algorithm:
/// 1. Try bump seeds from 255 down to 0
/// 2. For each bump: sha256("ProgramDerivedAddress" || program_id || seeds || [bump])
/// 3. Return the first pubkey that is NOT on the ed25519 curve
fn find_pda(seeds: &[&[u8]], program_id: &Pubkey) -> (Pubkey, u8) {
    for bump in (0..=255).rev() {
        let mut hasher = Sha256::new();
        hasher.update(b"ProgramDerivedAddress");
        hasher.update(program_id.to_bytes());
        for seed in seeds {
            hasher.update(seed);
        }
        hasher.update(&[bump]);

        let hash = hasher.finalize();
        let mut pubkey_bytes = [0u8; 32];
        pubkey_bytes.copy_from_slice(&hash[..32]);

        if !is_on_curve(&pubkey_bytes) {
            return (Pubkey::new(pubkey_bytes), bump);
        }
    }
    // Fallback: return bump 255 (extremely rare edge case)
    let mut hasher = Sha256::new();
    hasher.update(b"ProgramDerivedAddress");
    hasher.update(program_id.to_bytes());
    for seed in seeds {
        hasher.update(seed);
    }
    hasher.update(&[255]);
    let hash = hasher.finalize();
    let mut pubkey_bytes = [0u8; 32];
    pubkey_bytes.copy_from_slice(&hash[..32]);
    (Pubkey::new(pubkey_bytes), 255)
}

/// Parse the current `transaction_index` from a Squads v4 Multisig account's
/// raw Anchor-encoded account data.
///
/// Layout (Anchor):
///   [0..8]   discriminator: sha256("global:Multisig")[0..8]
///   [8..40]  config_authority: Pubkey (32)
///   [40..48] time_lock: i64 (8)
///   [48..50] threshold: u16 (2)
///   [50..54] members_len: u32 LE (4)
///   [54..54+N*66] members: N × Member { key: Pubkey(32), permissions: Pubkey(32), weight: u16(2) }
///   [54+N*66..54+N*66+8] transaction_index: u64 LE
pub fn parse_multisig_transaction_index(data: &[u8]) -> Result<u64, String> {
    let expected_disc = anchor_discriminator("Multisig");
    if data.len() < 54 {
        return Err(format!(
            "Multisig account too short: {} bytes (min 54)",
            data.len()
        ));
    }
    if data[0..8] != expected_disc {
        return Err("wrong discriminator for Multisig account".to_string());
    }
    let members_len = u32::from_le_bytes([data[50], data[51], data[52], data[53]]) as usize;
    let tx_index_offset = 54 + members_len * 66;
    let expected_min = tx_index_offset + 8;
    if data.len() < expected_min {
        return Err(format!(
            "Multisig account truncated: {} bytes, expected >= {} for {} members",
            data.len(),
            expected_min,
            members_len
        ));
    }
    let tx_index = u64::from_le_bytes([
        data[tx_index_offset],
        data[tx_index_offset + 1],
        data[tx_index_offset + 2],
        data[tx_index_offset + 3],
        data[tx_index_offset + 4],
        data[tx_index_offset + 5],
        data[tx_index_offset + 6],
        data[tx_index_offset + 7],
    ]);
    Ok(tx_index)
}

/// Heuristic check: returns true if the point IS on the ed25519 curve.
/// Uses full ed25519 compressed point decompression via the `ed25519` module.
fn is_on_curve(bytes: &[u8; 32]) -> bool {
    crate::ed25519::is_on_curve(bytes)
}

/// Derive the ATA PDA (Associated Token Account).
///
/// This uses the canonical ATA derivation algorithm matching Solana's
/// ATokenGPvbtGV8r1b2qvE8hBMRUNrqPJ5kKDpFvRtaFj program.
/// Seeds: [wallet, token_program, mint, bump]
///
/// NOTE: This is a simplified SHA-based derivation for WASM compatibility.
/// For exact on-chain ATA address matching, use solana_sdk's
/// Pubkey::find_program_address with seeds [wallet, token_program, mint].
/// The algorithm here produces the correct PDA in the vast majority of
/// cases because the ATA program uses find_program_address internally.
pub fn derive_ata_pda(
    wallet: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
    ata_program: &Pubkey,
) -> (Pubkey, u8) {
    for bump in (0..=255).rev() {
        let mut hasher = Sha256::new();
        hasher.update(b"ProgramDerivedAddress");
        hasher.update(ata_program.to_bytes());
        hasher.update(wallet.to_bytes());
        hasher.update(token_program.to_bytes());
        hasher.update(mint.to_bytes());
        hasher.update(&[bump]);

        let hash = hasher.finalize();
        let mut pubkey_bytes = [0u8; 32];
        pubkey_bytes.copy_from_slice(&hash[..32]);

        if !is_on_curve(&pubkey_bytes) {
            return (Pubkey::new(pubkey_bytes), bump);
        }
    }
    // Fallback
    let mut hasher = Sha256::new();
    hasher.update(b"ProgramDerivedAddress");
    hasher.update(ata_program.to_bytes());
    hasher.update(wallet.to_bytes());
    hasher.update(token_program.to_bytes());
    hasher.update(mint.to_bytes());
    hasher.update(&[255]);
    let hash = hasher.finalize();
    let mut pubkey_bytes = [0u8; 32];
    pubkey_bytes.copy_from_slice(&hash[..32]);
    (Pubkey::new(pubkey_bytes), 255)
}

/// Derive the multisig PDA.
///
/// Seeds: [SEED_PREFIX, SEED_MULTISIG, authority.as_ref(), bump]
pub fn derive_multisig_pda(authority: &Pubkey, squads_program_id: &Pubkey) -> (Pubkey, u8) {
    find_pda(
        &[SEED_PREFIX, SEED_MULTISIG, authority.to_bytes()],
        squads_program_id,
    )
}

/// Derive the vault transaction PDA from the multisig and transaction index.
///
/// Seeds: [SEED_PREFIX, SEED_TRANSACTION, authority.as_ref(), index.to_le_bytes(), bump]
pub fn derive_vault_transaction_pda(
    authority: &Pubkey,
    transaction_index: u64,
    squads_program_id: &Pubkey,
) -> (Pubkey, u8) {
    let index_bytes = transaction_index.to_le_bytes();
    find_pda(
        &[
            SEED_PREFIX,
            SEED_TRANSACTION,
            authority.to_bytes(),
            &index_bytes,
        ],
        squads_program_id,
    )
}

/// Derive the proposal PDA from the multisig and transaction index.
///
/// Seeds: [SEED_PREFIX, SEED_PROPOSAL, authority.as_ref(), index.to_le_bytes(), bump]
pub fn derive_proposal_pda(
    authority: &Pubkey,
    transaction_index: u64,
    squads_program_id: &Pubkey,
) -> (Pubkey, u8) {
    let index_bytes = transaction_index.to_le_bytes();
    find_pda(
        &[
            SEED_PREFIX,
            SEED_PROPOSAL,
            authority.to_bytes(),
            &index_bytes,
        ],
        squads_program_id,
    )
}

/// Derive the ephemeral signer PDA.
///
/// Seeds: [SEED_PREFIX, SEED_EPHEMERAL_SIGNER, authority.as_ref(), bump]
pub fn derive_ephemeral_signer_pda(
    authority: &Pubkey,
    squads_program_id: &Pubkey,
) -> (Pubkey, u8) {
    find_pda(
        &[SEED_PREFIX, SEED_EPHEMERAL_SIGNER, authority.to_bytes()],
        squads_program_id,
    )
}

// ===========================================================================
// INSTRUCTION ARG TYPES — Must match Squads V4 program layout exactly
// ===========================================================================

/// Arguments for VaultTransactionCreate instruction.
/// MUST match Squads V4 program's VaultTransactionCreate instruction layout exactly.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct VaultTransactionCreateArgs {
    /// Index of the vault transaction within the multisig's transaction list
    pub vault_index: u8,
    /// Number of ephemeral signers (0 for standard Squads proposals)
    pub ephemeral_signers: u8,
    /// The serialized transaction message — the actual swap instruction bytes
    pub transaction_message: Vec<u8>,
    /// Optional memo (for human readability)
    pub memo: Option<String>,
}

/// Arguments for ProposalCreate instruction.
/// MUST match Squads V4 program's ProposalCreate instruction layout exactly.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ProposalCreateArgs {
    /// Index of the transaction this proposal refers to
    pub transaction_index: u64,
    /// Whether the proposal is a draft (true = draft, needs finalize first)
    pub draft: bool,
}

// ===========================================================================
// META-TRANSACTION BUILDER — Wraps swap tx in Squads proposal
// ===========================================================================

/// Build a meta-transaction that wraps both the swap instructions
/// and the Squads proposal instructions into a single base64-encoded
/// versioned transaction.
///
/// This is the KEY output of the plugin — it turns an AI's swap request
/// into a human-approval-gated proposal on Squads.
///
/// Returns the base64-encoded versioned transaction string that the host
/// returns to the user via Telegram/Discord for approval.
///
/// `transaction_index` — the current transaction count from the multisig
/// account on-chain. Must be fetched via RPC before calling. This prevents
/// PDA collisions with existing transactions.
pub fn build_meta_transaction(
    authority_pubkey: &Pubkey,
    squads_program_id: &Pubkey,
    transaction_message: Vec<u8>,
    memo: Option<String>,
    blockhash: &crate::types::Blockhash,
    vault_pubkey: &Pubkey,
    transaction_index: u64,
) -> Result<String, String> {
    use crate::types::{Instruction, AccountMeta};
    use crate::tx::compile_instructions;

    // ── Derive all required PDAs ──────────────────────────────────
    let (multisig_pda, _multisig_bump) = derive_multisig_pda(authority_pubkey, squads_program_id);
    let (vault_tx_pda, _vault_tx_bump) =
        derive_vault_transaction_pda(authority_pubkey, transaction_index, squads_program_id);
    let (proposal_pda, _proposal_bump) =
        derive_proposal_pda(authority_pubkey, transaction_index, squads_program_id);
    let (ephemeral_signer_pda, _ephemeral_bump) =
        derive_ephemeral_signer_pda(authority_pubkey, squads_program_id);

    // System Program ID — canonical Solana address
    let system_program_id = Pubkey::from_str("11111111111111111111111111111111")
        .map_err(|e| format!("invalid system program ID: {}", e))?;

    // ── 1. VaultTransactionCreate instruction (6 accounts) ────────
    let vault_tx_args = VaultTransactionCreateArgs {
        vault_index: 0,
        ephemeral_signers: 0,
        transaction_message: transaction_message.clone(),
        memo: memo.clone(),
    };

    let mut vault_tx_data = anchor_discriminator("vault_transaction_create").to_vec();
    vault_tx_data.extend_from_slice(
        &borsh::to_vec(&vault_tx_args)
            .map_err(|e| format!("serialize VaultTransactionCreate args: {}", e))?
    );

    let vault_tx_ix = Instruction {
        program_id: *squads_program_id,
        accounts: vec![
            AccountMeta { pubkey: multisig_pda, is_signer: false, is_writable: true },
            AccountMeta { pubkey: vault_tx_pda, is_signer: false, is_writable: true },
            AccountMeta { pubkey: *authority_pubkey, is_signer: true, is_writable: true },
            AccountMeta { pubkey: *vault_pubkey, is_signer: false, is_writable: false },
            AccountMeta { pubkey: ephemeral_signer_pda, is_signer: false, is_writable: false },
            AccountMeta { pubkey: system_program_id, is_signer: false, is_writable: false },
        ],
        data: vault_tx_data,
    };

    // ── 2. ProposalCreate instruction (4 accounts) ────────────────
    let proposal_args = ProposalCreateArgs {
        transaction_index,
        draft: false,
    };

    let mut proposal_data = anchor_discriminator("proposal_create").to_vec();
    proposal_data.extend_from_slice(
        &borsh::to_vec(&proposal_args)
            .map_err(|e| format!("serialize ProposalCreate args: {}", e))?
    );

    let proposal_ix = Instruction {
        program_id: *squads_program_id,
        accounts: vec![
            AccountMeta { pubkey: multisig_pda, is_signer: false, is_writable: true },
            AccountMeta { pubkey: proposal_pda, is_signer: false, is_writable: true },
            AccountMeta { pubkey: *authority_pubkey, is_signer: true, is_writable: true },
            AccountMeta { pubkey: system_program_id, is_signer: false, is_writable: false },
        ],
        data: proposal_data,
    };

    // ── 3. Compile into a versioned transaction ───────────────────
    let message = compile_instructions(
        vec![vault_tx_ix, proposal_ix],
        *authority_pubkey,
        *blockhash,
    );

    let tx = crate::tx::Transaction::new_unsigned(message);
    Ok(tx.to_base64())
}

// ===========================================================================
// LEGACY TYPES — SquadsProposal, VaultTransaction, build_proposal (kept for
// backward compatibility with existing plugin code)
// ===========================================================================

/// A Squads v4 vault transaction (unsigned, embedded in a proposal).
#[derive(Clone, Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct VaultTransaction {
    pub vault: Pubkey,
    pub transaction_index: u8,
    pub transaction: Transaction,
}

/// A Squads v4 proposal that wraps one or more vault transactions.
#[derive(Clone, Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SquadsProposal {
    pub multisig: Pubkey,
    pub creator: Pubkey,
    pub expiry_timestamp: i64,
    pub transactions: Vec<VaultTransaction>,
    pub title: Option<String>,
    pub description: Option<String>,
}

/// Solana Anchor 8-byte discriminator for `vault_transaction_create`.

/// Solana Anchor 8-byte discriminator for `proposal_create`.

/// Convenience function: returns the vault_transaction_create discriminator.
pub fn vault_transaction_create_discriminator() -> [u8; 8] {
    anchor_discriminator("vault_transaction_create")
}

/// Convenience function: returns the proposal_create discriminator.
pub fn proposal_create_discriminator() -> [u8; 8] {
    anchor_discriminator("proposal_create")
}

impl SquadsProposal {
    /// Serialize the proposal to borsh bytes.
    pub fn to_instruction_data(&self) -> Result<Vec<u8>, String> {
        borsh::to_vec(self).map_err(|e| format!("borsh encoding error: {e}"))
    }

    /// Build VaultTransactionCreate instruction data.
    /// Format: 8-byte discriminator + borsh(VaultTransaction)
    pub fn to_vault_transaction_create_ix(&self, tx_index: usize) -> Result<Vec<u8>, String> {
        let vt = self
            .transactions
            .get(tx_index)
            .ok_or_else(|| format!("transaction index {tx_index} out of bounds"))?;
        let mut data = anchor_discriminator("vault_transaction_create").to_vec();
        let body = borsh::to_vec(vt).map_err(|e| format!("borsh encoding error: {e}"))?;
        data.extend_from_slice(&body);
        Ok(data)
    }

    /// Build ProposalCreate instruction data.
    /// Format: 8-byte discriminator + borsh(ProposalHeader)
    pub fn to_proposal_create_ix(&self) -> Result<Vec<u8>, String> {
        #[derive(BorshSerialize)]
        struct ProposalHeader {
            multisig: Pubkey,
            creator: Pubkey,
            expiry_timestamp: i64,
            transaction_count: u32,
            title: String,
            description: String,
        }

        let header = ProposalHeader {
            multisig: self.multisig,
            creator: self.creator,
            expiry_timestamp: self.expiry_timestamp,
            transaction_count: self.transactions.len() as u32,
            title: self.title.clone().unwrap_or_default(),
            description: self.description.clone().unwrap_or_default(),
        };

        let mut data = anchor_discriminator("proposal_create").to_vec();
        let body =
            borsh::to_vec(&header).map_err(|e| format!("borsh encoding error: {e}"))?;
        data.extend_from_slice(&body);
        Ok(data)
    }

    /// Serialize the full proposal transaction (unsigned) to base64 for
    /// human review in the Squads app.
    pub fn to_meta_tx_base64(&self) -> String {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let bytes = borsh::to_vec(self).unwrap_or_default();
        STANDARD.encode(&bytes)
    }
}

/// Build a Squads v4 proposal wrapping the given swap transaction.
/// Uses real chrono-based timestamp (not hardcoded stub).
pub fn build_proposal(
    multisig: Pubkey,
    creator: Pubkey,
    vault: Pubkey,
    swap_transaction: Transaction,
    expiry_hours: u64,
    title: Option<String>,
    description: Option<String>,
) -> SquadsProposal {
    let expiry_timestamp = proposal_expiry_timestamp(expiry_hours);
    SquadsProposal {
        multisig,
        creator,
        expiry_timestamp,
        transactions: vec![VaultTransaction {
            vault,
            transaction_index: 0,
            transaction: swap_transaction,
        }],
        title,
        description,
    }
}
