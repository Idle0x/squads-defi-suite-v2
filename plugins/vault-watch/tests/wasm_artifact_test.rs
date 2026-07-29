//! WASM artifact verification for `vault-watch`.
use std::path::PathBuf;

const MIN_WASM_SIZE: u64 = 50_000;
const WASM_FILE: &str = "vault_watch.wasm";

fn find_wasm() -> Option<PathBuf> {
    let root = std::env::var("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap());
    let workspace_root = root
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("target").exists())
        .unwrap_or(&root);
    let wasm_path = workspace_root.join("target").join("wasm32-wasip2").join("release").join(WASM_FILE);
    if wasm_path.exists() { Some(wasm_path) } else { None }
}

#[test]
fn test_wasm_artifact_exists() {
    assert!(find_wasm().is_some(), "WASM artifact '{}' not found. Run build.sh first.", WASM_FILE);
}

#[test]
fn test_wasm_artifact_has_reasonable_size() {
    let wasm_path = find_wasm().expect("WASM artifact not found");
    let size = std::fs::metadata(&wasm_path).unwrap().len();
    assert!(size >= MIN_WASM_SIZE, "WASM too small: {} bytes", size);
}
