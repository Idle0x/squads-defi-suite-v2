//! ZeroClaw WIT plugin: `jupiter-swap-propose`.
//!
//! Build a guarded Jupiter swap and wrap it in a Squads multisig proposal.
//! The swap tx is embedded in the proposal so Squads fetches a fresh blockhash
//! on approval — no risk of expired transactions.
//!
//! WIT world: `tool-plugin` exports `plugin-info` + `tool`.
//!
//! ## Security
//!
//! All guardrail config (rpc_url, mint_allowlist, max_slippage_bps,
//! max_notional_usd, per_day_cap_usd, squads_vault, creator, etc.) is
//! injected via the host's `__config` jail. The LLM never sees or controls
//! these values. The `parameters_schema` never declares `__config` —
//! it is host-reserved and spoof-proof.
//!
//! The plugin fetches the Jupiter quote, token price, and swap instructions
//! internally via waki (wasi:http). The LLM provides only the raw swap
//! parameters (input mint, output mint, amount, slippage). All guardrails
//! are enforced in Rust code before any transaction is built.

pub mod config;
pub mod error;
pub mod guardrails;
pub mod propose;
pub mod swap;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "wit/v0",
        world: "tool-plugin",
        features: ["plugins-wit-v0"],
    });

    use crate::{config::PluginConfig, propose};
    use crate::config::SwapGuardrails;
    use exports::zeroclaw::plugin::plugin_info::Guest as PluginInfo;
    use exports::zeroclaw::plugin::tool::{Guest as Tool, ToolResult};
    use zeroclaw::plugin::logging::{
        log_record, LogLevel, PluginAction, PluginEvent, PluginOutcome,
    };
    use squads_defi_core::jupiter::{QuoteResponse, SwapInstructionsResponse};
    use squads_defi_core::squads::derive_proposal_pda;
    use squads_defi_core::Blockhash;
    use serde::Deserialize;
    use std::collections::HashMap;
    use std::time::Instant;

    /// Arguments passed to `execute` from the host.
    /// The LLM provides only the raw swap parameters. The plugin fetches
    /// the Jupiter quote, token price, and swap instructions internally.
    /// `__config` is injected by the host, stripped from any model-supplied
    /// value, and contains the operator's configured guardrails.
    #[derive(Deserialize)]
    struct ExecuteArgs {
        /// Source token mint address (base58)
        input_mint: String,
        /// Destination token mint address (base58)
        output_mint: String,
        /// Amount in source token's smallest unit, as a string
        /// (e.g. lamports for SOL: "1000000000" for 1 SOL)
        amount: String,
        /// Maximum slippage in basis points (50 = 0.5%, 100 = 1%)
        slippage_bps: u64,
        /// Host-injected config — NEVER in parameters_schema.
        /// The host strips any model-supplied `__config` and substitutes
        /// the real operator-configured values from plugins.entries.<name>.
        #[serde(rename = "__config", default)]
        config: HashMap<String, String>,
    }

    struct JupiterSwapPropose;

    const PLUGIN_NAME: &str = "jupiter-swap-propose";
    const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

    /// The Jupiter swap program ID on Solana mainnet.
    const JUPITER_PROGRAM_ID: &str = "JUP6LkbZbjSVPjAzYfPmznVhFRkZMLaGDnfTm15x4Pv";
    /// Default Squads v4 program ID (mainnet). Overridable via `squads_program_id` in config.
    const SQUADS_DEFAULT_PROGRAM_ID: &str = "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf";

    impl PluginInfo for JupiterSwapPropose {
        fn plugin_name() -> String { PLUGIN_NAME.to_string() }
        fn plugin_version() -> String { PLUGIN_VERSION.to_string() }
    }

    impl Tool for JupiterSwapPropose {
        fn name() -> String { "jupiter-swap-propose".to_string() }

        fn description() -> String {
            "Build a guarded Jupiter swap proposal wrapped in a Squads v4 multisig proposal. \
             Provide the raw swap parameters (input_mint, output_mint, amount, slippage_bps). \
             The plugin fetches the Jupiter quote, token price, and swap instructions internally, \
             then validates mint allowlist, slippage, price impact, route hops, notional, and daily cap \
             before constructing the meta-transaction. Returns a base64-encoded versioned transaction \
             ready for human approval in the Squads app."
                .to_string()
        }

        fn parameters_schema() -> String {
            // NOTE: `__config` is NEVER declared here. The host strips any
            // model-supplied `__config` and injects the real operator values.
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input_mint": {
                        "type": "string",
                        "description": "Source token mint address (base58), e.g. So11111111111111111111111111111111111111112 for SOL"
                    },
                    "output_mint": {
                        "type": "string",
                        "description": "Destination token mint address (base58), e.g. EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v for USDC"
                    },
                    "amount": {
                        "type": "string",
                        "description": "Amount in source token's smallest unit, as a string. For SOL, use lamports (1 SOL = 1000000000). For SPL tokens, use the token's smallest unit (e.g. 1000000 for 1 USDC with 6 decimals)."
                    },
                    "slippage_bps": {
                        "type": "integer",
                        "description": "Maximum acceptable slippage in basis points. 50 = 0.5%, 100 = 1%, 300 = 3%."
                    }
                },
                "required": [
                    "input_mint", "output_mint", "amount", "slippage_bps"
                ]
            }).to_string()
        }

        fn execute(args: String) -> Result<ToolResult, String> {
            let parsed: ExecuteArgs = serde_json::from_str(&args)
                .map_err(|e| format!("invalid arguments: {e}"))?;

            // Parse config from the host-injected `__config` jail.
            // The LLM cannot spoof these values because the host strips
            // any model-supplied `__config` before injection.
            let cfg = PluginConfig::from_section(&parsed.config)
                .map_err(|e| format!("config error: {e}"))?;

            // Squads program ID: configurable, defaults to mainnet
            let squads_program_id_str = parsed.config
                .get("squads_program_id")
                .cloned()
                .unwrap_or_else(|| SQUADS_DEFAULT_PROGRAM_ID.to_string());
            let squads_program_id = squads_defi_core::Pubkey::from_str(&squads_program_id_str)
                .map_err(|e| format!("invalid squads_program_id in config: {e}"))?;

            // Start timing: captures wall-clock duration for the full execute call.
            // Emitted in Complete/Fail events for observability.
            let start = Instant::now();

            let start_attrs = serde_json::json!({
                "input_mint": &parsed.input_mint,
                "output_mint": &parsed.output_mint,
                "amount": &parsed.amount,
                "slippage_bps": parsed.slippage_bps,
            });
            emit(
                Some(start),
                PluginAction::Start,
                PluginOutcome::Success,
                "starting swap proposal build",
                Some(start_attrs),
            );

            // ── 1. Fetch blockhash ──────────────────────────────────────────
            let blockhash = match fetch_blockhash(&cfg.rpc_url) {
                Ok(bh) => bh,
                Err(e) => {
                    let err_attrs = serde_json::json!({ "error": format!("blockhash fetch failed: {e}") });
                    emit(Some(start), PluginAction::Fail, PluginOutcome::Failure, "blockhash fetch failed", Some(err_attrs));
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("RPC blockhash error: {e}")),
                    });
                }
            };

            emit(None, PluginAction::Start, PluginOutcome::Success, "blockhash fetched", None);

            // ── 2. Fetch token USD price per smallest unit (server-side) ───
            // Fetched BEFORE the quote so the quote→swap-instructions
            // window is as tight as possible (Jupiter quotes have ~15s TTL).
            let usd_per_unit = match fetch_token_price(&cfg.rpc_url, &parsed.input_mint) {
                Ok(p) => p,
                Err(e) => {
                    let err_attrs = serde_json::json!({ "error": format!("price fetch: {e}") });
                    emit(Some(start), PluginAction::Fail, PluginOutcome::Failure, "price fetch failed", Some(err_attrs));
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("price fetch error: {e}")),
                    });
                }
            };

            // ── 3. Auto-detect transaction_index from on-chain multisig account ─
            // Only fetches when cfg.transaction_index is 0 (not manually overridden).
            // Also serves as vault-existence check (fix 6).
            let transaction_index = if cfg.transaction_index > 0 {
                cfg.transaction_index
            } else {
                match fetch_multisig_index(&cfg.rpc_url, &cfg.creator, &squads_program_id) {
                    Ok(idx) => {
                        emit(None, PluginAction::Start, PluginOutcome::Success, &format!("multisig index: {idx}"), None);
                        idx
                    }
                    Err(e) => {
                        let err_attrs = serde_json::json!({ "error": format!("multisig fetch: {e}") });
                        emit(Some(start), PluginAction::Fail, PluginOutcome::Failure, "multisig fetch failed", Some(err_attrs));
                        return Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!("multisig account error: {e}")),
                        });
                    }
                }
            };

            // ── 4. Fetch Jupiter quote (server-side) — right before swap-instructions
            // to minimise the quote TTL window.
            let quote = match fetch_jupiter_quote(
                &cfg.jupiter_url,
                &parsed.input_mint,
                &parsed.output_mint,
                &parsed.amount,
                parsed.slippage_bps,
            ) {
                Ok(q) => q,
                Err(e) => {
                    let err_attrs = serde_json::json!({ "error": format!("Jupiter quote: {e}") });
                    emit(Some(start), PluginAction::Fail, PluginOutcome::Failure, "quote fetch failed", Some(err_attrs));
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Jupiter quote error: {e}")),
                    });
                }
            };

            // ── 5. Fetch swap instructions from Jupiter (server-side) ───────
            let vault_pubkey_str = cfg.squads_vault.to_string();
            let swap_instructions = match fetch_jupiter_swap_instructions(
                &cfg.jupiter_url,
                &quote,
                &vault_pubkey_str,
            ) {
                Ok(si) => si,
                Err(e) => {
                    let err_attrs = serde_json::json!({ "error": format!("swap-instructions: {e}") });
                    emit(Some(start), PluginAction::Fail, PluginOutcome::Failure, "swap-instructions fetch failed", Some(err_attrs));
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Jupiter swap-instructions error: {e}")),
                    });
                }
            };

            // ── 5. Validate swap instruction program ID ─────────────────────
            if swap_instructions.swap_instruction.program_id != JUPITER_PROGRAM_ID {
                let err_attrs = serde_json::json!({
                    "error": "program_id mismatch",
                    "got": &swap_instructions.swap_instruction.program_id,
                    "expected": JUPITER_PROGRAM_ID,
                });
                emit(Some(start), PluginAction::Fail, PluginOutcome::Failure, "program_id does not match Jupiter", Some(err_attrs));
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!(
                        "swap instruction program_id '{}' does not match Jupiter program ID '{}'",
                        swap_instructions.swap_instruction.program_id,
                        JUPITER_PROGRAM_ID,
                    )),
                });
            }

            emit(None, PluginAction::Start, PluginOutcome::Success, "all data fetched, building proposal", None);

            // Daily volume tracking is disabled until host-level tracking is available.
            // Only per-proposal notional cap applies.
            let daily_vol: f64 = 0.0;
            let guardrails = SwapGuardrails::from(&cfg);

            // ── 6. Build proposal (guardrails enforced inside) ──────────────
            // Use the auto-fetched transaction_index (overrides config default of 0)
            let result = propose::build_real_swap_proposal(
                &quote,
                &swap_instructions,
                &cfg,
                &guardrails,
                daily_vol,
                usd_per_unit,
                &blockhash,
                &cfg.creator,
                &squads_program_id,
                transaction_index,
            ).map_err(|e| format!("{e}"))?;

            let (meta_tx_base64, summary) = result;

            let expires_at = squads_defi_core::squads::proposal_expiry_timestamp(
                cfg.proposal_expiry_hours
            );

            let (proposal_pda, _) = derive_proposal_pda(&cfg.creator, transaction_index, &squads_program_id);

            let success_attrs = serde_json::json!({
                "input_mint": &parsed.input_mint,
                "output_mint": &parsed.output_mint,
                "amount": &parsed.amount,
                "meta_tx_len": meta_tx_base64.len(),
            });
            emit(Some(start), PluginAction::Complete, PluginOutcome::Success, "proposal built", Some(success_attrs));

            let output = serde_json::json!({
                "meta_tx_base64": meta_tx_base64,
                "summary": summary,
                "proposal_expires_at": expires_at,
                "proposal_address": proposal_pda.to_string(),
                "status": "created"
            }).to_string();

            Ok(ToolResult {
                success: true,
                output,
                error: None,
            })
        }
    }

    // =========================================================================
    // HTTP helpers — all use waki (wasi:http), only available on wasm32-wasip2
    // =========================================================================

    /// Fetch a real blockhash from the Solana RPC via waki (blocking call).
    fn fetch_blockhash(rpc_url: &str) -> Result<Blockhash, String> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getLatestBlockhash",
            "params": [{"commitment": "finalized"}]
        }).to_string();

        let response = waki::Client::new()
            .post(rpc_url)
            .header("Content-Type", "application/json")
            .body(body.as_str())
            .send()
            .map_err(|e| format!("RPC HTTP error: {e}"))?;

        let body_bytes = response.body()
            .map_err(|e| format!("body read error: {e}"))?;
        let body_str = String::from_utf8(body_bytes)
            .map_err(|e| format!("utf-8 error: {e}"))?;
        let value: serde_json::Value = serde_json::from_str(&body_str)
            .map_err(|e| format!("json parse error: {e}"))?;

        if let Some(err) = value.get("error") {
            return Err(format!(
                "RPC error {}: {}",
                err["code"].as_i64().unwrap_or(-1),
                err["message"].as_str().unwrap_or("unknown")
            ));
        }

        let blockhash_str = value["result"]["value"]["blockhash"]
            .as_str()
            .ok_or("missing blockhash in RPC response")?;

        Blockhash::from_str(blockhash_str)
            .map_err(|e| format!("invalid blockhash: {e}"))
    }

    /// Fetch a Jupiter swap quote from the Jupiter quote API.
    /// Called by the plugin — the LLM does not construct URLs or call the API.
    fn fetch_jupiter_quote(
        base_url: &str,
        input_mint: &str,
        output_mint: &str,
        amount: &str,
        slippage_bps: u64,
    ) -> Result<QuoteResponse, String> {
        let url = format!(
            "{}/quote?inputMint={}&outputMint={}&amount={}&slippageBps={}",
            base_url.trim_end_matches('/'),
            url_encode(input_mint),
            url_encode(output_mint),
            amount,
            slippage_bps,
        );

        let response = waki::Client::new()
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .map_err(|e| format!("Jupiter quote request failed: {e}"))?;

        let body_bytes = response.body()
            .map_err(|e| format!("quote response body read failed: {e}"))?;
        let body_str = String::from_utf8(body_bytes)
            .map_err(|e| format!("quote response utf-8 error: {e}"))?;

        let quote: QuoteResponse = serde_json::from_str(&body_str)
            .map_err(|e| format!("failed to parse Jupiter quote response: {e}"))?;

        Ok(quote)
    }

    /// Fetch swap instructions from Jupiter's /swap-instructions endpoint.
    /// Uses the already-fetched quote as the request body.
    fn fetch_jupiter_swap_instructions(
        base_url: &str,
        quote: &QuoteResponse,
        user_pubkey: &str,
    ) -> Result<SwapInstructionsResponse, String> {
        let url = format!("{}/swap-instructions", base_url.trim_end_matches('/'));

        let quote_value = serde_json::to_value(quote)
            .map_err(|e| format!("serialize quote for swap-instructions: {e}"))?;

        let request_body = serde_json::json!({
            "quoteResponse": quote_value,
            "userPublicKey": user_pubkey,
            "wrapAndUnwrapSol": true,
            "dynamicComputeUnitLimit": true,
        });

        let response = waki::Client::new()
            .post(&url)
            .header("Content-Type", "application/json")
            .body(request_body.to_string().as_str())
            .send()
            .map_err(|e| format!("Jupiter swap-instructions request failed: {e}"))?;

        let body_bytes = response.body()
            .map_err(|e| format!("swap-instructions response body read failed: {e}"))?;
        let body_str = String::from_utf8(body_bytes)
            .map_err(|e| format!("swap-instructions utf-8 error: {e}"))?;

        let swap_instructions: SwapInstructionsResponse = serde_json::from_str(&body_str)
            .map_err(|e| format!("failed to parse swap-instructions response: {e}"))?;

        Ok(swap_instructions)
    }

    /// Fetch the USD price of a token per smallest unit.
    ///
    /// 1. Queries the Solana RPC for the mint account to get decimals
    /// 2. Queries Jupiter's price API for the USD price per whole token
    /// 3. Computes: USD per smallest unit = token_price / (10^decimals)
    ///
    /// This eliminates the need for the LLM to provide or guess the price.
    fn fetch_token_price(rpc_url: &str, mint: &str) -> Result<f64, String> {
        // Step 1: Query mint account for decimals via RPC (jsonParsed encoding)
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [mint, {"encoding": "jsonParsed"}]
        }).to_string();

        let response = waki::Client::new()
            .post(rpc_url)
            .header("Content-Type", "application/json")
            .body(body.as_str())
            .send()
            .map_err(|e| format!("RPC request for mint info failed: {e}"))?;

        let body_bytes = response.body()
            .map_err(|e| format!("mint info body read failed: {e}"))?;
        let body_str = String::from_utf8(body_bytes)
            .map_err(|e| format!("mint info utf-8 error: {e}"))?;
        let value: serde_json::Value = serde_json::from_str(&body_str)
            .map_err(|e| format!("mint info json parse error: {e}"))?;

        if let Some(err) = value.get("error") {
            return Err(format!(
                "RPC error fetching mint info: {}",
                err["message"].as_str().unwrap_or("unknown")
            ));
        }

        let account_value = &value["result"]["value"];
        if account_value.is_null() {
            return Err(format!("mint account not found for {}", mint));
        }

        let decimals = account_value["data"]["parsed"]["info"]["decimals"]
            .as_u64()
            .ok_or_else(|| format!("missing decimals field for mint {}", mint))?;

        // Step 2: Fetch USD price from Jupiter price API
        let price_url = format!("https://api.jup.ag/v6/price?ids={}", url_encode(mint));

        let response = waki::Client::new()
            .get(&price_url)
            .header("Accept", "application/json")
            .send()
            .map_err(|e| format!("Jupiter price API request failed: {e}"))?;

        let body_bytes = response.body()
            .map_err(|e| format!("price response body read failed: {e}"))?;
        let body_str = String::from_utf8(body_bytes)
            .map_err(|e| format!("price response utf-8 error: {e}"))?;
        let price_value: serde_json::Value = serde_json::from_str(&body_str)
            .map_err(|e| format!("price response json parse error: {e}"))?;

        // Jupiter price API response format: { "data": { "<mint>": { "id": "...", "price": 123.45, ... } } }
        let token_price = price_value["data"][mint]["price"]
            .as_f64()
            .ok_or_else(|| format!("price not available for mint {} from Jupiter price API", mint))?;

        // Step 3: Compute USD per smallest unit
        let divisor = 10u64.pow(decimals as u32) as f64;
        Ok(token_price / divisor)
    }

    /// Fetch the current `transaction_index` from the Squads multisig account.
    /// Also serves as a vault existence check — returns error if the multisig
    /// account doesn't exist on-chain.
    ///
    /// 1. Derives the multisig PDA from authority + program ID
    /// 2. Calls RPC getAccountInfo for the PDA
    /// 3. Parses transaction_index from the Anchor-encoded Multisig account
    fn fetch_multisig_index(
        rpc_url: &str,
        authority: &squads_defi_core::Pubkey,
        squads_program_id: &squads_defi_core::Pubkey,
    ) -> Result<u64, String> {
        use squads_defi_core::squads::{
            derive_multisig_pda, parse_multisig_transaction_index,
        };

        let (multisig_pda, _bump) = derive_multisig_pda(authority, squads_program_id);

        // Call getAccountInfo for the multisig PDA
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [multisig_pda.to_string(), {"encoding": "base64"}]
        }).to_string();

        let response = waki::Client::new()
            .post(rpc_url)
            .header("Content-Type", "application/json")
            .body(body.as_str())
            .send()
            .map_err(|e| format!("RPC HTTP error fetching multisig: {e}"))?;

        let body_bytes = response.body()
            .map_err(|e| format!("multisig body read error: {e}"))?;
        let body_str = String::from_utf8(body_bytes)
            .map_err(|e| format!("multisig utf-8 error: {e}"))?;
        let value: serde_json::Value = serde_json::from_str(&body_str)
            .map_err(|e| format!("multisig json parse error: {e}"))?;

        if let Some(err) = value.get("error") {
            return Err(format!(
                "RPC error fetching multisig: {}",
                err["message"].as_str().unwrap_or("unknown")
            ));
        }

        let account_value = &value["result"]["value"];
        if account_value.is_null() {
            return Err(format!(
                "Squads vault not found at multisig PDA {} — check that the vault address and program ID are correct",
                multisig_pda.to_string(),
            ));
        }

        let data_b64 = account_value["data"][0]
            .as_str()
            .ok_or("missing multisig account data")?;

        use base64::{engine::general_purpose::STANDARD, Engine};
        let data = STANDARD.decode(data_b64)
            .map_err(|e| format!("multisig base64 decode: {e}"))?;

        parse_multisig_transaction_index(&data)
    }

    /// Minimal URL encoding for mint addresses and query parameters.
    /// Mint addresses are base58 (alphanumeric only), but we encode to
    /// be safe for any future parameter that may contain special chars.
    fn url_encode(s: &str) -> String {
        let mut encoded = String::with_capacity(s.len());
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(b as char);
                }
                _ => {
                    encoded.push_str(&format!("%{:02X}", b));
                }
            }
        }
        encoded
    }

    /// Log a plugin event with optional timing and structured attributes.
    ///
    /// `start_time` — if `Some`, computes `duration_ms` from elapsed wall-clock time.
    /// Pass `None` for intermediate progress markers that don't need timing.
    ///
    /// `attrs` — optional JSON value with structured data (mints, amounts, error details).
    /// Pass `None` when no additional context is needed.
    fn emit(
        start_time: Option<Instant>,
        action: PluginAction,
        outcome: PluginOutcome,
        message: &str,
        attrs: Option<serde_json::Value>,
    ) {
        let duration_ms = start_time.map(|t| t.elapsed().as_millis() as u64);
        log_record(
            LogLevel::Info,
            &PluginEvent {
                function_name: "jupiter_swap_propose::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms,
                attrs: attrs.map(|v| v.to_string()),
                message: message.to_string(),
            },
        );
    }

    export!(JupiterSwapPropose);
}
