//! Token budget tests — verify all plugin output stays ≤200 tokens.

use squads_defi_core::shape;

#[test]
fn test_execute_output_under_200_tokens() {
    // Simulate a realistic execute output from the swap-propose plugin
    let output = shape::shape_summary(
        "Swap Proposal Built",
        vec![
            ("Input", "10 SOL (So11111111111111111111111111111111111111112)".to_string()),
            ("Output", "~230 USDC (EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v)".to_string()),
            ("Rate", "1 SOL ≈ 23.00 USDC".to_string()),
            ("Slippage", "0.50%".to_string()),
            ("Fee", "~$0.30 (network + AMM)".to_string()),
            ("Proposal", "Created on Squads v4 — review in app to sign".to_string()),
            ("Expires", "24 hours from now".to_string()),
        ],
        800,
    );

    let tokens = shape::count_tokens(&output);
    assert!(
        tokens <= 200,
        "execute output must be ≤200 tokens, got {} tokens in: {}",
        tokens,
        output
    );
}

#[test]
fn test_error_output_under_200_tokens() {
    let error_output = "Denied: output mint not in allowlist (EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v)";
    let tokens = shape::count_tokens(error_output);
    assert!(tokens <= 200);
}

#[test]
fn test_long_error_message_truncated() {
    let long_error = format!("Denied: {}", "very long error message ".repeat(100));
    let truncated = shape::truncate_to_token_budget(&long_error, 200);
    let tokens = shape::count_tokens(&truncated);
    assert!(tokens <= 200, "truncated error must fit budget");
}

// Total: 3 tests
