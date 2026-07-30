//! Config parsing and guardrails struct for swap-propose.

use squads_defi_core::Pubkey;
use std::collections::HashMap;

use crate::error::ConfigError;

/// Plugin configuration parsed from the `__config` HashMap.
pub struct PluginConfig {
    /// Solana RPC URL.
    pub rpc_url: String,
    /// Jupiter API base URL.
    pub jupiter_url: String,
    /// The Squads multisig vault address.
    pub squads_vault: Pubkey,
    /// Creator wallet address (the proposer).
    pub creator: Pubkey,
    /// Allowed output mint addresses (empty = deny all).
    pub mint_allowlist: Vec<Pubkey>,
    /// Maximum slippage in basis points (e.g., 100 = 1%).
    pub max_slippage_bps: u64,
    /// Maximum swap notional value in USD.
    pub max_notional_usd: u64,
    /// Per-day spending cap in USD (resets at midnight UTC).
    pub per_day_cap_usd: u64,
    /// Proposal expiry in hours.
    pub proposal_expiry_hours: u64,
    /// Current transaction index from the multisig account.
    /// Default 0. Must be fetched on-chain for vaults with existing proposals.
    pub transaction_index: u64,
}

impl PluginConfig {
    /// Parse config from a HashMap of string key-value pairs.
    /// `__config` is injected by the host and is spoof-proof.
    pub fn from_section(section: &HashMap<String, String>) -> Result<Self, ConfigError> {
        let rpc_url = section
            .get("rpc_url")
            .cloned()
            .ok_or_else(|| ConfigError::MissingKey("rpc_url".to_string()))?;

        if !rpc_url.starts_with("https://") {
            return Err(ConfigError::InvalidValue {
                key: "rpc_url".to_string(),
                reason: format!("must use HTTPS, got '{}'", rpc_url),
            });
        }

        let jupiter_url = section
            .get("jupiter_url")
            .cloned()
            .unwrap_or_else(|| "https://quote-api.jup.ag/v6".to_string());

        if !jupiter_url.starts_with("https://") {
            return Err(ConfigError::InvalidValue {
                key: "jupiter_url".to_string(),
                reason: format!("must use HTTPS, got '{}'", jupiter_url),
            });
        }

        let squads_vault_str = section
            .get("squads_vault")
            .cloned()
            .ok_or_else(|| ConfigError::MissingKey("squads_vault".to_string()))?;
        let squads_vault = Pubkey::from_str(&squads_vault_str)
            .map_err(|e| ConfigError::InvalidValue {
                key: "squads_vault".to_string(),
                reason: e,
            })?;

        let creator = section
            .get("creator")
            .map(|s| Pubkey::from_str(s))
            .transpose()
            .map_err(|e| ConfigError::InvalidValue {
                key: "creator".to_string(),
                reason: e,
            })?
            .unwrap_or_else(|| squads_vault);

        let mint_allowlist = section
            .get("mint_allowlist")
            .map(|s| {
                s.split(',')
                    .filter(|m| !m.trim().is_empty())
                    .map(|m| Pubkey::from_str(m.trim()))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
            .map_err(|e| ConfigError::InvalidValue {
                key: "mint_allowlist".to_string(),
                reason: e.to_string(),
            })?
            .unwrap_or_default();

        let max_slippage_bps = section
            .get("max_slippage_bps")
            .map(|v| v.parse::<u64>())
            .transpose()
            .map_err(|e| ConfigError::InvalidValue {
                key: "max_slippage_bps".to_string(),
                reason: e.to_string(),
            })?
            .unwrap_or(100);

        let max_notional_usd = section
            .get("max_notional_usd")
            .map(|v| v.parse::<u64>())
            .transpose()
            .map_err(|e| ConfigError::InvalidValue {
                key: "max_notional_usd".to_string(),
                reason: e.to_string(),
            })?
            .unwrap_or(1000);

        let per_day_cap_usd = section
            .get("per_day_cap_usd")
            .map(|v| v.parse::<u64>())
            .transpose()
            .map_err(|e| ConfigError::InvalidValue {
                key: "per_day_cap_usd".to_string(),
                reason: e.to_string(),
            })?
            .unwrap_or(10000);

        let proposal_expiry_hours = section
            .get("proposal_expiry_hours")
            .map(|v| v.parse::<u64>())
            .transpose()
            .map_err(|e| ConfigError::InvalidValue {
                key: "proposal_expiry_hours".to_string(),
                reason: e.to_string(),
            })?
            .unwrap_or(24);

        // Validate: must be between 1 and 168 hours (1 week max)
        if proposal_expiry_hours < 1 || proposal_expiry_hours > 168 {
            return Err(ConfigError::InvalidValue {
                key: "proposal_expiry_hours".to_string(),
                reason: format!(
                    "must be between 1 and 168 hours, got {}",
                    proposal_expiry_hours
                ),
            });
        }

        let transaction_index = section
            .get("transaction_index")
            .map(|v| v.parse::<u64>())
            .transpose()
            .map_err(|e| ConfigError::InvalidValue {
                key: "transaction_index".to_string(),
                reason: e.to_string(),
            })?
            .unwrap_or(0);

        Ok(Self {
            rpc_url,
            jupiter_url,
            squads_vault,
            creator,
            mint_allowlist,
            max_slippage_bps,
            max_notional_usd,
            per_day_cap_usd,
            proposal_expiry_hours,
            transaction_index,
        })
    }
}

/// Guardrails enforced in Rust code — NOT the LLM prompt.
pub struct SwapGuardrails {
    pub max_slippage_bps: u64,
    pub max_notional_usd: u64,
    pub mint_allowlist: Vec<Pubkey>,
    pub per_day_cap_usd: u64,
}

impl From<&PluginConfig> for SwapGuardrails {
    fn from(cfg: &PluginConfig) -> Self {
        Self {
            max_slippage_bps: cfg.max_slippage_bps,
            max_notional_usd: cfg.max_notional_usd,
            mint_allowlist: cfg.mint_allowlist.clone(),
            per_day_cap_usd: cfg.per_day_cap_usd,
        }
    }
}
