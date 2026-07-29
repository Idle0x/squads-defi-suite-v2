//! ZeroClaw WIT plugin: `solana-pay-request`.
//!
//! Build Solana Pay URLs for payment requests. The recipient is read from
//! the host-injected `__config` jail — the LLM cannot redirect payments.
//!
//! WIT world: `tool-plugin` exports `plugin-info` + `tool`.

pub mod pay;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "wit/v0",
        world: "tool-plugin",
        features: ["plugins-wit-v0"],
    });

    use crate::pay;
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
        /// Amount to request (optional — can be "any")
        amount: Option<String>,
        /// Token mint address (optional — omit for SOL)
        spl_token: Option<String>,
        /// Short label for the payment request
        label: Option<String>,
        /// Human-readable message
        message: Option<String>,
        /// Optional memo string
        memo: Option<String>,
        /// Host-injected config — NEVER in parameters_schema.
        /// Contains `recipient` (the only address payments can go to)
        /// and other operator-configured values.
        #[serde(rename = "__config", default)]
        config: HashMap<String, String>,
    }

    struct SolanaPayRequest;

    impl PluginInfo for SolanaPayRequest {
        fn plugin_name() -> String { "solana-pay-request".into() }
        fn plugin_version() -> String { env!("CARGO_PKG_VERSION").into() }
    }

    impl Tool for SolanaPayRequest {
        fn name() -> String { "solana-pay-request".to_string() }

        fn description() -> String {
            "Build a Solana Pay URL for a payment request. Returns a QR-code-ready \
             link that the recipient can scan to pay SOL or SPL tokens. \
             The recipient is enforced by operator config — the LLM cannot \
             redirect payments to a different address."
                .to_string()
        }

        fn parameters_schema() -> String {
            // `__config` is NEVER declared here. The host injects `recipient`
            // from config — the LLM cannot redirect payments.
            serde_json::json!({
                "type": "object",
                "properties": {
                    "amount": {
                        "type": ["string", "null"],
                        "description": "Payment amount in lamports (for SOL) or smallest units (for SPL). Omit for 'any' amount."
                    },
                    "spl_token": {
                        "type": ["string", "null"],
                        "description": "Mint address for SPL token payment (omit for SOL)"
                    },
                    "label": {
                        "type": ["string", "null"],
                        "description": "Short label for the payment"
                    },
                    "message": {
                        "type": ["string", "null"],
                        "description": "Human-readable message attached to the payment"
                    },
                    "memo": {
                        "type": ["string", "null"],
                        "description": "Optional memo string for on-chain reference"
                    }
                },
                "required": []
            }).to_string()
        }

        fn execute(args: String) -> Result<ToolResult, String> {
            let parsed: ExecuteArgs = serde_json::from_str(&args)
                .map_err(|e| format!("invalid arguments: {e}"))?;

            let start = Instant::now();

            let start_attrs = serde_json::json!({
                "has_amount": parsed.amount.is_some(),
                "has_token": parsed.spl_token.is_some(),
                "has_label": parsed.label.is_some(),
            });
            emit(Some(start), PluginAction::Start, PluginOutcome::Success, "building Solana Pay URL", Some(start_attrs));

            // Recipient MUST come from __config — NEVER from the LLM.
            // The host strips any model-supplied __config and injects
            // the real operator-configured recipient. This is the
            // critical anti-redirect guardrail.
            let recipient = parsed.config.get("recipient")
                .cloned()
                .ok_or_else(|| "missing `recipient` in config — operator must configure a payment destination".to_string())?;

            let amount_opt = parsed.amount.as_deref();
            let token_opt = parsed.spl_token.as_deref();
            let label_opt = parsed.label.as_deref();
            let msg_opt = parsed.message.as_deref();
            let memo_opt = parsed.memo.as_deref();

            let pay_url = pay::build_pay_url(
                &recipient, amount_opt, token_opt, label_opt, msg_opt, memo_opt,
            ).map_err(|e| {
                let err_attrs = serde_json::json!({ "error": &e });
                emit(Some(start), PluginAction::Fail, PluginOutcome::Failure, "failed to build pay URL", Some(err_attrs));
                e
            })?;

            let short_recipient = if recipient.len() > 8 {
                format!("{}...{}", &recipient[..8], &recipient[recipient.len().saturating_sub(4)..])
            } else {
                recipient.clone()
            };

            let summary = format!(
                "Payment Request\nRecipient: {}\nAmount: {}{}\nScan QR or open link to pay.",
                short_recipient,
                amount_opt.unwrap_or("any"),
                token_opt.map(|t| format!(" {t}")).unwrap_or_default(),
            );

            let output = serde_json::json!({
                "pay_url": pay_url,
                "summary": summary,
                "qr_data": pay_url,
            }).to_string();

            let success_attrs = serde_json::json!({
                "recipient": &short_recipient,
                "amount": amount_opt.unwrap_or("any"),
                "has_token": token_opt.is_some(),
            });
            emit(Some(start), PluginAction::Complete, PluginOutcome::Success, "Solana Pay URL built", Some(success_attrs));

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
                function_name: "solana_pay_request::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms,
                attrs: attrs.map(|v| v.to_string()),
                message: message.to_string(),
            },
        );
    }

    export!(SolanaPayRequest);
}
