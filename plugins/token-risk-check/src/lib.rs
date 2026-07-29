//! ZeroClaw WIT plugin: `token-risk-check`.
//!
//! Analyze SPL token risk factors by querying on-chain mint account data.
//! The RPC URL is injected via the host's `__config` jail.
//! Returns real on-chain data or honest errors — never fake samples.
//!
//! WIT world: `tool-plugin` exports `plugin-info` + `tool`.

pub mod token;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "wit/v0",
        world: "tool-plugin",
        features: ["plugins-wit-v0"],
    });

    use crate::token;
    use base64::Engine;
    use exports::zeroclaw::plugin::plugin_info::Guest as PluginInfo;
    use exports::zeroclaw::plugin::tool::{Guest as Tool, ToolResult};
    use zeroclaw::plugin::logging::{
        log_record, LogLevel, PluginAction, PluginEvent, PluginOutcome,
    };
    use serde::Deserialize;
    use std::collections::HashMap;
    use std::time::Instant;

    #[derive(Deserialize)]
    struct ExecuteArgs {
        /// The mint address to analyze (base58)
        mint_address: String,
        /// Host-injected config — NEVER in parameters_schema.
        /// Contains `rpc_url` and other operator-configured values.
        #[serde(rename = "__config", default)]
        config: HashMap<String, String>,
    }

    struct TokenRiskCheck;

    impl PluginInfo for TokenRiskCheck {
        fn plugin_name() -> String { "token-risk-check".into() }
        fn plugin_version() -> String { env!("CARGO_PKG_VERSION").into() }
    }

    impl Tool for TokenRiskCheck {
        fn name() -> String { "token-risk-check".to_string() }

        fn description() -> String {
            "Analyze an SPL token's risk factors by querying on-chain mint account \
             data. Checks mint authority, freeze authority, Token-2022 extensions, \
             and holder concentration. Returns real on-chain data or an honest error."
                .to_string()
        }

        fn parameters_schema() -> String {
            // `__config` is NEVER declared here. The host injects `rpc_url`
            // from config — the LLM cannot redirect to a malicious RPC.
            serde_json::json!({
                "type": "object",
                "properties": {
                    "mint_address": {
                        "type": "string",
                        "description": "Base58-encoded mint account address to analyze"
                    }
                },
                "required": ["mint_address"]
            }).to_string()
        }

        fn execute(args: String) -> Result<ToolResult, String> {
            let parsed: ExecuteArgs = serde_json::from_str(&args)
                .map_err(|e| format!("invalid arguments: {e}"))?;

            // RPC URL comes from __config jail — never from the LLM
            let rpc_url = parsed.config.get("rpc_url")
                .cloned()
                .ok_or_else(|| "missing `rpc_url` in config".to_string())?;

            // HTTPS validation (D2)
            if !rpc_url.starts_with("https://") {
                return Err("rpc_url must use HTTPS".to_string());
            }

            let start = Instant::now();

            let start_attrs = serde_json::json!({
                "mint_address": &parsed.mint_address,
            });
            emit(Some(start), PluginAction::Start, PluginOutcome::Success, "starting risk check", Some(start_attrs));

            let mint_pk = squads_defi_core::Pubkey::from_str(&parsed.mint_address)
                .map_err(|e| format!("invalid mint address: {e}"))?;

            // Query the mint account on-chain via RPC
            let mint_data = match fetch_mint_account(&rpc_url, &mint_pk.to_string()) {
                Ok(data) => data,
                Err(e) => {
                    let err_attrs = serde_json::json!({ "error": format!("mint query: {e}") });
                    emit(Some(start), PluginAction::Fail, PluginOutcome::Failure, "mint query failed", Some(err_attrs));
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("mint account query failed: {e}")),
                    });
                }
            };

            let risk = token::assess_risk_from_mint_data(&mint_data, &mint_pk);
            let risk_level = token::assess_risk(&risk);
            let summary = token::format_risk_summary(
                &parsed.mint_address,
                None,
                &risk,
            );

            let success_attrs = serde_json::json!({
                "mint_address": &parsed.mint_address,
                "risk_level": risk_level.as_str(),
                "has_mint_authority": risk.mint_authority.is_some(),
                "has_freeze_authority": risk.freeze_authority.is_some(),
                "is_token22": risk.is_token22,
            });
            emit(Some(start), PluginAction::Complete, PluginOutcome::Success, "risk check completed", Some(success_attrs));

            Ok(ToolResult {
                success: true,
                output: summary,
                error: None,
            })
        }
    }

    /// Fetch a mint account's raw data from the Solana RPC via waki.
    fn fetch_mint_account(rpc_url: &str, pubkey: &str) -> Result<Vec<u8>, String> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [pubkey, {"encoding": "base64"}]
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

        let account_value = &value["result"]["value"];
        if account_value.is_null() {
            return Err("mint account not found".to_string());
        }

        let data_b64 = account_value["data"][0]
            .as_str()
            .ok_or("missing account data in response")?;

        let data = base64::engine::general_purpose::STANDARD
            .decode(data_b64)
            .map_err(|e| format!("base64 decode: {e}"))?;

        Ok(data)
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
                function_name: "token_risk_check::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms,
                attrs: attrs.map(|v| v.to_string()),
                message: message.to_string(),
            },
        );
    }

    export!(TokenRiskCheck);
}
