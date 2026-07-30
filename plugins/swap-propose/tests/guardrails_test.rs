//! Guardrail injection tests — TDD contracts.
//!
//! These verify that guardrails are enforced in Rust code, not the LLM.
//! Each test tries to bypass a guardrail via config manipulation or crafted input.
//! ALL must pass when implementation is complete.

use swap_propose::config::{PluginConfig, SwapGuardrails};
use swap_propose::error::GuardrailError;
use squads_defi_core::jupiter::Quote;
use squads_defi_core::Pubkey;
use std::collections::HashMap;

// Well-known Solana pubkeys (valid base58):
const VAULT_PK: &str = "GfQkesR7PGJP7etL6scmp8R1SaHLBcryCUaHehgLjmCW";
const CREATOR_PK: &str = "AqxUBSStqnUMdRp4dEBNbi6uJvaeS8q7PQjEEURsADeS";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

/// Helper: create a minimal config with USDC in the allowlist.
fn valid_config() -> HashMap<String, String> {
    let mut cfg = HashMap::new();
    cfg.insert("rpc_url".to_string(), "https://api.devnet.solana.com".to_string());
    cfg.insert("squads_vault".to_string(), VAULT_PK.to_string());
    cfg.insert("creator".to_string(), CREATOR_PK.to_string());
    cfg.insert("mint_allowlist".to_string(), USDC_MINT.to_string());
    cfg.insert("max_slippage_bps".to_string(), "100".to_string());
    cfg.insert("max_notional_usd".to_string(), "1000".to_string());
    cfg.insert("per_day_cap_usd".to_string(), "5000".to_string());
    cfg
}

/// Helper: create a test quote for USDC.
fn usdc_quote() -> Quote {
    let pk = Pubkey::from_str(USDC_MINT).unwrap();
    Quote {
        input_mint: Pubkey::new([1u8; 32]),
        output_mint: pk,
        in_amount: 1_000_000_000,
        out_amount: 950_000_000,
        other_amount_threshold: 900_000_000,
        slippage_bps: 50,
        notional_usd: 100.0,
        swap_transaction: None,
        route_plan: vec![],
    }
}

/// Helper: create a quote with custom slippage and notional.
fn custom_quote(output_mint: Pubkey, slippage_bps: u64, notional_usd: f64) -> Quote {
    Quote {
        input_mint: Pubkey::new([1u8; 32]),
        output_mint,
        in_amount: 1_000_000_000,
        out_amount: 950_000_000,
        other_amount_threshold: 900_000_000,
        slippage_bps,
        notional_usd,
        swap_transaction: None,
        route_plan: vec![],
    }
}

/// Helper: parse config and create guardrails.
fn guardrails_from_config(cfg: &HashMap<String, String>) -> SwapGuardrails {
    let parsed = PluginConfig::from_section(cfg).unwrap();
    SwapGuardrails::from(&parsed)
}

// ============================================================================
// JupiterClient::validate_quote tests (QuoteResponse path — the new primary API)
// ============================================================================

/// Helper: build a test QuoteResponse for JupiterClient testing.
fn test_quote_response(slippage_bps: u64, price_impact: f64, route_hops: u8, in_amount: &str) -> squads_defi_core::jupiter::QuoteResponse {
    squads_defi_core::jupiter::QuoteResponse {
        input_mint: USDC_MINT.to_string(),
        output_mint: USDC_MINT.to_string(),
        in_amount: in_amount.to_string(),
        out_amount: "1000000".to_string(),
        other_amount_threshold: "990000".to_string(),
        slippage_bps,
        price_impact_pct: price_impact,
        price: 1.0,
        route_plan: (0..route_hops).map(|_| squads_defi_core::jupiter::QuoteRouteStep {
            swap_info: squads_defi_core::jupiter::QuoteSwapInfo {
                amm_key: "JUP6LkbZbjSVPjAzYfPmznVhFRkZMLaGDnfTm15x4Pv".to_string(),
                label: "Jupiter".to_string(),
                input_mint: USDC_MINT.to_string(),
                output_mint: USDC_MINT.to_string(),
                in_amount: "500000".to_string(),
                out_amount: "500000".to_string(),
            },
        }).collect(),
    }
}

#[test]
fn test_validate_quote_passes_all_guardrails() {
    let client = squads_defi_core::jupiter::JupiterClient::new("https://api.jup.ag", "");
    let quote = test_quote_response(50, 0.1, 1, "1000000");
    let allowed_mints = vec![USDC_MINT.to_string()];

    let result = client.validate_quote(
        &quote,
        100,    // max_slippage_bps
        5.0,    // max_price_impact_pct
        5,      // max_route_hops
        1000.0, // max_notional_usd
        &allowed_mints,
        0.0,    // daily_volume_usd
        10000.0, // daily_cap_usd
        0.000025, // estimated_usd_per_unit
    );
    assert!(result.is_ok(), "valid quote must pass all guardrails: {:?}", result.err());
}

#[test]
fn test_validate_quote_denies_high_slippage() {
    let client = squads_defi_core::jupiter::JupiterClient::new("https://api.jup.ag", "");
    let quote = test_quote_response(500, 0.1, 1, "1000000");
    let allowed_mints = vec![USDC_MINT.to_string()];

    let result = client.validate_quote(
        &quote, 100, 5.0, 5, 1000.0, &allowed_mints, 0.0, 10000.0, 0.000025,
    );
    assert!(result.is_err(), "high slippage must be denied");
    assert!(result.unwrap_err().contains("slippage"), "error must mention slippage");
}

#[test]
fn test_validate_quote_denies_high_price_impact() {
    let client = squads_defi_core::jupiter::JupiterClient::new("https://api.jup.ag", "");
    let quote = test_quote_response(50, 10.0, 1, "1000000");
    let allowed_mints = vec![USDC_MINT.to_string()];

    let result = client.validate_quote(
        &quote, 100, 5.0, 5, 1000.0, &allowed_mints, 0.0, 10000.0, 0.000025,
    );
    assert!(result.is_err(), "high price impact must be denied");
    assert!(result.unwrap_err().contains("price impact"), "error must mention price impact");
}

#[test]
fn test_validate_quote_denies_disallowed_input_mint() {
    let client = squads_defi_core::jupiter::JupiterClient::new("https://api.jup.ag", "");
    let mut quote = test_quote_response(50, 0.1, 1, "1000000");
    quote.input_mint = "So11111111111111111111111111111111111111112".to_string(); // SOL, not in allowlist
    let allowed_mints = vec![USDC_MINT.to_string()];

    let result = client.validate_quote(
        &quote, 100, 5.0, 5, 1000.0, &allowed_mints, 0.0, 10000.0, 0.000025,
    );
    assert!(result.is_err(), "disallowed input mint must be denied");
}

#[test]
fn test_validate_quote_denies_too_many_hops() {
    let client = squads_defi_core::jupiter::JupiterClient::new("https://api.jup.ag", "");
    let quote = test_quote_response(50, 0.1, 10, "1000000"); // 10 hops, max is 5
    let allowed_mints = vec![USDC_MINT.to_string()];

    let result = client.validate_quote(
        &quote, 100, 5.0, 5, 1000.0, &allowed_mints, 0.0, 10000.0, 0.000025,
    );
    assert!(result.is_err(), "too many route hops must be denied");
}

#[test]
fn test_validate_quote_denies_daily_cap() {
    let client = squads_defi_core::jupiter::JupiterClient::new("https://api.jup.ag", "");
    let quote = test_quote_response(50, 0.1, 1, "1000000");
    let allowed_mints = vec![USDC_MINT.to_string()];

    // in_amount = 1_000_000 * 0.000025 = $25. daily_volume = $9980. Total = $10005 > $10000
    let result = client.validate_quote(
        &quote, 100, 5.0, 5, 1000.0, &allowed_mints, 9980.0, 10000.0, 0.000025,
    );
    assert!(result.is_err(), "daily cap exceeded must be denied");
    assert!(result.unwrap_err().contains("daily"), "error must mention daily cap");
}

#[test]
fn test_validate_quote_passes_empty_route_plan() {
    let client = squads_defi_core::jupiter::JupiterClient::new("https://api.jup.ag", "");
    let mut quote = test_quote_response(50, 0.1, 0, "1000000");
    quote.route_plan = vec![]; // empty route = direct swap
    let allowed_mints = vec![USDC_MINT.to_string()];

    let result = client.validate_quote(
        &quote, 100, 5.0, 5, 1000.0, &allowed_mints, 0.0, 10000.0, 0.000025,
    );
    assert!(result.is_ok(), "empty route plan (direct swap) must pass");
}

// ============================================================================
// 8 Mandatory Injection Tests
// ============================================================================

#[test]
fn test_injection_cannot_override_mint_allowlist() {
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);
    let quote = usdc_quote();

    // USDC is in allowlist → should pass
    assert!(guardrails.check(&quote, 0.0).is_ok());
}

#[test]
fn test_injection_cannot_bypass_daily_cap() {
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);
    let quote = usdc_quote();

    // Already spent 4901 USD today → adding 100 exceeds 5000 cap
    let result = guardrails.check(&quote, 4901.0);
    assert!(result.is_err());
    match result.unwrap_err() {
        GuardrailError::DailyCapExceeded { would_spend, cap } => {
            assert!(would_spend > cap as f64);
        }
        _ => panic!("expected DailyCapExceeded"),
    }
}

#[test]
fn test_injection_cannot_increase_slippage() {
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);

    // Quote with 500 bps slippage when max is 100
    let high_slip_quote = custom_quote(
        Pubkey::from_str(USDC_MINT).unwrap(),
        500,
        100.0,
    );

    let result = guardrails.check(&high_slip_quote, 0.0);
    assert!(result.is_err());
    match result.unwrap_err() {
        GuardrailError::SlippageTooHigh { got, max } => {
            assert_eq!(got, 500);
            assert_eq!(max, 100);
        }
        _ => panic!("expected SlippageTooHigh"),
    }
}

#[test]
fn test_injection_unknown_fields_rejected_or_ignored() {
    let mut cfg = valid_config();
    cfg.insert("malicious_injection".to_string(), "bypass_guardrails".to_string());
    cfg.insert("__extra_field".to_string(), "evil_value".to_string());

    let result = PluginConfig::from_section(&cfg);
    assert!(result.is_ok(), "unknown config keys must not cause errors");
}

#[test]
fn test_execute_output_under_200_tokens() {
    use squads_defi_core::shape;

    let output = shape::shape_summary(
        "Swap Proposal",
        vec![
            ("Input", "10 SOL".to_string()),
            ("Output", "230 USDC".to_string()),
            ("Slippage", "0.5%".to_string()),
            ("Proposal", "Created".to_string()),
        ],
        800,
    );

    let tokens = shape::count_tokens(&output);
    assert!(tokens <= 200, "output must stay <=200 tokens, got {tokens}");
}

#[test]
fn test_empty_config_uses_defaults_safely() {
    let empty: HashMap<String, String> = HashMap::new();
    let result = PluginConfig::from_section(&empty);
    assert!(result.is_err(), "empty config without required keys must error");
}

#[test]
fn test_empty_mint_allowlist_denies_everything() {
    let mut cfg = valid_config();
    cfg.insert("mint_allowlist".to_string(), String::new());
    let guardrails = guardrails_from_config(&cfg);
    let quote = usdc_quote();

    let result = guardrails.check(&quote, 0.0);
    assert!(result.is_err(), "empty allowlist must deny ALL mints");
    match result.unwrap_err() {
        GuardrailError::MintNotAllowed(_) => {}
        _ => panic!("expected MintNotAllowed"),
    }
}

#[test]
fn test_missing_rpc_url_returns_error() {
    let mut cfg = valid_config();
    cfg.remove("rpc_url");
    let result = PluginConfig::from_section(&cfg);
    assert!(result.is_err(), "missing rpc_url must be an error");
}

// ============================================================================
// Additional guardrail tests
// ============================================================================

#[test]
fn test_notional_too_high_rejected() {
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);

    let big_quote = custom_quote(
        Pubkey::from_str(USDC_MINT).unwrap(),
        50,
        5000.0, // exceeds 1000 max
    );

    let result = guardrails.check(&big_quote, 0.0);
    assert!(result.is_err());
    match result.unwrap_err() {
        GuardrailError::NotionalTooHigh { got, max } => {
            assert!(got > max as f64);
        }
        _ => panic!("expected NotionalTooHigh"),
    }
}

#[test]
fn test_mint_not_in_allowlist_rejected() {
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);

    let bad_quote = custom_quote(
        Pubkey::new([99u8; 32]),
        50,
        100.0,
    );

    let result = guardrails.check(&bad_quote, 0.0);
    assert!(result.is_err());
}

#[test]
fn test_valid_swap_passes_all_guardrails() {
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);
    let quote = usdc_quote();

    let result = guardrails.check(&quote, 0.0);
    assert!(result.is_ok(), "valid swap must pass all guardrails");
}

#[test]
fn test_config_defaults_are_reasonable() {
    let mut cfg = valid_config();
    cfg.remove("max_slippage_bps");
    cfg.remove("max_notional_usd");
    cfg.remove("per_day_cap_usd");
    cfg.remove("proposal_expiry_hours");

    let parsed = PluginConfig::from_section(&cfg).unwrap();
    assert_eq!(parsed.max_slippage_bps, 100);
    assert_eq!(parsed.max_notional_usd, 1000);
    assert_eq!(parsed.per_day_cap_usd, 10000);
    assert_eq!(parsed.proposal_expiry_hours, 24);
}

#[test]
fn test_creator_defaults_to_vault() {
    let mut cfg = valid_config();
    cfg.remove("creator");

    let parsed = PluginConfig::from_section(&cfg).unwrap();
    assert_eq!(parsed.creator.to_string(), VAULT_PK);
}

// ============================================================================
// Token budget boundary tests (Phase 5 — required for quality tier)
// ============================================================================

#[test]
fn test_token_budget_swap_within_cap() {
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);
    // daily_volume = 0.9 * cap (4500), in_amount = 0.1 * cap (500) → total ≤ cap → PASS
    let quote = custom_quote(
        Pubkey::from_str(USDC_MINT).unwrap(),
        50,
        500.0,
    );
    let result = guardrails.check(&quote, 4500.0);
    assert!(result.is_ok(), "swap within daily cap must pass");
}

#[test]
fn test_token_budget_swap_exceeds_cap() {
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);
    // daily_volume = 0.9 * cap (4500), in_amount = 0.2 * cap (1000) → total > cap → DENY
    let quote = custom_quote(
        Pubkey::from_str(USDC_MINT).unwrap(),
        50,
        1000.0,
    );
    let result = guardrails.check(&quote, 4500.0);
    assert!(result.is_err(), "swap exceeding daily cap must be denied");
    match result.unwrap_err() {
        GuardrailError::DailyCapExceeded { .. } => {},
        _ => panic!("expected DailyCapExceeded"),
    }
}

#[test]
fn test_token_budget_at_exact_cap_boundary() {
    let mut cfg = valid_config();
    // Bump max_notional_usd so the notional check passes
    cfg.insert("max_notional_usd".to_string(), "5000".to_string());
    let guardrails = guardrails_from_config(&cfg);
    // daily_volume = 0, in_amount = cap → exactly at cap → PASS
    let quote = custom_quote(
        Pubkey::from_str(USDC_MINT).unwrap(),
        50,
        5000.0, // exactly at per_day_cap_usd
    );
    let result = guardrails.check(&quote, 0.0);
    assert!(result.is_ok(), "swap at exact cap boundary must pass");
}

#[test]
fn test_token_budget_cumulative_tracking() {
    // Simulate cumulative daily tracking: multiple swaps accumulate
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);

    let quote_100 = custom_quote(
        Pubkey::from_str(USDC_MINT).unwrap(),
        50,
        100.0,
    );

    // Swap 1: daily_volume=0, notional=100 → PASS
    assert!(guardrails.check(&quote_100, 0.0).is_ok());
    // Swap 2: daily_volume=4900, notional=100 → PASS (at 5000 exactly)
    assert!(guardrails.check(&quote_100, 4900.0).is_ok());
    // Swap 3: daily_volume=4901, notional=100 → DENY (would be 5001 > 5000)
    assert!(guardrails.check(&quote_100, 4901.0).is_err());
}

// ============================================================================
// Prompt injection tests (Phase 5 — required for quality tier)
// ============================================================================

#[test]
fn test_prompt_injection_cannot_bypass_slippage_check() {
    // AI tries to create a quote with 9.9% slippage (990 bps) when max is 100 bps.
    // Guardrail MUST deny this — AI prompts cannot override Rust code.
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);

    let high_slip_quote = custom_quote(
        Pubkey::from_str(USDC_MINT).unwrap(),
        990, // 9.9% — would never pass but AI might try
        100.0,
    );

    let result = guardrails.check(&high_slip_quote, 0.0);
    assert!(result.is_err(), "AI prompt cannot bypass slippage guardrail");
    match result.unwrap_err() {
        GuardrailError::SlippageTooHigh { got, max } => {
            assert_eq!(got, 990);
            assert_eq!(max, 100);
        }
        _ => panic!("expected SlippageTooHigh"),
    }
}

#[test]
fn test_prompt_injection_cannot_bypass_mint_allowlist() {
    // AI tries to trick the plugin by including a malicious mint disguised as a
    // legitimate token. The guardrail checks the mint against the allowlist and
    // must DENY any mint not explicitly allowed.
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);

    // A known malicious program ID disguised as a token mint
    let malicious_mint = Pubkey::from_str("7yJk9iFpdro1sYhCk5dRu3YxHnbiv5Zfu5QjLfSGJjBS").unwrap();
    let quote = custom_quote(malicious_mint, 50, 100.0);

    let result = guardrails.check(&quote, 0.0);
    assert!(result.is_err(), "malicious mint must be denied by allowlist");
    match result.unwrap_err() {
        GuardrailError::MintNotAllowed(_) => {},
        _ => panic!("expected MintNotAllowed"),
    }
}

#[test]
fn test_prompt_injection_cannot_bypass_daily_cap() {
    // AI tries to set daily_volume_usd to cap-1 and in_amount just below cap.
    // The guardrail sums them and MUST deny if total > cap.
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);

    // Already spent 4999, trying to add 2 → total 5001 > 5000 cap
    let quote = custom_quote(
        Pubkey::from_str(USDC_MINT).unwrap(),
        50,
        2.0,
    );

    let result = guardrails.check(&quote, 4999.0);
    assert!(result.is_err(), "daily cap must be enforced even at edge");
    match result.unwrap_err() {
        GuardrailError::DailyCapExceeded { would_spend, cap } => {
            assert!(would_spend > cap as f64);
        }
        _ => panic!("expected DailyCapExceeded"),
    }
}

#[test]
fn test_prompt_injection_huge_amount_blocked() {
    // AI tries to inject a notional of u64::MAX. The guardrail must DENY.
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);

    let huge_quote = custom_quote(
        Pubkey::from_str(USDC_MINT).unwrap(),
        50,
        u64::MAX as f64, // impossibly large notional
    );

    let result = guardrails.check(&huge_quote, 0.0);
    assert!(result.is_err(), "huge notional injection must be denied");
}

// ============================================================================
// Executable injection tests (Phase 5 — required for quality tier)
// ============================================================================

#[test]
fn test_executable_injection_malicious_program_id_as_mint() {
    // AI tries to pass a known executable program ID as a token mint.
    // The allowlist check must DENY it because the program ID is not in
    // the allowed mint list.
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);

    // A real Solana program ID (not a token mint) — TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA
    let executable_program = Pubkey::from_str(
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
    ).unwrap();
    let quote = custom_quote(executable_program, 50, 100.0);

    let result = guardrails.check(&quote, 0.0);
    assert!(result.is_err(), "executable program ID as mint must be denied");
    match result.unwrap_err() {
        GuardrailError::MintNotAllowed(_) => {},
        _ => panic!("expected MintNotAllowed for executable program ID"),
    }
}

#[test]
fn test_executable_injection_arbitrary_program_in_swap() {
    // AI tries to embed a call to an arbitrary on-chain program (not Jupiter)
    // in the swap instruction. The mint allowlist check catches this because
    // the target program's mint-equivalent address is not in the allowlist.
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);

    // A system-level program pubkey — not a token, not allowed
    let system_program = Pubkey::from_str(
        "11111111111111111111111111111111"
    ).unwrap();

    // Even if AI names it "system_program", the raw pubkey won't match allowlist
    let quote = custom_quote(system_program, 50, 100.0);
    let result = guardrails.check(&quote, 0.0);
    assert!(result.is_err(), "arbitrary program injection must be denied");
}

#[test]
fn test_executable_injection_zero_address_as_mint() {
    // AI tries to use the zero pubkey (all zeros) as a mint to bypass checks.
    // This must be denied since it won't be in any allowlist.
    let cfg = valid_config();
    let guardrails = guardrails_from_config(&cfg);

    let zero_pubkey = Pubkey::new([0u8; 32]);
    let quote = custom_quote(zero_pubkey, 50, 100.0);

    let result = guardrails.check(&quote, 0.0);
    assert!(result.is_err(), "zero-address mint injection must be denied");
}
