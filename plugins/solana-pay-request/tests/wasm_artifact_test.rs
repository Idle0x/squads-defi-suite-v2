//! WASM artifact verification for `solana-pay-request`.
use std::path::PathBuf;

const MIN_WASM_SIZE: u64 = 20_000;
const WASM_FILE: &str = "solana_pay_request.wasm";

fn find_wasm() -> Option<PathBuf> {
    let root = std::env::var("CARGO_MANIFEST_DIR").map(std::path::PathBuf::from).unwrap_or_default();
    let ws = root.ancestors().find(|p| p.join("Cargo.toml").exists() && p.join("target").exists()).unwrap_or(&root);
    let p = ws.join("target").join("wasm32-wasip2").join("release").join(WASM_FILE);
    if p.exists() { Some(p) } else { None }
}

#[test]
fn test_wasm_artifact_exists() {
    assert!(find_wasm().is_some(), "WASM '{}' not found. Run build.sh.", WASM_FILE);
}

#[test]
fn test_wasm_artifact_has_reasonable_size() {
    let size = std::fs::metadata(&find_wasm().expect("not found")).unwrap().len();
    assert!(size >= MIN_WASM_SIZE, "WASM too small: {} bytes", size);
}
