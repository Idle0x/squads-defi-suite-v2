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
//! The plugin calls waki (wasi:http) directly for RPC calls via the
//! rpc_url from `__config`. All guardrails are enforced in Rust code
//! before any transaction is built.

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

    /// Arguments passed to `execute` from the host.
    /// `__config` is injected by the host, stripped from any model-supplied
    /// value, and contains the operator's configured guardrails.
    /// It is NEVER declared in `parameters_schema` — spoof-proof.
    #[derive(Deserialize)]
    struct ExecuteArgs {
        /// Jupiter quote API response as JSON string (host-fetched)
        quote_json: String,
        /// Jupiter swap-instructions response as JSON string (host-fetched)
        swap_instructions_json: String,
        /// Cumulative USD volume spent today (tracked by host across calls).
        /// NOT config — this is dynamic state. The WASM component is stateless
        /// so the host must track and inject this value.
        daily_volume_usd: String,
        /// USD price per base unit of the input token. Required for the
        /// notional guardrail and daily cap. The host (AI agent) fetches
        /// this from Jupiter's price API.
        usd_per_unit: Option<f64>,
        /// Host-injected config — NEVER in parameters_schema.
        /// The host strips any model-supplied `__config` and substitutes
        /// the real operator-configured values from plugins.entries.<name>.
        #[serde(rename = "__config", default)]
        config: HashMap<String, String>,
    }

    struct JupiterSwapPropose;

    const PLUGIN_NAME: &str = "jupiter-swap-propose";
    const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

    impl PluginInfo for JupiterSwapPropose {
        fn plugin_name() -> String { PLUGIN_NAME.to_string() }
        fn plugin_version() -> String { PLUGIN_VERSION.to_string() }
    }

    impl Tool for JupiterSwapPropose {
        fn name() -> String { "jupiter-swap-propose".to_string() }

        fn description() -> String {
            "Build a guarded Jupiter swap proposal wrapped in a Squads v4 multisig proposal. \
             Validates mint allowlist, slippage, price impact, route hops, notional, and daily cap \
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
                    "quote_json": {
                        "type": "string",
                        "description": "Full Jupiter quote API response as JSON (host-fetched from api.jup.ag)"
                    },
                    "swap_instructions_json": {
                        "type": "string",
                        "description": "Full Jupiter swap-instructions API response as JSON (host-fetched)"
                    },
                    "daily_volume_usd": {
                        "type": "string",
                        "description": "Cumulative daily swap volume in USD (tracked by host across calls)"
                    },
                    "usd_per_unit": {
                        "type": "number",
                        "description": "USD price per base unit of input token (host-provided from price API). Required for notional guardrail."
                    }
                },
                "required": [
                    "quote_json", "swap_instructions_json", "daily_volume_usd"
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
                .unwrap_or_else(|| "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf".to_string());
            let squads_program_id = squads_defi_core::Pubkey::from_str(&squads_program_id_str)
                .map_err(|e| format!("invalid squads_program_id in config: {e}"))?;

            // Blockhash fetch — uses rpc_url from __config, never env var
            let blockhash = match fetch_blockhash(&cfg.rpc_url) {
                Ok(bh) => bh,
                Err(e) => {
                    emit(PluginAction::Fail, PluginOutcome::Failure, &format!("blockhash fetch failed: {e}"));
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("RPC blockhash error: {e}")),
                    });
                }
            };

            // Parse the quote and instructions
            let quote_response: QuoteResponse = serde_json::from_str(&parsed.quote_json)
                .map_err(|e| format!("invalid quote JSON: {e}"))?;

            let swap_instructions: SwapInstructionsResponse =
                serde_json::from_str(&parsed.swap_instructions_json)
                    .map_err(|e| format!("invalid swap-instructions JSON: {e}"))?;

            let daily_vol: f64 = parsed.daily_volume_usd.parse().unwrap_or(0.0);

            let guardrails = SwapGuardrails::from(&cfg);

            let result = propose::build_real_swap_proposal(
                &quote_response,
                &swap_instructions,
                &cfg,
                &guardrails,
                daily_vol,
                parsed.usd_per_unit,
                &blockhash,
                &cfg.creator,
                &squads_program_id,
            ).map_err(|e| format!("{e}"))?;

            let (meta_tx_base64, summary) = result;

            let expires_at = squads_defi_core::squads::proposal_expiry_timestamp(
                cfg.proposal_expiry_hours
            );

            let (proposal_pda, _) = derive_proposal_pda(&cfg.creator, 0, &squads_program_id);

            emit(PluginAction::Complete, PluginOutcome::Success, "proposal built");

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

    fn emit(action: PluginAction, outcome: PluginOutcome, message: &str) {
        log_record(
            LogLevel::Info,
            &PluginEvent {
                function_name: "jupiter_swap_propose::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms: None,
                attrs: None,
                message: message.to_string(),
            },
        );
    }

    export!(JupiterSwapPropose);
}
