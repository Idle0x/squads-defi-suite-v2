//! ZeroClaw WIT plugin: `vault-watch`.
//!
//! Daily treasury briefings: pending Squads proposals, token balances,
//! and lending health factors — all in <=200 tokens.
//!
//! WIT world: `tool-plugin` exports `plugin-info` + `tool`.
//!
//! ## Security
//!
//! The vault address, RPC URL, and Squads program ID are injected via the
//! host's `__config` jail. The LLM never sees or controls these values.
//! The `parameters_schema` never declares `__config` — it is host-reserved
//! and spoof-proof.

pub mod balances;
pub mod briefing;
pub mod health;
pub mod proposals;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "wit/v0",
        world: "tool-plugin",
        features: ["plugins-wit-v0"],
    });

    use crate::balances;
    use crate::briefing;
    use crate::health;
    use crate::proposals;
    use exports::zeroclaw::plugin::plugin_info::Guest as PluginInfo;
    use exports::zeroclaw::plugin::tool::{Guest as Tool, ToolResult};
    use zeroclaw::plugin::logging::{
        log_record, LogLevel, PluginAction, PluginEvent, PluginOutcome,
    };
    use serde::Deserialize;
    use std::collections::HashMap;
    use std::time::Instant;

    /// Default Squads v4 program ID (mainnet).
    const SQUADS_DEFAULT_PROGRAM_ID: &str = "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf";

    /// `execute` arguments.
    /// `__config` carries the host-injected configuration (vault address, RPC URL).
    /// `refresh` is an optional flag to force a full re-fetch.
    #[derive(Deserialize)]
    struct ExecuteArgs {
        /// If true, force a full re-fetch of all on-chain data.
        /// If false or absent, may return cached data (currently always fresh).
        refresh: Option<bool>,
        /// Host-injected config — NEVER in parameters_schema.
        /// The host strips any model-supplied `__config` and substitutes
        /// the real operator-configured values.
        #[serde(rename = "__config", default)]
        config: HashMap<String, String>,
    }

    struct VaultWatch;

    const PLUGIN_NAME: &str = "vault-watch";
    const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

    impl PluginInfo for VaultWatch {
        fn plugin_name() -> String { PLUGIN_NAME.to_string() }
        fn plugin_version() -> String { PLUGIN_VERSION.to_string() }
    }

    impl Tool for VaultWatch {
        fn name() -> String { "vault-watch".to_string() }

        fn description() -> String {
            "Generate a daily treasury briefing for a Squads vault — pending proposals, \
             token balances, and lending health factors. All data fetched on-chain \
             via Solana RPC. Returns a structured summary under 200 tokens."
                .to_string()
        }

        fn parameters_schema() -> String {
            // `__config` is NEVER declared here. Host-injected and spoof-proof.
            serde_json::json!({
                "type": "object",
                "properties": {
                    "refresh": {
                        "type": "boolean",
                        "description": "If true, force a full re-fetch of all on-chain data. If false or absent, returns current data."
                    }
                },
                "required": []
            }).to_string()
        }

        fn execute(args: String) -> Result<ToolResult, String> {
            let parsed: ExecuteArgs = serde_json::from_str(&args)
                .map_err(|e| format!("invalid arguments: {e}"))?;

            // Read vault address and RPC URL from __config jail
            let vault_str = parsed.config.get("squads_vault")
                .ok_or_else(|| "missing `squads_vault` in config".to_string())?;

            let rpc_url = parsed.config.get("rpc_url")
                .cloned()
                .ok_or_else(|| "missing `rpc_url` in config".to_string())?;

            // HTTPS validation (D2)
            if !rpc_url.starts_with("https://") {
                return Err("rpc_url must use HTTPS".to_string());
            }

            let vault_pk = match squads_defi_core::Pubkey::from_str(vault_str) {
                Ok(pk) => pk,
                Err(e) => {
                    emit(None, PluginAction::Fail, PluginOutcome::Failure, "invalid squads_vault in config", None);
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("invalid squads_vault: {e}")),
                    });
                }
            };

            // Squads program ID: configurable, defaults to mainnet (N12)
            let squads_program_id_str = parsed.config
                .get("squads_program_id")
                .cloned()
                .unwrap_or_else(|| SQUADS_DEFAULT_PROGRAM_ID.to_string());
            let squads_program_id = squads_defi_core::Pubkey::from_str(&squads_program_id_str)
                .map_err(|e| format!("invalid squads_program_id: {e}"))?;

            let start = Instant::now();

            let start_attrs = serde_json::json!({
                "vault": vault_str,
                "refresh": parsed.refresh.unwrap_or(false),
            });
            emit(Some(start), PluginAction::Start, PluginOutcome::Success, "starting vault briefing", Some(start_attrs));

            // Collect warnings from partial failures (E1 fix)
            let mut warnings: Vec<String> = Vec::new();

            // Fetch proposals — errors become warnings, not silent empty vec
            let pending_proposals = match proposals::fetch_pending_proposals(&rpc_url, &vault_pk, &squads_program_id) {
                Ok(p) => p,
                Err(e) => {
                    warnings.push(format!("proposals: {e}"));
                    emit(None, PluginAction::Start, PluginOutcome::Success, &format!("proposals fetch warning: {e}"), None);
                    vec![]
                }
            };

            // Fetch balances — errors become warnings
            let token_balances = match balances::fetch_balances(&rpc_url, &vault_pk) {
                Ok(b) => b,
                Err(e) => {
                    warnings.push(format!("balances: {e}"));
                    emit(None, PluginAction::Start, PluginOutcome::Success, &format!("balances fetch warning: {e}"), None);
                    vec![]
                }
            };

            // Fetch health factors — errors become warnings
            let health_reports = match health::fetch_health_factors(&rpc_url, &vault_pk, &token_balances) {
                Ok(h) => h,
                Err(e) => {
                    warnings.push(format!("health: {e}"));
                    emit(None, PluginAction::Start, PluginOutcome::Success, &format!("health fetch warning: {e}"), None);
                    vec![]
                }
            };

            // Format briefing
            let briefing_text = briefing::format_briefing(
                &pending_proposals,
                &token_balances,
                &health_reports,
            );

            // Build output: plain text with optional warning prefix.
            // Using plain text (not JSON envelope) ensures the total output
            // stays within the 200-token budget. Warnings are not silently
            // dropped — they're prepended when present (E1 fix).
            let output = if warnings.is_empty() {
                briefing_text
            } else {
                let warning_line = format!("⚠️ Warnings: {}\n\n", warnings.join("; "));
                let combined = format!("{}{}", warning_line, briefing_text);
                // If warnings push over the token budget, drop them and
                // return just the briefing text.
                if squads_defi_core::shape::count_tokens(&combined) > squads_defi_core::shape::MAX_OUTPUT_TOKENS {
                    briefing_text
                } else {
                    combined
                }
            };

            let success_attrs = serde_json::json!({
                "vault": vault_str,
                "proposals": pending_proposals.len(),
                "balances": token_balances.len(),
                "health_reports": health_reports.len(),
                "warnings": warnings.len(),
            });
            emit(Some(start), PluginAction::Complete, PluginOutcome::Success, "briefing generated", Some(success_attrs));

            Ok(ToolResult {
                success: true,
                output,
                error: None,
            })
        }
    }

    /// Log a plugin event with optional timing and structured attributes.
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
                function_name: "vault_watch::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms,
                attrs: attrs.map(|v| v.to_string()),
                message: message.to_string(),
            },
        );
    }

    export!(VaultWatch);
}
