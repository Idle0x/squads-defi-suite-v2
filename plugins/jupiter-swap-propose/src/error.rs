//! Error types for the jupiter-swap-propose plugin.

use thiserror::Error;

/// Errors that can occur during swap proposal building.
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("config error: {0}")]
    Config(String),

    #[error("guardrail violation: {0}")]
    Guardrail(String),

    #[error("swap error: {0}")]
    Swap(String),

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("invalid arguments: {0}")]
    InvalidArgs(String),
}

/// Guardrail-specific errors — returned when a check fails.
#[derive(Error, Debug)]
pub enum GuardrailError {
    #[error("output mint not in allowlist: {0}")]
    MintNotAllowed(String),

    #[error("slippage too high: got {got} bps, max {max} bps")]
    SlippageTooHigh { got: u64, max: u64 },

    #[error("notional value too high: got ${got}, max ${max}")]
    NotionalTooHigh { got: f64, max: u64 },

    #[error("daily cap exceeded: would spend ${would_spend}, cap ${cap}")]
    DailyCapExceeded { would_spend: f64, cap: u64 },

    #[error("missing RPC URL in config")]
    MissingRpcUrl,
}

/// Config parsing errors.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("missing required config key: {0}")]
    MissingKey(String),

    #[error("invalid value for key '{key}': {reason}")]
    InvalidValue { key: String, reason: String },
}
