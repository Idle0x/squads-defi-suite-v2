//! Token budget tests for vault-watch — all output must stay ≤200 tokens.

use squads_defi_core::shape;
use vault_watch::briefing;

#[test]
fn test_execute_output_under_200_tokens() {
    // Generate a briefing and verify it stays under 200 tokens
    let result = briefing::format_briefing(&[], &[], &[]);
    let tokens = shape::count_tokens(&result);
    assert!(tokens <= 200, "briefing must be ≤200 tokens, got {tokens}");
}

#[test]
fn test_large_briefing_truncated_to_budget() {
    // Generate a briefing with simulated data to verify truncation
    use vault_watch::proposals::PendingProposal;
    use squads_defi_core::Pubkey;
    use vault_watch::balances::TokenBalance;
    use vault_watch::health::HealthReport;

    // Create many proposals
    let proposals: Vec<PendingProposal> = (0..10)
        .map(|i| PendingProposal {
            proposal_pubkey: Pubkey::new([i as u8; 32]),
            multisig: Pubkey::new([1u8; 32]),
            creator: Pubkey::new([2u8; 32]),
            expiry_timestamp: 1_760_000_000,
            approvals: i,
            threshold: 5,
            title: Some(format!("Proposal {}", i)),
            executed: false,
        })
        .collect();

    // Create many balances
    let balances: Vec<TokenBalance> = (0..10)
        .map(|i| TokenBalance {
            mint: Pubkey::new([i as u8; 32]),
            symbol: Some(format!("TOKEN{}", i)),
            amount: 1_000_000_000,
            decimals: 6,
            usd_value: Some(100.0 * i as f64),
        })
        .collect();

    // Create health reports
    let health_reports: Vec<HealthReport> = (0..5)
        .map(|i| HealthReport {
            protocol: format!("Protocol{}", i),
            position_pubkey: Pubkey::new([i as u8; 32]),
            borrowed_mint: Pubkey::new([100 + i as u8; 32]),
            collateral_mint: Pubkey::new([200 + i as u8; 32]),
            health_factor: 1.0 + i as f64 * 0.2,
            borrowed_usd: 500.0,
            collateral_usd: 750.0,
        })
        .collect();

    let result = briefing::format_briefing(&proposals, &balances, &health_reports);
    let tokens = shape::count_tokens(&result);
    assert!(
        tokens <= 200,
        "large briefing must be truncated to ≤200 tokens, got {tokens}: {result}"
    );
}

// Total: 2 tests
