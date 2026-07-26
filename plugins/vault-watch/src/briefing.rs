//! Format all vault data into a ≤200 token daily briefing.

use squads_defi_core::shape::{self, truncate_to_token_budget, MAX_OUTPUT_TOKENS};

use crate::balances::TokenBalance;
use crate::health::HealthReport;
use crate::proposals::{PendingProposal, count_by_status, expiring_soon};

const BRIEFING_MAX_CHARS: usize = MAX_OUTPUT_TOKENS * 4;

/// Produce a formatted daily treasury briefing.
pub fn format_briefing(
    proposals: &[PendingProposal],
    balances: &[TokenBalance],
    health_reports: &[HealthReport],
) -> String {
    let (pending, executed, approved) = count_by_status(proposals);
    let expiring = expiring_soon(proposals, 24);
    let at_risk = crate::health::at_risk_positions(health_reports);
    let total_usd = 0.0; // USD values removed — no hardcoded prices

    let mut sections: Vec<(&str, String)> = Vec::new();

    // Proposals section
    if !proposals.is_empty() {
        sections.push((
            "Proposals",
            format!(
                "{pending} pending, {approved} ready, {executed} executed{}",
                if expiring.is_empty() {
                    String::new()
                } else {
                    format!(", {} expiring soon", expiring.len())
                }
            ),
        ));
    } else {
        sections.push(("Proposals", "No proposals found".to_string()));
    }

    // Balances section
    if !balances.is_empty() {
        let balance_strs: Vec<String> = balances.iter().map(|b| b.formatted()).collect();
        sections.push(("Balances", balance_strs.join(" | ")));
    } else {
        sections.push(("Balances", "No balances available".to_string()));
    }

    // Health section
    if !health_reports.is_empty() {
        let health_strs: Vec<String> = health_reports.iter().map(|h| h.summary()).collect();
        sections.push(("Health", health_strs.join("; ")));

        if !at_risk.is_empty() {
            sections.push((
                "⚠️ WARNING",
                format!("{} position(s) at risk", at_risk.len()),
            ));
        }
    } else {
        sections.push(("Health", "No lending positions tracked".to_string()));
    }

    // Total value
    if total_usd > 0.0 {
        sections.push(("Total", format!("${:.2}", total_usd)));
    }

    let result = shape::shape_summary("Daily Treasury Briefing", sections, BRIEFING_MAX_CHARS);

    // Final safety: ensure under 200 tokens
    if shape::count_tokens(&result) > MAX_OUTPUT_TOKENS {
        truncate_to_token_budget(&result, MAX_OUTPUT_TOKENS)
    } else {
        result
    }
}
