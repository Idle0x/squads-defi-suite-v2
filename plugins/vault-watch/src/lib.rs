//! ZeroClaw WIT plugin: `vault-watch`.
//!
//! Daily treasury briefings: pending Squads proposals, token balances,
//! and lending health factors — all in <=200 tokens.
//!
//! WIT world: `tool-plugin` exports `plugin-info` + `tool`.
//!
//! ## Security
//!
//! The vault address and RPC URL are injected via the host's `__config` jail.
//! The LLM never sees or controls these values. The `parameters_schema` never
//! declares `__config` — it is host-reserved and spoof-proof.

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

    /// `execute` arguments. Only `__config` carries the host-injected
    /// configuration (vault address, RPC URL). No business arguments needed
    /// for a daily briefing — it reads everything from config.
    #[derive(Deserialize)]
    struct ExecuteArgs {
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
                "properties": {}
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

            let vault_pk = match squads_defi_core::Pubkey::from_str(vault_str) {
                Ok(pk) => pk,
                Err(e) => {
                    emit(PluginAction::Fail, PluginOutcome::Failure, "invalid squads_vault in config");
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("invalid squads_vault: {e}")),
                    });
                }
            };

            // Fetch proposals
            let pending_proposals = proposals::fetch_pending_proposals(&rpc_url, &vault_pk)
                .unwrap_or_else(|e| {
                    emit(PluginAction::Fail, PluginOutcome::Failure, &format!("proposal fetch failed: {e}"));
                    vec![]
                });

            // Fetch balances
            let token_balances = balances::fetch_balances(&rpc_url, &vault_pk)
                .unwrap_or_default();

            // Fetch health factors
            let health_reports = health::fetch_health_factors(
                &rpc_url,
                &vault_pk,
                &token_balances,
            ).unwrap_or_default();

            // Format briefing
            let output = briefing::format_briefing(
                &pending_proposals,
                &token_balances,
                &health_reports,
            );

            emit(PluginAction::Complete, PluginOutcome::Success, "briefing generated");

            Ok(ToolResult {
                success: true,
                output,
                error: None,
            })
        }
    }

    fn emit(action: PluginAction, outcome: PluginOutcome, message: &str) {
        log_record(
            LogLevel::Info,
            &PluginEvent {
                function_name: "vault_watch::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms: None,
                attrs: None,
                message: message.to_string(),
            },
        );
    }

    export!(VaultWatch);
}
