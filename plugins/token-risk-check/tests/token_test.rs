use token_risk_check::token::{assess_risk, RiskLevel, TokenRiskInput};
use squads_defi_core::Pubkey;

fn low_risk_input() -> TokenRiskInput {
    TokenRiskInput {
        mint_authority: None,
        freeze_authority: None,
        holder_concentration_pct: 23.0,
        is_token22: false,
        has_transfer_hook: false,
        has_transfer_fee: false,
        has_permanent_delegate: false,
        lp_exists: true,
    }
}

#[test] fn test_revoked_authority_is_low_risk() {
    assert_eq!(assess_risk(&low_risk_input()), RiskLevel::Low);
}

#[test] fn test_active_freeze_authority_is_high_risk() {
    let mut input = low_risk_input();
    input.freeze_authority = Some(Pubkey::new([1u8; 32]));
    assert_eq!(assess_risk(&input), RiskLevel::High);
}

#[test] fn test_concentration_above_80_is_high_risk() {
    let mut input = low_risk_input();
    input.holder_concentration_pct = 85.0;
    assert_eq!(assess_risk(&input), RiskLevel::High);
}

#[test] fn test_active_mint_authority_is_medium_risk() {
    let mut input = low_risk_input();
    input.mint_authority = Some(Pubkey::new([1u8; 32]));
    assert_eq!(assess_risk(&input), RiskLevel::Medium);
}
