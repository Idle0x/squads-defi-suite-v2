//! Swap construction tests — TDD contracts for swap tx building.

use swap_propose::swap;
use squads_defi_core::jupiter::Quote;
use squads_defi_core::types::MessageHeader;
use squads_defi_core::{Blockhash, Pubkey};

/// Helper: create a test quote (replaces deleted make_test_quote).
fn test_quote(output_mint: Pubkey, slippage_bps: u64, notional_usd: f64) -> Quote {
    let swap_tx = squads_defi_core::Transaction::new_unsigned(
        squads_defi_core::tx::Message {
            header: MessageHeader {
                num_required_signatures: 1,
                num_readonly_signed_accounts: 0,
                num_readonly_unsigned_accounts: 0,
            },
            account_keys: vec![Pubkey::new([1u8; 32])],
            instructions: vec![squads_defi_core::tx::CompiledInstruction {
                program_id_index: 0,
                accounts: vec![0],
                data: vec![1, 2, 3],
            }],
            recent_blockhash: Blockhash::new([0u8; 32]),
            address_table_lookups: vec![],
        },
    );
    Quote {
        input_mint: Pubkey::new([1u8; 32]),
        output_mint,
        in_amount: 1_000_000_000,
        out_amount: 950_000_000,
        other_amount_threshold: 900_000_000,
        slippage_bps,
        notional_usd,
        swap_transaction: Some(swap_tx.to_base64()),
        route_plan: vec![],
    }
}

#[test]
fn test_build_swap_transaction_from_quote() {
    let quote = test_quote(
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        50,
        100.0,
    );
    let user = Pubkey::new([1u8; 32]);
    let blockhash = Blockhash::new([2u8; 32]);

    let result = swap::build_swap_transaction(&quote, &user, blockhash);
    assert!(result.is_ok(), "valid quote must produce a transaction");
    let txn = result.unwrap();
    assert!(!txn.message.instructions.is_empty());
}

#[test]
fn test_build_swap_transaction_fails_without_swap_data() {
    let mut quote = test_quote(
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        50,
        100.0,
    );
    quote.swap_transaction = None;

    let user = Pubkey::new([1u8; 32]);
    let blockhash = Blockhash::new([2u8; 32]);

    let result = swap::build_swap_transaction(&quote, &user, blockhash);
    assert!(result.is_err(), "quote without swap data must error");
}

#[test]
fn test_test_quote_has_valid_fields() {
    let quote = test_quote(
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
        100,
        50.0,
    );
    assert_eq!(quote.slippage_bps, 100);
    assert_eq!(quote.notional_usd, 50.0);
    assert!(quote.swap_transaction.is_some());
    assert!(quote.in_amount > 0);
}

#[test]
fn test_transaction_preserves_user_as_signer() {
    let quote = test_quote(
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        50,
        100.0,
    );
    let user = Pubkey::new([42u8; 32]);
    let blockhash = Blockhash::new([2u8; 32]);

    let result = swap::build_swap_transaction(&quote, &user, blockhash);
    assert!(result.is_ok());
    let txn = result.unwrap();
    // account_keys includes payer + program ID (compile_instructions deduplicates)
    assert!(txn.message.account_keys.len() >= 1, "account_keys must have at least the user");
    assert_eq!(txn.message.account_keys[0], user);
}

#[test]
fn test_build_swap_transaction_with_zero_amount() {
    let mut quote = test_quote(
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        50,
        0.0,
    );
    quote.in_amount = 0;
    let user = Pubkey::new([1u8; 32]);
    let blockhash = Blockhash::new([2u8; 32]);

    let result = swap::build_swap_transaction(&quote, &user, blockhash);
    assert!(result.is_ok(), "zero-amount swap should still produce a transaction");
}

#[test]
fn test_build_swap_transaction_with_empty_route_plan() {
    let mut quote = test_quote(
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        50,
        100.0,
    );
    quote.route_plan = vec![];
    let user = Pubkey::new([1u8; 32]);
    let blockhash = Blockhash::new([2u8; 32]);

    let result = swap::build_swap_transaction(&quote, &user, blockhash);
    assert!(result.is_ok());
}

#[test]
fn test_test_quote_output_mint_matches_input() {
    let output_mint = Pubkey::from_str("DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263").unwrap();
    let quote = test_quote(output_mint, 100, 50.0);
    assert_eq!(quote.output_mint, output_mint);
    assert_eq!(quote.input_mint, Pubkey::new([1u8; 32]));
}

#[test]
fn test_transaction_base64_output_nonempty() {
    let quote = test_quote(
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        50,
        100.0,
    );
    let user = Pubkey::new([1u8; 32]);
    let blockhash = Blockhash::new([2u8; 32]);

    let result = swap::build_swap_transaction(&quote, &user, blockhash);
    assert!(result.is_ok());
    let txn = result.unwrap();
    let b64 = txn.to_base64();
    assert!(!b64.is_empty(), "serialized tx must produce non-empty base64");
}

#[test]
fn test_build_swap_transaction_preserves_blockhash() {
    let quote = test_quote(
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        50,
        100.0,
    );
    let user = Pubkey::new([1u8; 32]);
    let blockhash = Blockhash::new([42u8; 32]);

    let result = swap::build_swap_transaction(&quote, &user, blockhash);
    assert!(result.is_ok());
    let txn = result.unwrap();
    assert_eq!(txn.message.recent_blockhash, blockhash);
}

// Total: 9 tests
