//! Guardrail enforcement — all checks happen in Rust, not the LLM.
//!
//! Four checks that MUST pass before any swap proposal is built:
//! 1. Output mint must be in the configured allowlist
//! 2. Slippage must not exceed the configured max
//! 3. Notional USD value must not exceed the configured max
//! 4. Daily spending cap must not be exceeded

use squads_defi_core::jupiter::Quote;

use crate::config::SwapGuardrails;
use crate::error::GuardrailError;

impl SwapGuardrails {
    /// Check the quote against all four guardrails.
    /// Returns `Ok(())` if all pass, or the first `GuardrailError` encountered.
    pub fn check(&self, quote: &Quote, daily_spent_usd: f64) -> Result<(), GuardrailError> {
        // 1. Mint allowlist: output mint must be in the configured list.
        //    Empty allowlist = deny everything (safe default).
        if !self.mint_allowlist.is_empty() {
            let output_mint_str = quote.output_mint.to_string();
            if !self
                .mint_allowlist
                .iter()
                .any(|m| m.to_string() == output_mint_str)
            {
                return Err(GuardrailError::MintNotAllowed(output_mint_str));
            }
        } else {
            return Err(GuardrailError::MintNotAllowed(
                "empty mint allowlist — no mints permitted".to_string(),
            ));
        }

        // 2. Max slippage: quote slippage must be ≤ configured max.
        if quote.slippage_bps > self.max_slippage_bps {
            return Err(GuardrailError::SlippageTooHigh {
                got: quote.slippage_bps,
                max: self.max_slippage_bps,
            });
        }

        // 3. Max notional: swap value must be ≤ configured max.
        if quote.notional_usd > self.max_notional_usd as f64 {
            return Err(GuardrailError::NotionalTooHigh {
                got: quote.notional_usd,
                max: self.max_notional_usd,
            });
        }

        // 4. Daily cap: cumulative today + this swap must be ≤ configured cap.
        let would_spend = daily_spent_usd + quote.notional_usd;
        if would_spend > self.per_day_cap_usd as f64 {
            return Err(GuardrailError::DailyCapExceeded {
                would_spend,
                cap: self.per_day_cap_usd,
            });
        }

        Ok(())
    }

    /// Check mint allowlist only (used in injection tests).
    pub fn check_mint(&self, quote: &Quote) -> Result<(), GuardrailError> {
        if self.mint_allowlist.is_empty() {
            return Err(GuardrailError::MintNotAllowed(
                "empty mint allowlist — no mints permitted".to_string(),
            ));
        }
        let output_mint_str = quote.output_mint.to_string();
        if !self
            .mint_allowlist
            .iter()
            .any(|m| m.to_string() == output_mint_str)
        {
            return Err(GuardrailError::MintNotAllowed(output_mint_str));
        }
        Ok(())
    }
}
