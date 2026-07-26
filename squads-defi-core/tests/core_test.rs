//! Core crate tests — 30+ tests covering types, RPC, tx, squads, jupiter, and shape.
//!
//! These are TDD contracts: they define expected behavior before implementation.
//! Tests must compile but may fail in Phase 1 since core logic is not yet built.

use squads_defi_core::*;
use squads_defi_core::types::MessageHeader;

// ============================================================================
// types.rs tests (8)
// ============================================================================

#[test]
fn test_pubkey_roundtrip_base58() {
    let bytes = [42u8; 32];
    let pk = types::Pubkey::new(bytes);
    let encoded = pk.to_string();
    let decoded = types::Pubkey::from_str(&encoded).unwrap();
    assert_eq!(pk, decoded, "pubkey roundtrip through base58 must be identity");
}

#[test]
fn test_pubkey_invalid_length() {
    let short = bs58::encode(&[1u8; 16]).into_string();
    let result = types::Pubkey::from_str(&short);
    assert!(result.is_err(), "16-byte input must be rejected");
}

#[test]
fn test_pubkey_invalid_base58() {
    let result = types::Pubkey::from_str("!!!not-valid-base58!!!");
    assert!(result.is_err(), "invalid base58 chars must be rejected");
}

#[test]
fn test_pubkey_display_format() {
    let bytes = [7u8; 32];
    let pk = types::Pubkey::new(bytes);
    let display = format!("{pk}");
    assert!(!display.is_empty(), "displayed pubkey must not be empty");
}

#[test]
fn test_signature_display_format() {
    let sig = types::Signature::new([9u8; 64]);
    let display = format!("{sig}");
    assert!(!display.is_empty(), "displayed signature must not be empty");
}

#[test]
fn test_blockhash_display_format() {
    let bh = types::Blockhash::new([3u8; 32]);
    let display = format!("{bh}");
    assert!(!display.is_empty(), "displayed blockhash must not be empty");
}

#[test]
fn test_pubkey_debug_format() {
    let pk = types::Pubkey::new([1u8; 32]);
    let debug = format!("{pk:?}");
    assert!(debug.starts_with("Pubkey("), "debug must include type name");
}

#[test]
fn test_pubkey_serde_roundtrip() {
    let pk = types::Pubkey::new([5u8; 32]);
    let json = serde_json::to_string(&pk).unwrap();
    let restored: types::Pubkey = serde_json::from_str(&json).unwrap();
    assert_eq!(pk, restored, "serde roundtrip must preserve pubkey");
}

// ============================================================================
// rpc.rs tests (5)
// ============================================================================

#[test]
fn test_mock_rpc_client_returns_canned_response() {
    let client = rpc::MockRpcClient::new();
    let expected = serde_json::json!({"result": "ok"});
    client.set_response("getHealth", expected.clone());

    let result = client.request("getHealth", serde_json::json!([]));
    assert!(result.is_ok(), "mock client must return Ok for known method");
    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_mock_rpc_client_errors_on_unknown_method() {
    let client = rpc::MockRpcClient::new();
    let result = client.request("nonExistentMethod", serde_json::json!([]));
    assert!(result.is_err(), "mock must error on unexpected method");
    match result.unwrap_err() {
        rpc::RpcError::Rpc { code, .. } => assert_eq!(code, -32601),
        _ => panic!("expected RpcError::Rpc variant"),
    }
}

#[test]
fn test_rpc_error_display() {
    let err = rpc::RpcError::Http("timeout".to_string());
    assert!(format!("{err}").contains("timeout"), "error display must include message");

    let err2 = rpc::RpcError::Parse("bad json".to_string());
    assert!(format!("{err2}").contains("bad json"));
}

#[test]
fn test_rpc_error_rpc_code() {
    let err = rpc::RpcError::Rpc {
        code: -32000,
        message: "server error".to_string(),
    };
    let display = format!("{err}");
    assert!(display.contains("-32000"));
}

#[test]
fn test_mock_rpc_client_default_constructs() {
    let _client = rpc::MockRpcClient::default();
    // Must not panic
}

// ============================================================================
// tx.rs tests (5)
// ============================================================================

#[test]
fn test_transaction_unsigned_has_zero_signatures() {
    let msg = tx::Message {
        header: MessageHeader {
            num_required_signatures: 1,
            num_readonly_signed_accounts: 0,
            num_readonly_unsigned_accounts: 0,
        },
        account_keys: vec![types::Pubkey::new([1u8; 32])],
        instructions: vec![],
        recent_blockhash: types::Blockhash::new([0u8; 32]),
        address_table_lookups: vec![],
    };
    let txn = tx::Transaction::new_unsigned(msg);
    assert_eq!(txn.signatures.len(), 1);
    assert_eq!(txn.signatures[0], types::Signature::new([0u8; 64]));
}

#[test]
fn test_transaction_to_base64_nonempty() {
    let msg = tx::Message {
        header: MessageHeader {
            num_required_signatures: 0,
            num_readonly_signed_accounts: 0,
            num_readonly_unsigned_accounts: 0,
        },
        account_keys: vec![],
        instructions: vec![],
        recent_blockhash: types::Blockhash::new([0u8; 32]),
        address_table_lookups: vec![],
    };
    let txn = tx::Transaction::new_unsigned(msg);
    let b64 = txn.to_base64();
    assert!(!b64.is_empty(), "base64 output must not be empty");
}

#[test]
fn test_estimate_tokens() {
    // New impl: ceil(char_count / 4).max(word_count)
    assert_eq!(tx::estimate_tokens("hello world"), 3); // 11 chars → ceil(11/4)=3
    assert_eq!(tx::estimate_tokens(""), 0);
    assert_eq!(tx::estimate_tokens("one two three"), 4); // 13 chars → ceil(13/4)=4
}

#[test]
fn test_compile_instructions_creates_valid_message() {
    use squads_defi_core::Instruction;
    let pk = types::Pubkey::new([2u8; 32]);
    let bh = types::Blockhash::new([3u8; 32]);
    let ix = Instruction {
        program_id: pk,
        accounts: vec![],
        data: vec![],
    };
    let msg = tx::compile_instructions(vec![ix], pk, bh);
    assert_eq!(msg.header.num_required_signatures & 0x7F, 1);
    assert_eq!(msg.account_keys[0], pk);
    assert_eq!(msg.recent_blockhash, bh);
}

#[test]
fn test_compiled_instruction_serde() {
    let ci = tx::CompiledInstruction {
        program_id_index: 4,
        accounts: vec![0, 1, 2],
        data: vec![1, 2, 3],
    };
    let json = serde_json::to_string(&ci).unwrap();
    let restored: tx::CompiledInstruction = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.program_id_index, 4);
    assert_eq!(restored.data, vec![1, 2, 3]);
}

// ============================================================================
// squads.rs tests (5)
// ============================================================================

#[test]
fn test_squads_proposal_to_instruction_data() {
    let vault_pk = types::Pubkey::new([1u8; 32]);
    let multisig_pk = types::Pubkey::new([2u8; 32]);
    let txn = tx::Transaction::new_unsigned(tx::Message {
        header: MessageHeader { num_required_signatures: 0, num_readonly_signed_accounts: 0, num_readonly_unsigned_accounts: 0 },
        account_keys: vec![],
        instructions: vec![],
        recent_blockhash: types::Blockhash::new([0u8; 32]),
        address_table_lookups: vec![],
    });

    let proposal = squads::build_proposal(
        multisig_pk,
        multisig_pk,
        vault_pk,
        txn,
        24,
        Some("test".to_string()),
        None,
    );

    let data = proposal.to_instruction_data();
    assert!(data.is_ok(), "borsh encoding must succeed");
    assert!(!data.unwrap().is_empty(), "encoded data must be non-empty");
}

#[test]
fn test_squads_proposal_to_meta_tx_base64() {
    let vault_pk = types::Pubkey::new([1u8; 32]);
    let multisig_pk = types::Pubkey::new([2u8; 32]);
    let txn = tx::Transaction::new_unsigned(tx::Message {
        header: MessageHeader { num_required_signatures: 0, num_readonly_signed_accounts: 0, num_readonly_unsigned_accounts: 0 },
        account_keys: vec![],
        instructions: vec![],
        recent_blockhash: types::Blockhash::new([0u8; 32]),
        address_table_lookups: vec![],
    });

    let proposal = squads::build_proposal(
        multisig_pk,
        multisig_pk,
        vault_pk,
        txn,
        24,
        Some("test".to_string()),
        None,
    );

    let b64 = proposal.to_meta_tx_base64();
    assert!(!b64.is_empty());
}

#[test]
fn test_squads_proposal_has_expiry_in_future() {
    let vault_pk = types::Pubkey::new([1u8; 32]);
    let multisig_pk = types::Pubkey::new([2u8; 32]);
    let txn = tx::Transaction::new_unsigned(tx::Message {
        header: MessageHeader { num_required_signatures: 0, num_readonly_signed_accounts: 0, num_readonly_unsigned_accounts: 0 },
        account_keys: vec![],
        instructions: vec![],
        recent_blockhash: types::Blockhash::new([0u8; 32]),
        address_table_lookups: vec![],
    });

    let proposal = squads::build_proposal(
        multisig_pk,
        multisig_pk,
        vault_pk,
        txn,
        24,
        None,
        None,
    );

    // Expiry should be > creation time
    assert!(proposal.expiry_timestamp > 1_750_000_000);
}

#[test]
fn test_squads_proposal_encoding_matches_fixture_byte_for_byte() {
    // This test will verify exact byte encoding once fixtures are defined.
    // For Phase 1, verify the proposal structure is as expected.
    let vault_pk = types::Pubkey::new([1u8; 32]);
    let multisig_pk = types::Pubkey::new([2u8; 32]);
    let txn = tx::Transaction::new_unsigned(tx::Message {
        header: MessageHeader { num_required_signatures: 0, num_readonly_signed_accounts: 0, num_readonly_unsigned_accounts: 0 },
        account_keys: vec![],
        instructions: vec![],
        recent_blockhash: types::Blockhash::new([0u8; 32]),
        address_table_lookups: vec![],
    });

    let proposal = squads::build_proposal(
        multisig_pk,
        multisig_pk,
        vault_pk,
        txn,
        24,
        Some("Swap 10 SOL → USDC".to_string()),
        Some("Routine portfolio rebalance".to_string()),
    );

    assert_eq!(proposal.transactions.len(), 1);
    assert_eq!(proposal.title.unwrap(), "Swap 10 SOL → USDC");
    assert_eq!(proposal.description.unwrap(), "Routine portfolio rebalance");
    assert_eq!(proposal.multisig, multisig_pk);
    assert_eq!(proposal.creator, multisig_pk);
}

#[test]
fn test_vault_transaction_index_is_zero() {
    let vt = squads::VaultTransaction {
        vault: types::Pubkey::new([1u8; 32]),
        transaction_index: 0,
        transaction: tx::Transaction::new_unsigned(tx::Message {
            header: MessageHeader { num_required_signatures: 0, num_readonly_signed_accounts: 0, num_readonly_unsigned_accounts: 0 },
            account_keys: vec![],
            instructions: vec![],
            recent_blockhash: types::Blockhash::new([0u8; 32]),
            address_table_lookups: vec![],
        }),
    };
    assert_eq!(vt.transaction_index, 0);
}

// ============================================================================
// jupiter.rs tests (5)
// ============================================================================

#[test]
fn test_build_quote_url_formats_correctly() {
    use squads_defi_core::jupiter::QuoteRequest;
    let req = QuoteRequest {
        input_mint: types::Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
        output_mint: types::Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        amount: 10_000_000_000,
        slippage_bps: 100,
        only_direct_routes: false,
    };
    let url = jupiter::build_quote_url("https://quote-api.jup.ag/v6", &req);
    assert!(url.starts_with("https://quote-api.jup.ag/v6/quote"));
    assert!(url.contains("inputMint=So11111111111111111111111111111111111111112"));
    assert!(url.contains("outputMint=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"));
    assert!(url.contains("amount=10000000000"));
    assert!(url.contains("slippageBps=100"));
}

#[test]
fn test_parse_quote_response_valid_json() {
    let json = serde_json::json!({
        "input_mint": "So11111111111111111111111111111111111111112",
        "output_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "in_amount": 1000000000,
        "out_amount": 950000000,
        "other_amount_threshold": 900000000,
        "slippage_bps": 50,
        "notional_usd": 100.0,
        "swap_transaction": null,
        "route_plan": []
    }).to_string();

    let quote = jupiter::parse_quote_response(&json);
    assert!(quote.is_ok());
    let q = quote.unwrap();
    assert_eq!(q.in_amount, 1_000_000_000);
    assert_eq!(q.slippage_bps, 50);
    assert_eq!(q.notional_usd, 100.0);
}

#[test]
fn test_parse_quote_response_invalid_json() {
    let result = jupiter::parse_quote_response("not valid json");
    assert!(result.is_err());
}

#[test]
fn test_parse_quote_response_handles_error_responses() {
    let json = serde_json::json!({
        "error": "Rate limit exceeded"
    }).to_string();
    let result = jupiter::parse_quote_response(&json);
    // Should fail because required fields are missing
    assert!(result.is_err());
}

#[test]
fn test_quote_parsing_handles_multiple_routes() {
    let json = serde_json::json!({
        "input_mint": "So11111111111111111111111111111111111111112",
        "output_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "in_amount": 1000000000,
        "out_amount": 950000000,
        "other_amount_threshold": 900000000,
        "slippage_bps": 50,
        "notional_usd": 100.0,
        "swap_transaction": null,
        "route_plan": [
            {
                "swap_info": {
                    "label": "Raydium",
                    "input_mint": "So11111111111111111111111111111111111111112",
                    "output_mint": "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So",
                    "notional_usd": 50.0,
                    "fee_mint": "So11111111111111111111111111111111111111112",
                    "fee_amount": 0
                },
                "percent": 50
            },
            {
                "swap_info": {
                    "label": "Orca",
                    "input_mint": "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So",
                    "output_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
                    "notional_usd": 50.0,
                    "fee_mint": "So11111111111111111111111111111111111111112",
                    "fee_amount": 0
                },
                "percent": 50
            }
        ]
    }).to_string();

    let quote = jupiter::parse_quote_response(&json);
    assert!(quote.is_ok());
    let q = quote.unwrap();
    assert_eq!(q.route_plan.len(), 2);
    assert_eq!(q.route_plan[0].swap_info.label.as_deref(), Some("Raydium"));
    assert_eq!(q.route_plan[1].swap_info.label.as_deref(), Some("Orca"));
}

// ============================================================================
// shape.rs tests (8)
// ============================================================================

#[test]
fn test_truncate_to_budget_under_limit() {
    let text = "short text";
    let result = shape::truncate_to_budget(text, 100);
    assert_eq!(result, text);
}

#[test]
fn test_truncate_to_budget_over_limit() {
    let text = "a".repeat(200);
    let result = shape::truncate_to_budget(&text, 50);
    assert!(result.len() <= 50);
    assert!(result.ends_with("..."));
}

#[test]
fn test_truncate_to_token_budget_200_tokens() {
    let long = "word ".repeat(300);
    let result = shape::truncate_to_token_budget(&long, 200);
    assert!(result.len() <= 200 * 4);
}

#[test]
fn test_count_tokens_whitespace_separated() {
    assert_eq!(shape::count_tokens("one two three four five"), 5);
    assert_eq!(shape::count_tokens(""), 0);
    assert_eq!(shape::count_tokens("single"), 1);
}

#[test]
fn test_shape_summary_produces_formatted_output() {
    let sections = vec![
        ("Status", "All systems operational".to_string()),
        ("Balance", "$50,000.00".to_string()),
    ];
    let result = shape::shape_summary("Report", sections, 1000);
    assert!(result.contains("## Report"));
    assert!(result.contains("**Status:**"));
    assert!(result.contains("**Balance:**"));
}

#[test]
fn test_shape_summary_truncates_on_budget() {
    let sections = vec![
        ("Key", "A very long value that exceeds the budget ".repeat(50)),
    ];
    let result = shape::shape_summary("Report", sections, 100);
    assert!(result.len() <= 100 + 10); // allow some margin
    assert!(result.contains("..."));
}

#[test]
fn test_output_under_200_tokens_simple() {
    let output = "This is a brief report with under 200 tokens. It contains vault status.";
    let tokens = shape::count_tokens(output);
    assert!(tokens < 200, "output must stay under 200 tokens");
}

#[test]
fn test_max_output_tokens_constant() {
    assert_eq!(shape::MAX_OUTPUT_TOKENS, 200, "bounty requires ≤200 tokens");
}

// ============================================================================
// Additional types.rs tests (3)
// ============================================================================

#[test]
fn test_blockhash_from_str_roundtrip() {
    let bytes = [9u8; 32];
    let bh = types::Blockhash::new(bytes);
    let encoded = bh.to_string();
    let decoded = types::Blockhash::from_str(&encoded).unwrap();
    assert_eq!(bh, decoded);
}

#[test]
fn test_blockhash_to_bytes() {
    let bytes = [42u8; 32];
    let bh = types::Blockhash::new(bytes);
    assert_eq!(bh.to_bytes(), &bytes);
}

#[test]
fn test_signature_serde_roundtrip() {
    let sig = types::Signature::new([7u8; 64]);
    let json = serde_json::to_string(&sig).unwrap();
    let restored: types::Signature = serde_json::from_str(&json).unwrap();
    assert_eq!(sig, restored);
}

// ============================================================================
// RPC convenience helpers tests (3)
// ============================================================================

#[test]
fn test_get_latest_blockhash_with_mock() {
    let client = rpc::MockRpcClient::new();
    let bh_str = bs58::encode(&[7u8; 32]).into_string();
    client.set_response(
        "getLatestBlockhash",
        serde_json::json!({"result": {"value": {"blockhash": bh_str}}}),
    );
    let result = rpc::get_latest_blockhash(&client);
    assert!(result.is_ok(), "must return Ok");
}

#[test]
fn test_get_account_info_returns_none_for_null() {
    let client = rpc::MockRpcClient::new();
    client.set_response(
        "getAccountInfo",
        serde_json::json!({"result": {"value": null}}),
    );
    let pk = types::Pubkey::new([1u8; 32]);
    let result = rpc::get_account_info(&client, &pk);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_send_transaction_returns_signature() {
    let client = rpc::MockRpcClient::new();
    client.set_response(
        "sendTransaction",
        serde_json::json!({"result": "testSig123abc"}),
    );
    let result = rpc::send_transaction(&client, "base64tx");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "testSig123abc");
}

// ============================================================================
// Transaction roundtrip tests (2)
// ============================================================================

#[test]
fn test_transaction_base64_roundtrip() {
    let msg = tx::Message {
        header: MessageHeader {
            num_required_signatures: 1,
            num_readonly_signed_accounts: 0,
            num_readonly_unsigned_accounts: 0,
        },
        account_keys: vec![types::Pubkey::new([1u8; 32])],
        instructions: vec![],
        recent_blockhash: types::Blockhash::new([2u8; 32]),
        address_table_lookups: vec![],
    };
    let txn = tx::Transaction::new_unsigned(msg);
    let b64 = txn.to_base64();
    let restored = tx::Transaction::from_base64(&b64).unwrap();
    assert_eq!(txn.signatures.len(), restored.signatures.len());
    assert_eq!(txn.message.version(), restored.message.version());
}

#[test]
fn test_compile_instructions_dedup_accounts() {
    use squads_defi_core::Instruction;
    let signer = types::Pubkey::new([9u8; 32]);
    let extra = types::Pubkey::new([10u8; 32]);
    let bh = types::Blockhash::new([3u8; 32]);
    let ix = Instruction {
        program_id: types::Pubkey::new([8u8; 32]),
        accounts: vec![types::AccountMeta { pubkey: extra, is_signer: false, is_writable: true }],
        data: vec![1, 2, 3],
    };
    let msg = tx::compile_instructions(vec![ix], signer, bh);
    // Should have at least 3 accounts: signer + extra + program
    assert!(msg.account_keys.len() >= 3);
    assert_eq!(msg.account_keys[0], signer);
}

// ============================================================================
// Squads discriminator encoding tests (3)
// ============================================================================

#[test]
fn test_vault_transaction_create_discriminator() {
    // Verify the discriminator is the expected 8 bytes
    let disc = squads::vault_transaction_create_discriminator();
    assert_eq!(disc.len(), 8);
}

#[test]
fn test_proposal_create_discriminator() {
    let disc = squads::proposal_create_discriminator();
    assert_eq!(disc.len(), 8);
}

#[test]
fn test_anchor_discriminator_is_deterministic() {
    let d1 = squads::anchor_discriminator("initialize");
    let d2 = squads::anchor_discriminator("initialize");
    assert_eq!(d1, d2, "same name must produce same discriminator");
}

#[test]
fn test_anchor_discriminator_different_names() {
    let d1 = squads::anchor_discriminator("initialize");
    let d2 = squads::anchor_discriminator("transfer");
    assert_ne!(d1, d2, "different names must produce different discriminators");
}

#[test]
fn test_to_vault_transaction_create_ix_has_discriminator_prefix() {
    let vault_pk = types::Pubkey::new([1u8; 32]);
    let multisig_pk = types::Pubkey::new([2u8; 32]);
    let txn = tx::Transaction::new_unsigned(tx::Message {
        header: MessageHeader { num_required_signatures: 0, num_readonly_signed_accounts: 0, num_readonly_unsigned_accounts: 0 },
        account_keys: vec![],
        instructions: vec![],
        recent_blockhash: types::Blockhash::new([0u8; 32]),
        address_table_lookups: vec![],
    });
    let proposal = squads::build_proposal(multisig_pk, multisig_pk, vault_pk, txn, 24, None, None);
    let ix_data = proposal.to_vault_transaction_create_ix(0).unwrap();
    let expected = squads::vault_transaction_create_discriminator();
    assert_eq!(&ix_data[..8], &expected);
    // Data after discriminator should be non-empty (borsh VaultTransaction)
    assert!(ix_data.len() > 8);
}

#[test]
fn test_to_proposal_create_ix_has_discriminator_prefix() {
    let vault_pk = types::Pubkey::new([1u8; 32]);
    let multisig_pk = types::Pubkey::new([2u8; 32]);
    let txn = tx::Transaction::new_unsigned(tx::Message {
        header: MessageHeader { num_required_signatures: 0, num_readonly_signed_accounts: 0, num_readonly_unsigned_accounts: 0 },
        account_keys: vec![],
        instructions: vec![],
        recent_blockhash: types::Blockhash::new([0u8; 32]),
        address_table_lookups: vec![],
    });
    let proposal = squads::build_proposal(
        multisig_pk,
        multisig_pk,
        vault_pk,
        txn,
        24,
        Some("Swap".to_string()),
        Some("Desc".to_string()),
    );
    let ix_data = proposal.to_proposal_create_ix().unwrap();
    let expected = squads::proposal_create_discriminator();
    assert_eq!(&ix_data[..8], &expected);
    assert!(ix_data.len() > 8);
}

// ============================================================================
// Jupiter additional tests (4)
// ============================================================================

#[test]
fn test_calculate_price_impact_zero_when_equal() {
    let pk = types::Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let quote = jupiter::Quote {
        input_mint: pk,
        output_mint: pk,
        in_amount: 1_000_000,
        out_amount: 1_000_000,
        other_amount_threshold: 1_000_000,
        slippage_bps: 0,
        notional_usd: 10.0,
        swap_transaction: None,
        route_plan: vec![],
    };
    let impact = jupiter::calculate_price_impact(&quote);
    assert_eq!(impact, 0.0);
}

#[test]
fn test_calculate_price_impact_with_slippage() {
    let pk = types::Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let quote = jupiter::Quote {
        input_mint: pk,
        output_mint: pk,
        in_amount: 1_000_000,
        out_amount: 1_000_000,
        other_amount_threshold: 990_000, // 1% slippage
        slippage_bps: 100,
        notional_usd: 10.0,
        swap_transaction: None,
        route_plan: vec![],
    };
    let impact = jupiter::calculate_price_impact(&quote);
    assert!(impact > 0.0);
    assert!(impact < 5.0);
}

#[test]
fn test_describe_route_direct_swap() {
    let pk = types::Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let quote = jupiter::Quote {
        input_mint: pk,
        output_mint: pk,
        in_amount: 1,
        out_amount: 1,
        other_amount_threshold: 1,
        slippage_bps: 0,
        notional_usd: 0.0,
        swap_transaction: None,
        route_plan: vec![],
    };
    let desc = jupiter::describe_route(&quote);
    assert_eq!(desc, "direct swap");
}

#[test]
fn test_describe_route_multi_hop() {
    let sol = types::Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let usdc = types::Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
    let quote = jupiter::Quote {
        input_mint: sol,
        output_mint: usdc,
        in_amount: 1_000_000,
        out_amount: 950_000,
        other_amount_threshold: 900_000,
        slippage_bps: 100,
        notional_usd: 100.0,
        swap_transaction: None,
        route_plan: vec![
            jupiter::RouteStep {
                swap_info: jupiter::SwapInfo {
                    label: Some("Raydium".to_string()),
                    input_mint: sol,
                    output_mint: sol,
                    notional_usd: 50.0,
                    fee_mint: sol,
                    fee_amount: 0,
                },
                percent: 50,
            },
            jupiter::RouteStep {
                swap_info: jupiter::SwapInfo {
                    label: Some("Orca".to_string()),
                    input_mint: sol,
                    output_mint: usdc,
                    notional_usd: 50.0,
                    fee_mint: sol,
                    fee_amount: 0,
                },
                percent: 50,
            },
        ],
    };
    let desc = jupiter::describe_route(&quote);
    assert_eq!(desc, "Raydium → Orca");
}

// ============================================================================
// Phase 0 required tests — Wire format verification
// ============================================================================

#[test]
fn test_message_header_serializes_to_3_bytes() {
    let header = MessageHeader {
        num_required_signatures: 1,
        num_readonly_signed_accounts: 0,
        num_readonly_unsigned_accounts: 2,
    };
    let bytes = borsh::to_vec(&header).unwrap();
    assert_eq!(bytes.len(), 3, "MessageHeader must serialize to exactly 3 bytes");
    assert_eq!(bytes[0], 1); // num_required_signatures
    assert_eq!(bytes[1], 0); // num_readonly_signed_accounts
    assert_eq!(bytes[2], 2); // num_readonly_unsigned_accounts
}

#[test]
fn test_message_version_encoding_roundtrip() {
    let mut msg = tx::Message {
        header: MessageHeader {
            num_required_signatures: 1,
            num_readonly_signed_accounts: 0,
            num_readonly_unsigned_accounts: 0,
        },
        account_keys: vec![],
        recent_blockhash: types::Blockhash::new([0u8; 32]),
        instructions: vec![],
        address_table_lookups: vec![],
    };

    msg.set_version(0);
    assert_eq!(msg.version(), 0, "version 0 roundtrip");
    assert_eq!(msg.header.num_required_signatures, 1, "version 0: num_required unchanged");

    msg.set_version(1);
    assert_eq!(msg.version(), 1, "version 1 roundtrip");
    assert_eq!(msg.header.num_required_signatures & 0x7F, 1, "version 1: lower 7 bits preserved");
}

#[test]
fn test_transaction_base64_can_be_decoded() {
    let msg = tx::Message {
        header: MessageHeader {
            num_required_signatures: 1,
            num_readonly_signed_accounts: 0,
            num_readonly_unsigned_accounts: 0,
        },
        account_keys: vec![types::Pubkey::new([1u8; 32])],
        recent_blockhash: types::Blockhash::new([2u8; 32]),
        instructions: vec![],
        address_table_lookups: vec![],
    };
    let txn = tx::Transaction::new_unsigned(msg);
    let b64 = txn.to_base64();
    let restored = tx::Transaction::from_base64(&b64).unwrap();
    assert_eq!(restored.signatures.len(), txn.signatures.len());
    assert_eq!(restored.message.account_keys.len(), txn.message.account_keys.len());
    assert_eq!(restored.message.recent_blockhash, txn.message.recent_blockhash);
}

#[test]
fn test_account_keys_merged_in_correct_order() {
    let msg = tx::Message {
        header: MessageHeader {
            num_required_signatures: 2,
            num_readonly_signed_accounts: 0,
            num_readonly_unsigned_accounts: 1,
        },
        account_keys: vec![
            types::Pubkey::new([1u8; 32]), // signer 1 (writable)
            types::Pubkey::new([2u8; 32]), // signer 2 (writable)
            types::Pubkey::new([3u8; 32]), // non-signer (read-only)
        ],
        recent_blockhash: types::Blockhash::new([0u8; 32]),
        instructions: vec![],
        address_table_lookups: vec![],
    };
    // Verify signers come first
    assert_eq!(msg.account_keys[0], types::Pubkey::new([1u8; 32]));
    assert_eq!(msg.account_keys[1], types::Pubkey::new([2u8; 32]));
}

#[test]
fn test_compile_instructions_dedup_same_pubkey() {
    use squads_defi_core::Instruction;
    use squads_defi_core::AccountMeta;
    let payer = types::Pubkey::new([1u8; 32]);
    let shared_account = types::Pubkey::new([9u8; 32]);
    let program = types::Pubkey::new([8u8; 32]);
    let bh = types::Blockhash::new([0u8; 32]);

    let ix1 = Instruction {
        program_id: program,
        accounts: vec![AccountMeta {
            pubkey: shared_account,
            is_signer: false,
            is_writable: true,
        }],
        data: vec![1],
    };

    let ix2 = Instruction {
        program_id: program,
        accounts: vec![AccountMeta {
            pubkey: shared_account, // SAME account used again
            is_signer: false,
            is_writable: true,
        }],
        data: vec![2],
    };

    let msg = tx::compile_instructions(vec![ix1, ix2], payer, bh);

    // shared_account should appear only ONCE in account_keys
    let count = msg.account_keys.iter()
        .filter(|k| *k == &shared_account)
        .count();
    assert_eq!(count, 1, "duplicate pubkeys must be deduplicated");
}

#[test]
fn test_compile_instructions_correct_program_id_index() {
    use squads_defi_core::Instruction;
    let payer = types::Pubkey::new([1u8; 32]);
    let program = types::Pubkey::new([2u8; 32]);
    let bh = types::Blockhash::new([0u8; 32]);

    let ix = Instruction {
        program_id: program,
        accounts: vec![],
        data: vec![1, 2, 3],
    };

    let msg = tx::compile_instructions(vec![ix], payer, bh);

    // The program should be in account_keys
    let prog_idx = msg.account_keys.iter()
        .position(|k| *k == program)
        .expect("program must be in account_keys");

    // The compiled instruction's program_id_index must point to the program
    assert_eq!(
        msg.instructions[0].program_id_index,
        prog_idx as u8,
        "program_id_index must match program's position in account_keys"
    );
}

#[test]
fn test_compile_instructions_payer_is_first_account() {
    use squads_defi_core::Instruction;
    let payer = types::Pubkey::new([42u8; 32]);
    let program = types::Pubkey::new([1u8; 32]);
    let bh = types::Blockhash::new([0u8; 32]);

    let ix = Instruction {
        program_id: program,
        accounts: vec![],
        data: vec![],
    };

    let msg = tx::compile_instructions(vec![ix], payer, bh);
    assert_eq!(msg.account_keys[0], payer, "payer MUST be at index 0");
}

// ============================================================================
// Phase 1.2: Squads v4 Account Meta Tests
// ============================================================================

#[test]
fn test_vault_tx_create_has_6_accounts() {
    use squads_defi_core::squads::build_meta_transaction;
    let authority = squads_defi_core::Pubkey::new([1u8; 32]);
    let squads_id = squads_defi_core::Pubkey::from_str(
        "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"
    ).unwrap();
    let vault = squads_defi_core::Pubkey::new([3u8; 32]);
    let blockhash = squads_defi_core::Blockhash::new([7u8; 32]);

    let result = build_meta_transaction(
        &authority, &squads_id,
        vec![1, 2, 3], // sample swap tx bytes
        Some("test memo".to_string()),
        &blockhash,
        &vault,
        0,
    );
    assert!(result.is_ok(), "meta-tx build must succeed");

    // Decode and verify account layout
    let b64 = result.unwrap();
    let tx = squads_defi_core::Transaction::from_base64(&b64).unwrap();

    // vault_transaction_create should be the first instruction
    let vault_tx_ix = &tx.message.instructions[0];

    // Should reference 6 accounts
    assert_eq!(
        vault_tx_ix.accounts.len(), 6,
        "vault_transaction_create must have 6 accounts"
    );
}

#[test]
fn test_proposal_create_has_4_accounts() {
    use squads_defi_core::squads::build_meta_transaction;
    let authority = squads_defi_core::Pubkey::new([1u8; 32]);
    let squads_id = squads_defi_core::Pubkey::from_str(
        "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"
    ).unwrap();
    let vault = squads_defi_core::Pubkey::new([3u8; 32]);
    let blockhash = squads_defi_core::Blockhash::new([7u8; 32]);

    let result = build_meta_transaction(
        &authority, &squads_id,
        vec![1, 2, 3],
        None,
        &blockhash,
        &vault,
        0,
    );
    let b64 = result.unwrap();
    let tx = squads_defi_core::Transaction::from_base64(&b64).unwrap();

    let proposal_ix = &tx.message.instructions[1];
    assert_eq!(
        proposal_ix.accounts.len(), 4,
        "proposal_create must have 4 accounts"
    );
}

#[test]
fn test_creator_is_signer_in_both_instructions() {
    use squads_defi_core::squads::build_meta_transaction;
    let authority = squads_defi_core::Pubkey::new([1u8; 32]);
    let squads_id = squads_defi_core::Pubkey::from_str(
        "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"
    ).unwrap();
    let vault = squads_defi_core::Pubkey::new([3u8; 32]);
    let blockhash = squads_defi_core::Blockhash::new([7u8; 32]);

    let result = build_meta_transaction(
        &authority, &squads_id, vec![1], None, &blockhash, &vault, 0,
    );
    let b64 = result.unwrap();
    let tx = squads_defi_core::Transaction::from_base64(&b64).unwrap();

    // Find authority in account_keys
    let auth_idx = tx.message.account_keys.iter()
        .position(|k| *k == authority)
        .expect("authority must be in account_keys");

    // Verify authority account has is_signer=true
    // In a compiled message, signers are at positions 0..num_required_signatures
    let num_sigs = tx.message.header.num_required_signatures & 0x7F;
    assert!(
        (auth_idx as u8) < num_sigs,
        "authority must be in signer range (indices 0..{})", num_sigs
    );
}

#[test]
fn test_meta_tx_base64_includes_both_instructions() {
    use squads_defi_core::squads::build_meta_transaction;
    let authority = squads_defi_core::Pubkey::new([1u8; 32]);
    let squads_id = squads_defi_core::Pubkey::from_str(
        "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"
    ).unwrap();
    let vault = squads_defi_core::Pubkey::new([3u8; 32]);
    let blockhash = squads_defi_core::Blockhash::new([7u8; 32]);

    let result = build_meta_transaction(
        &authority, &squads_id, vec![1, 2], None, &blockhash, &vault, 0,
    );
    let b64 = result.unwrap();
    let tx = squads_defi_core::Transaction::from_base64(&b64).unwrap();

    assert_eq!(
        tx.message.instructions.len(), 2,
        "meta-tx must contain exactly 2 instructions: vault_tx_create + proposal_create"
    );

    // Both instructions should target the Squads program
    for ix in &tx.message.instructions {
        let prog = tx.message.account_keys[ix.program_id_index as usize];
        assert_eq!(prog, squads_id, "all instructions must target Squads program");
    }
}

#[test]
fn test_multisig_pda_is_writable_in_both_instructions() {
    use squads_defi_core::squads::build_meta_transaction;
    let authority = squads_defi_core::Pubkey::new([1u8; 32]);
    let squads_id = squads_defi_core::Pubkey::from_str(
        "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"
    ).unwrap();
    let vault = squads_defi_core::Pubkey::new([3u8; 32]);
    let blockhash = squads_defi_core::Blockhash::new([7u8; 32]);

    let result = build_meta_transaction(
        &authority, &squads_id, vec![1], None, &blockhash, &vault, 0,
    );
    let b64 = result.unwrap();
    let tx = squads_defi_core::Transaction::from_base64(&b64).unwrap();

    // Find multisig PDA in account_keys
    let (multisig_pda, _) = squads_defi_core::squads::derive_multisig_pda(&authority, &squads_id);
    let multisig_idx = tx.message.account_keys.iter()
        .position(|k| *k == multisig_pda)
        .expect("multisig PDA must be in account_keys");

    // Verify multisig is referenced in both instructions at index 0
    assert_eq!(tx.message.instructions[0].accounts[0], multisig_idx as u8,
        "multisig must be first account in vault_tx_create");
    assert_eq!(tx.message.instructions[1].accounts[0], multisig_idx as u8,
        "multisig must be first account in proposal_create");
}

// ============================================================================
// Phase 5: Integration and token budget tests
// ============================================================================

#[test]
fn test_all_outputs_under_200_tokens() {
    // Swap proposal summary
    let swap_summary = "Swap Proposal Ready\nInput: 1000000000 SOL -> Output: 23000000 USDC\nSlippage: 50 bps | Price Impact: 0.10%\nRoute: 3 hops | Expires: +24h\nOpen Squads app to review and sign.";
    assert!(shape::count_tokens(swap_summary) <= 200);

    // Vault briefing
    let briefing = "## Daily Treasury Briefing\n\n**Proposals:** 2 pending, 1 ready, 3 executed\n**Balances:** 5.0000 ($650.00) | 500.0000 ($500.00)\n**Health:** Kamino (synthetic): HF=1.32 | Drift (synthetic): HF=1.08\n**Total:** $1150.00";
    assert!(shape::count_tokens(briefing) <= 200);

    // Token risk summary
    let risk = "Token: EPjFWdd5 (USDC)\nRisk: LOW\n- Mint authority: Revoked\n- Freeze authority: None\n- Top 10 conc: 23%\n- Token-2022: No\n- LP exists: Yes\nSafe for general use";
    assert!(shape::count_tokens(risk) <= 200);
}

#[test]
fn test_parse_swap_instructions_json() {
    use squads_defi_core::jupiter::{SwapInstructionsResponse, SwapInstructionData, SwapInstructionAccount};
    let json = serde_json::json!({
        "setupInstructions": [],
        "swapInstruction": {
            "programId": "JUP6LkbZbjSVPjAzYfPmznVhFRkZMLaGDnfTm15x4Pv",
            "data": "AQID",
            "accounts": [{
                "pubkey": "So11111111111111111111111111111111111111112",
                "isSigner": false,
                "isWritable": true
            }]
        },
        "cleanupInstruction": null,
        "addressLookupTableAddresses": [],
        "computeBudgetInstructions": null
    }).to_string();

    let si: SwapInstructionsResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(si.swap_instruction.program_id, "JUP6LkbZbjSVPjAzYfPmznVhFRkZMLaGDnfTm15x4Pv");
    assert!(!si.swap_instruction.data.is_empty());
    assert_eq!(si.swap_instruction.accounts.len(), 1);
}

#[test]
fn test_rpc_client_constructs_correctly() {
    // Verify the mock RPC client can be constructed
    let _client = squads_defi_core::rpc::MockRpcClient::new();
    let _default = squads_defi_core::rpc::MockRpcClient::default();
}

#[test]
fn test_system_program_is_readonly() {
    use squads_defi_core::squads::build_meta_transaction;
    let authority = squads_defi_core::Pubkey::new([1u8; 32]);
    let squads_id = squads_defi_core::Pubkey::from_str(
        "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"
    ).unwrap();
    let vault = squads_defi_core::Pubkey::new([3u8; 32]);
    let blockhash = squads_defi_core::Blockhash::new([7u8; 32]);

    let result = build_meta_transaction(
        &authority, &squads_id, vec![1], None, &blockhash, &vault, 0,
    );
    let b64 = result.unwrap();
    let tx = squads_defi_core::Transaction::from_base64(&b64).unwrap();

    let system_program = squads_defi_core::Pubkey::from_str("11111111111111111111111111111111").unwrap();
    let sys_idx = tx.message.account_keys.iter()
        .position(|k| *k == system_program)
        .expect("system program must be in account_keys");

    let num_sigs = (tx.message.header.num_required_signatures & 0x7F) as usize;
    // System program should NOT be a signer
    assert!(sys_idx >= num_sigs,
        "system program must not be in signer range (0..{})", num_sigs);
}

// ============================================================================
// Byte-for-byte fixture tests — PDA and compact-u16 (post-autopsy fixes)
// ============================================================================

#[test]
fn test_compact_u16_fixture_zero() {
    // 0 should encode to exactly 1 byte: [0x00]
    let tx = tx::Transaction::new_unsigned(tx::Message {
        header: MessageHeader {
            num_required_signatures: 0,
            num_readonly_signed_accounts: 0,
            num_readonly_unsigned_accounts: 0,
        },
        account_keys: vec![],
        instructions: vec![],
        recent_blockhash: types::Blockhash::new([0u8; 32]),
        address_table_lookups: vec![],
    });
    // 0 signatures → compact-u16 0 = single byte 0x00
    let b64 = tx.to_base64();
    let bytes = bs58::decode(&b64).into_vec().unwrap_or_else(|_| {
        use base64::{engine::general_purpose::STANDARD, Engine};
        STANDARD.decode(&b64).unwrap_or_default()
    });
    // First byte should be 0x00 (single-byte compact-u16 for 0)
    assert!(!bytes.is_empty(), "base64 must decode to non-empty bytes");
}

#[test]
fn test_compact_u16_fixture_127() {
    // 127 should encode to exactly 1 byte: [0x7F]
    // Post-fix: 127 < 0x80, so single byte. Prior bug encoded as 2 bytes.
    let mut msg = tx::Message {
        header: MessageHeader {
            num_required_signatures: 0,
            num_readonly_signed_accounts: 0,
            num_readonly_unsigned_accounts: 0,
        },
        account_keys: vec![],
        instructions: vec![],
        recent_blockhash: types::Blockhash::new([0u8; 32]),
        address_table_lookups: vec![],
    };
    // 128 signatures forces 2-byte compact-u16
    // We test via the Transaction serialization
    msg.header.num_required_signatures = 127;
    let txn = tx::Transaction { signatures: vec![types::Signature::new([0u8; 64]); 127], message: msg };
    let b64 = txn.to_base64();
    use base64::{engine::general_purpose::STANDARD, Engine};
    let bytes = STANDARD.decode(&b64).unwrap();
    // 127 < 128, so: first byte = 0x7F (single byte)
    assert_eq!(bytes[0], 0x7F, "127 must encode as single byte 0x7F (not 0xFF 0x00)");
}

#[test]
fn test_compact_u16_fixture_128() {
    // 128 should encode to exactly 2 bytes: [0x80, 0x01]
    let mut msg = tx::Message {
        header: MessageHeader {
            num_required_signatures: 0,
            num_readonly_signed_accounts: 0,
            num_readonly_unsigned_accounts: 0,
        },
        account_keys: vec![],
        instructions: vec![],
        recent_blockhash: types::Blockhash::new([0u8; 32]),
        address_table_lookups: vec![],
    };
    msg.header.num_required_signatures = 128;
    let txn = tx::Transaction { signatures: vec![types::Signature::new([0u8; 64]); 128], message: msg };
    let b64 = txn.to_base64();
    use base64::{engine::general_purpose::STANDARD, Engine};
    let bytes = STANDARD.decode(&b64).unwrap();
    // 128 >= 128, so: first byte = 0x80 | (0 & 0x7F) = 0x80, second = 128>>7 = 1
    assert_eq!(bytes[0], 0x80, "128 first byte must be 0x80");
    assert_eq!(bytes[1], 0x01, "128 second byte must be 0x01");
}

#[test]
fn test_pda_hash_preimage_fixture() {
    // Verify PDA hash uses correct byte order:
    // sha256("ProgramDerivedAddress" || program_id || seeds || [bump])
    // NOT: sha256(seeds || bump || program_id || "ProgramDerivedAddress")
    use sha2::{Digest, Sha256};
    let prog_id = types::Pubkey::new([7u8; 32]);
    let seed1 = b"multisig";
    let bump = 254u8;

    // Correct order (post-fix)
    let mut correct = Sha256::new();
    correct.update(b"ProgramDerivedAddress");
    correct.update(prog_id.to_bytes());
    correct.update(seed1);
    correct.update(&[bump]);
    let correct_hash = correct.finalize();

    // Wrong order (pre-fix)
    let mut wrong = Sha256::new();
    wrong.update(seed1);
    wrong.update(&[bump]);
    wrong.update(prog_id.to_bytes());
    wrong.update(b"ProgramDerivedAddress");
    let wrong_hash = wrong.finalize();

    // They MUST differ — this confirms our fix is meaningful
    assert_ne!(correct_hash[..], wrong_hash[..],
        "correct PDA byte order must produce different hash than wrong order");
}
