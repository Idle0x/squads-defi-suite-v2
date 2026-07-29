//! End-to-end swap proposal builder.
//!
//! Wires together: Jupiter quote → guardrail checks → swap tx → Squads proposal.
//! All pure Rust — no wasm dependencies. Testable with `cargo test` on host.

use squads_defi_core::jupiter::{Quote, QuoteResponse, SwapInstructionsResponse, describe_route};
use squads_defi_core::shape::{self, MAX_OUTPUT_TOKENS};
use squads_defi_core::squads::{
    build_meta_transaction,
};
use squads_defi_core::{Blockhash, Pubkey, SquadsProposal, Transaction};

use crate::config::{PluginConfig, SwapGuardrails};
use crate::error::PluginError;

/// Truncate a base58 pubkey for display (first 8 chars + …).
fn short_mint(s: &str) -> String {
    if s.len() > 8 {
        format!("{}…", &s[..8])
    } else {
        s.to_string()
    }
}

/// The result of building a swap proposal.
pub struct ProposalResult {
    /// The unsigned Squads v4 proposal (ready for review/signing).
    pub proposal: SquadsProposal,
    /// The unsigned swap transaction embedded in the proposal.
    pub swap_tx: Transaction,
    /// Human-readable summary (≤200 tokens for the bounty).
    pub summary: String,
    /// Base64-encoded meta-transaction for Squads app import.
    pub meta_tx_base64: String,
}

/// Build a full swap proposal: quote → guardrails → swap tx → Squads proposal.
///
/// # Arguments
/// * `quote` — Jupiter swap quote from the API (legacy Quote type)
/// * `cfg` — Plugin config (parsed from `__config` HashMap)
/// * `guardrails` — Guardrails struct (derived from config)
/// * `blockhash` — Recent blockhash from the cluster
/// * `daily_spent_usd` — Cumulative USD spent today (for daily cap check)
pub fn build_swap_proposal(
    quote: &Quote,
    cfg: &PluginConfig,
    guardrails: &SwapGuardrails,
    blockhash: Blockhash,
    daily_spent_usd: f64,
) -> Result<ProposalResult, PluginError> {
    // ── 1. Guardrail check ──────────────────────────────────────────
    guardrails
        .check(quote, daily_spent_usd)
        .map_err(|e| PluginError::Guardrail(e.to_string()))?;

    // ── 2. Build swap transaction ───────────────────────────────────
    let swap_tx = crate::swap::build_swap_transaction(quote, &cfg.creator, blockhash)?;

    // ── 3. Wrap in Squads v4 proposal ───────────────────────────────
    let route_desc = describe_route(quote);
    let title = format!(
        "Swap {} → {} ({})",
        quote.input_mint,
        quote.output_mint,
        route_desc
    );
    let description = format!(
        "{} input → ~{} output | Slippage: {} bps | Notional: ${:.2}",
        quote.in_amount, quote.out_amount, quote.slippage_bps, quote.notional_usd
    );

    let proposal = squads_defi_core::build_proposal(
        cfg.squads_vault,
        cfg.creator,
        cfg.squads_vault,
        swap_tx.clone(),
        cfg.proposal_expiry_hours,
        Some(title),
        Some(description),
    );

    let meta_tx_base64 = proposal.to_meta_tx_base64();

    // ── 4. Shape the output summary (≤200 tokens) ───────────────────
    let summary = shape_summary_for_proposal(quote, &proposal, &route_desc);

    Ok(ProposalResult {
        proposal,
        swap_tx,
        summary,
        meta_tx_base64,
    })
}

/// Build a swap proposal from a real QuoteResponse and SwapInstructionsResponse.
///
/// This is the primary proposal builder for the "agent proposes, Squads disposes"
/// workflow. It validates the quote against guardrails, builds the swap transaction
/// from real Jupiter instructions, wraps it in a Squads proposal, and returns
/// the base64 meta-tx + human-readable summary.
pub fn build_real_swap_proposal(
    quote: &QuoteResponse,
    swap_instructions: &SwapInstructionsResponse,
    cfg: &PluginConfig,
    guardrails: &SwapGuardrails,
    daily_spent_usd: f64,
    usd_per_unit: f64,
    blockhash: &Blockhash,
    authority_pubkey: &Pubkey,
    squads_program_id: &Pubkey,
    transaction_index: u64,
) -> Result<(String, String), PluginError> {
    use squads_defi_core::jupiter::JupiterClient;
    use squads_defi_core::Transaction;

    // ── 1. Validate all guardrails ────────────────────────────────
    let client = JupiterClient::new(&cfg.jupiter_url, "");

    // Use the host-provided USD price per unit.
    // The plugin fetches this from Jupiter's price API internally.
    // It is always available — notional and daily cap guardrails
    // can always be enforced.
    let per_unit = usd_per_unit;
    client.validate_quote(
        quote,
        guardrails.max_slippage_bps,
        5.0,  // max_price_impact_pct
        5,    // max_route_hops
        guardrails.max_notional_usd as f64,
        &cfg.mint_allowlist.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
        daily_spent_usd,
        guardrails.per_day_cap_usd as f64,
        per_unit,
    ).map_err(|e| PluginError::Guardrail(e))?;

    // ── 2. Build real swap transaction from Jupiter instructions ──
    let swap_tx_base64 = crate::swap::build_real_swap_tx(
        swap_instructions,
        authority_pubkey,
        blockhash,
    )?;

    // ── 3. Serialize swap tx message for Squads embedding ─────────
    let swap_tx = Transaction::from_base64(&swap_tx_base64)
        .map_err(|e| PluginError::Swap(format!("decode swap tx: {}", e)))?;

    let swap_tx_message_bytes = swap_tx.message.to_wire();

    // ── 4. Wrap in Squads meta-transaction ────────────────────────
    let meta_tx_base64 = build_meta_transaction(
        authority_pubkey,
        squads_program_id,
        swap_tx_message_bytes,
        Some(format!(
            "Jupiter swap: {} {} → {} {}",
            quote.in_amount, quote.input_mint,
            quote.out_amount, quote.output_mint,
        )),
        blockhash,
        &cfg.squads_vault,
        transaction_index,
    ).map_err(|e| PluginError::Swap(format!("meta-tx build: {e}")))?;

    // ── 5. Human-readable summary (≤200 tokens) ───────────────────
    let input_short = short_mint(&quote.input_mint);
    let output_short = short_mint(&quote.output_mint);
    let summary = format!(
        "Swap Proposal Ready\n\
         Input: {} {} → Output: {} {}\n\
         Slippage: {} bps | Price Impact: {:.2}%\n\
         Route: {} hops | Expires: +{}h\n\
         Open Squads app to review and sign.",
        quote.in_amount, input_short,
        quote.out_amount, output_short,
        quote.slippage_bps, quote.price_impact_pct,
        quote.route_plan.len(), cfg.proposal_expiry_hours
    );

    Ok((meta_tx_base64, summary))
}

/// Build a human-readable summary for the proposal output.
/// Must fit within 200 tokens for the bounty requirement.
fn shape_summary_for_proposal(quote: &Quote, proposal: &SquadsProposal, route_desc: &str) -> String {
    let input_label = quote.input_mint.to_string();
    let input_short = if input_label.len() > 8 {
        format!("{}…", &input_label[..8])
    } else {
        input_label
    };

    let output_label = quote.output_mint.to_string();
    let output_short = if output_label.len() > 8 {
        format!("{}…", &output_label[..8])
    } else {
        output_label
    };

    let sections = vec![
        ("Input", format!("{} {} lamports", quote.in_amount, input_short)),
        ("Output", format!("~{} {} lamports", quote.out_amount, output_short)),
        ("Route", route_desc.to_string()),
        (
            "Slippage",
            format!("{} bps ({:.2}%)", quote.slippage_bps, quote.slippage_bps as f64 / 100.0),
        ),
        ("Notional", format!("${:.2}", quote.notional_usd)),
        (
            "Expires",
            format!("{}h from now", proposal.expiry_timestamp),
        ),
    ];

    shape::shape_summary("Swap Proposal Built", sections, MAX_OUTPUT_TOKENS * 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use squads_defi_core::{Blockhash, Pubkey};
    use std::collections::HashMap;

    const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    const VAULT_PK: &str = "GfQkesR7PGJP7etL6scmp8R1SaHLBcryCUaHehgLjmCW";

    fn test_quote() -> Quote {
        use squads_defi_core::tx::CompiledInstruction;
        use squads_defi_core::types::MessageHeader;
        let placeholder_tx = Transaction::new_unsigned(squads_defi_core::tx::Message {
            header: MessageHeader {
                num_required_signatures: 1,
                num_readonly_signed_accounts: 0,
                num_readonly_unsigned_accounts: 0,
            },
            account_keys: vec![Pubkey::new([1u8; 32])],
            instructions: vec![CompiledInstruction {
                program_id_index: 0,
                accounts: vec![0],
                data: vec![1, 2, 3],
            }],
            recent_blockhash: Blockhash::new([0u8; 32]),
            address_table_lookups: vec![],
        });

        Quote {
            input_mint: Pubkey::new([1u8; 32]),
            output_mint: Pubkey::from_str(USDC_MINT).unwrap(),
            in_amount: 1_000_000_000,
            out_amount: 950_000_000,
            other_amount_threshold: 900_000_000,
            slippage_bps: 50,
            notional_usd: 100.0,
            swap_transaction: Some(placeholder_tx.to_base64()),
            route_plan: vec![],
        }
    }

    fn test_config() -> PluginConfig {
        let mut cfg = HashMap::new();
        cfg.insert("rpc_url".to_string(), "https://api.devnet.solana.com".to_string());
        cfg.insert("squads_vault".to_string(), VAULT_PK.to_string());
        cfg.insert("mint_allowlist".to_string(), USDC_MINT.to_string());
        PluginConfig::from_section(&cfg).unwrap()
    }

    #[test]
    fn test_build_swap_proposal_succeeds() {
        let cfg = test_config();
        let guardrails = SwapGuardrails::from(&cfg);
        let quote = test_quote();
        let blockhash = Blockhash::new([7u8; 32]);

        let result = build_swap_proposal(&quote, &cfg, &guardrails, blockhash, 0.0);
        assert!(result.is_ok(), "valid proposal must succeed: {:?}", result.err());
    }

    #[test]
    fn test_proposal_has_nonempty_meta_tx() {
        let cfg = test_config();
        let guardrails = SwapGuardrails::from(&cfg);
        let quote = test_quote();
        let blockhash = Blockhash::new([7u8; 32]);

        let result = build_swap_proposal(&quote, &cfg, &guardrails, blockhash, 0.0).unwrap();
        assert!(!result.meta_tx_base64.is_empty());
    }

    #[test]
    fn test_proposal_summary_under_200_tokens() {
        let cfg = test_config();
        let guardrails = SwapGuardrails::from(&cfg);
        let quote = test_quote();
        let blockhash = Blockhash::new([7u8; 32]);

        let result = build_swap_proposal(&quote, &cfg, &guardrails, blockhash, 0.0).unwrap();
        let tokens = shape::count_tokens(&result.summary);
        assert!(tokens <= 200, "summary must be ≤200 tokens, got {tokens}");
    }

    #[test]
    fn test_guardrail_failure_propagates() {
        let cfg = test_config();
        let guardrails = SwapGuardrails::from(&cfg);
        let bad_quote = Quote {
            output_mint: Pubkey::new([99u8; 32]), // not in allowlist
            ..test_quote()
        };
        let blockhash = Blockhash::new([7u8; 32]);

        let result = build_swap_proposal(&bad_quote, &cfg, &guardrails, blockhash, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_daily_cap_exceeded_in_proposal() {
        let cfg = test_config();
        let guardrails = SwapGuardrails::from(&cfg);
        let quote = test_quote();
        let blockhash = Blockhash::new([7u8; 32]);

        // Already spent 9901 → adding 100 exceeds 10000 cap
        let result = build_swap_proposal(&quote, &cfg, &guardrails, blockhash, 9_901.0);
        assert!(result.is_err());
    }

    /// Integration test: full chain QuoteResponse → validate → build proposal → meta-tx.
    /// Tests the new primary API path (not the legacy Quote path).
    #[test]
    fn test_full_chain_quote_response_to_meta_tx() {
        use squads_defi_core::jupiter::{
            QuoteResponse, QuoteRouteStep, QuoteSwapInfo, SwapInstructionsResponse,
            SwapInstructionData, SwapInstructionAccount,
        };

        let cfg = test_config();
        let guardrails = SwapGuardrails::from(&cfg);
        let authority = cfg.creator;
        let squads_program_id = Pubkey::from_str(
            "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"
        ).unwrap();

        let quote = QuoteResponse {
            input_mint: cfg.mint_allowlist[0].to_string(),
            output_mint: cfg.mint_allowlist[0].to_string(),
            in_amount: "1000000".to_string(),
            out_amount: "990000".to_string(),
            other_amount_threshold: "980000".to_string(),
            slippage_bps: 50,
            price_impact_pct: 0.1,
            price: 1.0,
            route_plan: vec![QuoteRouteStep {
                swap_info: QuoteSwapInfo {
                    amm_key: "JUP6LkbZbjSVPjAzYfPmznVhFRkZMLaGDnfTm15x4Pv".to_string(),
                    label: "Jupiter".to_string(),
                    input_mint: cfg.mint_allowlist[0].to_string(),
                    output_mint: cfg.mint_allowlist[0].to_string(),
                    in_amount: "1000000".to_string(),
                    out_amount: "990000".to_string(),
                },
            }],
        };

        // Minimal SwapInstructionsResponse for the test
        use base64::Engine;
        let swap_instructions = SwapInstructionsResponse {
            setup_instructions: vec![],
            swap_instruction: SwapInstructionData {
                program_id: "JUP6LkbZbjSVPjAzYfPmznVhFRkZMLaGDnfTm15x4Pv".to_string(),
                data: base64::engine::general_purpose::STANDARD.encode(&[1, 2, 3]),
                accounts: vec![SwapInstructionAccount {
                    pubkey: cfg.mint_allowlist[0].to_string(),
                    is_signer: false,
                    is_writable: true,
                }],
            },
            cleanup_instruction: None,
            address_lookup_table_addresses: vec![],
            compute_budget_instructions: None,
        };

        let blockhash = Blockhash::new([0u8; 32]);

        let result = build_real_swap_proposal(
            &quote, &swap_instructions, &cfg, &guardrails,
            0.0, 0.001, &blockhash, &authority, &squads_program_id,
            0,
        );

        assert!(result.is_ok(), "full chain must succeed: {:?}", result.err());
        let (meta_tx_base64, summary) = result.unwrap();
        assert!(!meta_tx_base64.is_empty(), "meta-tx must be non-empty");
        assert!(!summary.is_empty(), "summary must be non-empty");
        assert!(summary.contains("Swap"), "summary must mention swap");
    }

    /// Integration test: denial at guardrail step stops the chain.
    #[test]
    fn test_full_chain_denial_stops_proposal() {
        use squads_defi_core::jupiter::{
            QuoteResponse, QuoteRouteStep, QuoteSwapInfo, SwapInstructionsResponse,
            SwapInstructionData, SwapInstructionAccount,
        };

        let cfg = test_config();
        let guardrails = SwapGuardrails::from(&cfg);
        let authority = cfg.creator;
        let squads_program_id = Pubkey::from_str(
            "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"
        ).unwrap();

        let bad_quote = QuoteResponse {
            input_mint: cfg.mint_allowlist[0].to_string(),
            output_mint: cfg.mint_allowlist[0].to_string(),
            in_amount: "1000000".to_string(),
            out_amount: "990000".to_string(),
            other_amount_threshold: "980000".to_string(),
            slippage_bps: 500,
            price_impact_pct: 0.1,
            price: 1.0,
            route_plan: vec![],
        };

        use base64::Engine;
        let swap_instructions = SwapInstructionsResponse {
            setup_instructions: vec![],
            swap_instruction: SwapInstructionData {
                program_id: "JUP6LkbZbjSVPjAzYfPmznVhFRkZMLaGDnfTm15x4Pv".to_string(),
                data: base64::engine::general_purpose::STANDARD.encode(&[1, 2, 3]),
                accounts: vec![SwapInstructionAccount {
                    pubkey: cfg.mint_allowlist[0].to_string(),
                    is_signer: false, is_writable: true,
                }],
            },
            cleanup_instruction: None,
            address_lookup_table_addresses: vec![],
            compute_budget_instructions: None,
        };

        let blockhash = Blockhash::new([0u8; 32]);

        let result = build_real_swap_proposal(
            &bad_quote, &swap_instructions, &cfg, &guardrails,
            0.0, 0.001, &blockhash, &authority, &squads_program_id,
            0,
        );

        assert!(result.is_err(), "guardrail denial must stop the chain");
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("slippage") || err.contains("Denied"),
            "error must mention the guardrail violation: {}", err);
    }
}
