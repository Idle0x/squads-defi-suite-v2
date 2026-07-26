//! Jupiter Quote API client and types.
//!
//! Provides QuoteResponse parsing, JupiterClient with guardrail validation,
//! and transaction building — all pure data processing (no HTTP in WASM).

use serde::{Deserialize, Serialize};
use thiserror::Error;
use crate::types::Pubkey;

// ---------------------------------------------------------------------------
// QuoteResponse — parsed from Jupiter API JSON (host fetches this via HTTP)
// ---------------------------------------------------------------------------

/// Represents a Jupiter swap quote response (parsed from the host's HTTP call).
/// This struct mirrors the JSON response from api.jup.ag.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResponse {
    pub input_mint: String,
    pub output_mint: String,
    /// u64 as string from Jupiter API
    pub in_amount: String,
    /// u64 as string from Jupiter API
    pub out_amount: String,
    pub other_amount_threshold: String,
    pub slippage_bps: u64,
    /// e.g., 0.3 for 0.3%
    pub price_impact_pct: f64,
    /// Jupiter's computed price (input_amount / output_amount in token decimals)
    pub price: f64,
    pub route_plan: Vec<QuoteRouteStep>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRouteStep {
    pub swap_info: QuoteSwapInfo,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteSwapInfo {
    pub amm_key: String,
    pub label: String,
    pub input_mint: String,
    pub output_mint: String,
    pub in_amount: String,
    pub out_amount: String,
}

// ---------------------------------------------------------------------------
// JupiterClient — Quote Validator and Transaction Builder (NO HTTP)
// ---------------------------------------------------------------------------

/// Validates Jupiter quotes against configured guardrails.
/// This is PURE LOGIC — no network calls. The host fetches the quote via HTTP
/// and passes the parsed QuoteResponse to this client for validation.
pub struct JupiterClient {
    pub base_url: String,
    pub api_key: String,
}

/// Error type for JupiterClient operations.
#[derive(Error, Debug)]
pub enum JupiterClientError {
    #[error("Invalid quote data: {0}")]
    InvalidQuote(String),
    #[error("Quote validation failed: {0}")]
    ValidationFailed(String),
    #[error("Insufficient liquidity for route")]
    InsufficientLiquidity,
}

impl JupiterClient {
    /// Create a new JupiterClient.
    /// base_url: normally "https://api.jup.ag"
    /// api_key: the JUPITER_API_KEY environment variable value
    pub fn new(base_url: &str, api_key: &str) -> Self {
        JupiterClient {
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
        }
    }

    /// Validate a Jupiter quote against configured guardrails.
    ///
    /// Returns Ok(()) if the quote passes all guardrails.
    /// Returns Err(String) with a clear denial message if any guardrail fails.
    ///
    /// Guardrails checked (in order):
    /// 1. Slippage — quote slippage_bps must be <= max_slippage_bps (from config)
    /// 2. Price impact — quote price_impact_pct must be <= max_price_impact (from config)
    /// 3. Route complexity — route_plan.len() must be <= max_route_hops (from config)
    ///   4. Mint allowlist — both input_mint and output_mint must be in allowed_mints
    ///   5. Max notional — in_amount * estimated_usd_per_unit must be <= max_notional_usd
    ///   6. Daily cap — in_amount * estimated_usd_per_unit + daily_volume_usd must be <= daily_cap_usd
    pub fn validate_quote(
        &self,
        quote: &QuoteResponse,
        max_slippage_bps: u64,
        max_price_impact_pct: f64,
        max_route_hops: u8,
        max_notional_usd: f64,
        allowed_mints: &[String],
        daily_volume_usd: f64,
        daily_cap_usd: f64,
        estimated_usd_per_unit: f64,
    ) -> Result<(), String> {
        // CHECK 1: Slippage
        if quote.slippage_bps > max_slippage_bps {
            return Err(format!(
                "Denied: slippage {} bps exceeds maximum {} bps",
                quote.slippage_bps, max_slippage_bps
            ));
        }

        // CHECK 2: Price impact
        if quote.price_impact_pct > max_price_impact_pct {
            return Err(format!(
                "Denied: price impact {:.2}% exceeds maximum {:.2}%",
                quote.price_impact_pct, max_price_impact_pct
            ));
        }

        // CHECK 3: Route complexity
        let route_len = quote.route_plan.len() as u8;
        if route_len > max_route_hops {
            return Err(format!(
                "Denied: route has {} hops, maximum is {}",
                route_len, max_route_hops
            ));
        }

        // CHECK 4: Mint allowlist
        let input_mint = quote.input_mint.trim();
        let output_mint = quote.output_mint.trim();
        if !allowed_mints.iter().any(|m| m.trim() == input_mint) {
            return Err(format!(
                "Denied: input mint {} is not in the allowed mint list",
                input_mint
            ));
        }
        if !allowed_mints.iter().any(|m| m.trim() == output_mint) {
            return Err(format!(
                "Denied: output mint {} is not in the allowed mint list",
                output_mint
            ));
        }

        // CHECK 5: Max notional (per-swap cap)
        let in_amount_u64 = quote.in_amount.parse::<u64>().unwrap_or(0);
        let in_value_usd = (in_amount_u64 as f64) * estimated_usd_per_unit;
        if max_notional_usd > 0.0 && in_value_usd > max_notional_usd {
            return Err(format!(
                "Denied: swap notional ${:.2} exceeds max ${:.2}",
                in_value_usd, max_notional_usd
            ));
        }

        // CHECK 6: Daily cap
        if (daily_volume_usd + in_value_usd) > daily_cap_usd {
            return Err(format!(
                "Denied: daily swap cap reached (current: ${:.2} + ${:.2} > cap: ${:.2})",
                daily_volume_usd, in_value_usd, daily_cap_usd
            ));
        }

        Ok(())
    }

    /// Parse a Jupiter quote API response into a QuoteResponse struct.
    pub fn parse_quote_response(json: &str) -> Result<QuoteResponse, String> {
        serde_json::from_str(json).map_err(|e| format!("failed to parse quote: {e}"))
    }
}

/// A Jupiter swap quote with route information (legacy type — kept for backward compatibility).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Quote {
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub in_amount: u64,
    pub out_amount: u64,
    pub other_amount_threshold: u64,
    pub slippage_bps: u64,
    pub notional_usd: f64,
    pub swap_transaction: Option<String>,
    pub route_plan: Vec<RouteStep>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RouteStep {
    pub swap_info: SwapInfo,
    pub percent: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapInfo {
    pub label: Option<String>,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub notional_usd: f64,
    pub fee_mint: Pubkey,
    pub fee_amount: u64,
}

/// Jupiter quote API query parameters.
#[derive(Clone, Debug)]
pub struct QuoteRequest {
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub amount: u64,
    pub slippage_bps: u64,
    pub only_direct_routes: bool,
}

// ===========================================================================
// Jupiter /swap-instructions Response Types (Phase 1)
// ===========================================================================

/// Jupiter /swap-instructions response — the actual executable instructions.
/// The host POSTs the quote response to /swap/v1/swap-instructions and
/// receives this structure. The plugin uses these instructions to build
/// the real unsigned swap transaction.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapInstructionsResponse {
    /// Instructions to run before the swap (e.g., create ATAs, wrap SOL)
    pub setup_instructions: Vec<SwapInstructionData>,
    /// The main swap instruction — THIS is the Jupiter program invocation
    pub swap_instruction: SwapInstructionData,
    /// Instructions to run after the swap (e.g., close wSOL account, unwrap)
    pub cleanup_instruction: Option<SwapInstructionData>,
    /// Address Lookup Table addresses used by these instructions
    pub address_lookup_table_addresses: Vec<String>,
    /// Compute budget instructions (set compute unit limit + price)
    pub compute_budget_instructions: Option<Vec<SwapInstructionData>>,
}

/// A single instruction from Jupiter's swap-instructions response.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapInstructionData {
    /// The program ID that executes this instruction (base58)
    pub program_id: String,
    /// Serialized instruction data (base64-encoded bytes)
    pub data: String,
    /// Accounts for this instruction
    pub accounts: Vec<SwapInstructionAccount>,
}

/// An account reference in a Jupiter swap instruction.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapInstructionAccount {
    /// The account's public key (base58)
    pub pubkey: String,
    /// Whether this account signs the transaction
    pub is_signer: bool,
    /// Whether this account is writable
    pub is_writable: bool,
}

/// Build a Jupiter quote URL from request parameters.
pub fn build_quote_url(base_url: &str, req: &QuoteRequest) -> String {
    format!(
        "{}/quote?inputMint={}&outputMint={}&amount={}&slippageBps={}&onlyDirectRoutes={}",
        base_url.trim_end_matches('/'),
        req.input_mint.to_string(),
        req.output_mint.to_string(),
        req.amount,
        req.slippage_bps,
        req.only_direct_routes
    )
}

/// Parse a Jupiter quote API response into a Quote struct (legacy).
pub fn parse_quote_response(json: &str) -> Result<Quote, String> {
    serde_json::from_str(json).map_err(|e| format!("failed to parse quote: {e}"))
}

/// Build the Jupiter swap transaction URL.
pub fn build_swap_url(base_url: &str, user_pubkey: &Pubkey, quote_response_b64: &str) -> String {
    format!(
        "{}/swap?userPublicKey={}&quoteResponse={}&wrapAndUnwrapSol=true",
        base_url.trim_end_matches('/'),
        user_pubkey.to_string(),
        quote_response_b64
    )
}

/// Calculate price impact from the swap route.
pub fn calculate_price_impact(quote: &Quote) -> f64 {
    if quote.other_amount_threshold == 0 || quote.out_amount == 0 {
        return 0.0;
    }
    let diff = quote.out_amount as f64 - quote.other_amount_threshold as f64;
    (diff / quote.out_amount as f64) * 100.0
}

/// Describe the swap route as a human-readable summary.
pub fn describe_route(quote: &Quote) -> String {
    if quote.route_plan.is_empty() {
        return "direct swap".to_string();
    }
    let labels: Vec<String> = quote
        .route_plan
        .iter()
        .filter_map(|step| step.swap_info.label.clone())
        .collect();
    if labels.is_empty() {
        format!("{}-hop route", quote.route_plan.len())
    } else {
        labels.join(" → ")
    }
}
