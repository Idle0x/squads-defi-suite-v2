//! `squads-defi-core` — Shared core for Squads DeFi plugins on ZeroClaw.
//!
//! Provides hand-rolled Solana types, RPC abstraction, transaction construction,
//! Squads v4 proposal encoding, Jupiter quote validation, and response shaping.
//!
//! No `solana-sdk` or `solana-client` dependency — builds for `wasm32-wasip2`.

pub mod types;
pub mod rpc;
pub mod tx;
pub mod squads;
pub mod jupiter;
pub mod shape;
pub mod test_utils;
pub mod ed25519;

// ── Re-exports (flat namespace) ──────────────────────────────────────

// Solana primitives
pub use types::{Blockhash, Pubkey, Signature};
// Wire-format types (Phase 0)
pub use types::{MessageHeader, AccountMeta, Instruction, MessageAddressTableLookup};

// RPC abstraction
pub use rpc::{
    get_account_info, get_latest_blockhash, send_transaction, MockRpcClient, RpcClient, RpcError,
};

// Transaction construction
pub use tx::{
    compile_instructions, build_swap_instruction, build_transfer_instruction, estimate_tokens,
    CompiledInstruction, Message, Transaction,
};

// Squads v4 proposals
pub use squads::{
    build_proposal, SquadsProposal, VaultTransaction,
    // NEW: seed constants
    SEED_PREFIX, SEED_MULTISIG, SEED_TRANSACTION, SEED_PROPOSAL, SEED_EPHEMERAL_SIGNER,
    // NEW: timestamp helpers
    unix_now_seconds, proposal_expiry_timestamp,
    // NEW: anchor discriminator
    anchor_discriminator, vault_transaction_create_discriminator, proposal_create_discriminator,
    // NEW: PDA derivation
    derive_multisig_pda, derive_vault_transaction_pda, derive_proposal_pda,
    derive_ephemeral_signer_pda,
    // NEW: instruction arg types
    VaultTransactionCreateArgs, ProposalCreateArgs,
    // NEW: meta-transaction builder
    build_meta_transaction,
};

// Jupiter Quote API
pub use jupiter::{
    build_quote_url, build_swap_url, calculate_price_impact, describe_route,
    parse_quote_response, Quote, QuoteRequest, RouteStep, SwapInfo,
    // NEW: JupiterClient + QuoteResponse for host-side validation
    JupiterClient, JupiterClientError, QuoteResponse,
    // Phase 1: Swap instructions types
    SwapInstructionsResponse, SwapInstructionData, SwapInstructionAccount,
};

// Response shaping
pub use shape::{
    count_tokens, shape_summary, truncate_to_budget, truncate_to_token_budget, MAX_OUTPUT_TOKENS,
};

// ── Utilities ─────────────────────────────────────────────────────────

/// Decode a base58 string into raw bytes. Convenience for plugin consumers.
pub fn decode_base58(s: &str) -> Result<Vec<u8>, String> {
    bs58::decode(s).into_vec().map_err(|e| format!("base58 decode error: {e}"))
}
