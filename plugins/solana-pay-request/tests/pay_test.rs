use solana_pay_request::pay;

const VALID_WALLET: &str = "GfQkesR7PGJP7etL6scmp8R1SaHLBcryCUaHehgLjmCW";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

#[test] fn test_build_pay_url_sol_transfer() {
    let url = pay::build_pay_url(VALID_WALLET, Some("1.5"), None, None, None, None).unwrap();
    assert!(url.starts_with("solana:"));
    assert!(url.contains("amount=1.5"));
}

#[test] fn test_build_pay_url_spl_transfer() {
    let url = pay::build_pay_url(VALID_WALLET, Some("100"), Some(USDC_MINT), None, None, None).unwrap();
    assert!(url.contains("spl-token=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"));
    assert!(url.contains("amount=100"));
}

#[test] fn test_build_pay_url_with_label_and_message() {
    let url = pay::build_pay_url(VALID_WALLET, Some("5"), None, Some("Coffee"), Some("Table 4"), None).unwrap();
    assert!(url.contains("label=Coffee"));
    assert!(url.contains("message=Table%204"));
}

#[test] fn test_build_pay_url_invalid_recipient() {
    let result = pay::build_pay_url("not-a-valid-address!!!", None, None, None, None, None);
    assert!(result.is_err());
}

#[test] fn test_pay_url_matches_spec_format() {
    let url = pay::build_pay_url(VALID_WALLET, None, None, None, None, None).unwrap();
    assert_eq!(url, format!("solana:{VALID_WALLET}"));
}

#[test] fn test_pay_url_with_all_params() {
    let url = pay::build_pay_url(VALID_WALLET, Some("10"), Some(USDC_MINT), Some("Dinner"), Some("Thanks!"), Some("inv-42")).unwrap();
    assert!(url.starts_with("solana:"));
    assert!(url.contains("amount=10"));
    assert!(url.contains("spl-token="));
    assert!(url.contains("label=Dinner"));
    assert!(url.contains("message=Thanks!"));
    assert!(url.contains("memo=inv-42"));
}
