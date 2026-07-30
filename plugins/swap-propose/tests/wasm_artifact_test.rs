//! WASM artifact verification for `swap-propose`.
//!
//! Verifies the compiled `.wasm` file exists, has a reasonable size,
//! and passes `wasm-tools validate` for the component model.
//! This ensures the WIT contract exports are structurally valid.
//!
//! Requires `wasm-tools` CLI for full validation; see `build.sh` or CI.
//! The size and existence checks always run.
//!
//! NOTE: These tests silently skip when the WASM artifact hasn't been
//! built yet (e.g. during `cargo test --workspace` before build.sh).
//! Run `bash scripts/build.sh` first, or let CI handle the ordering.

use std::process::Command;

/// Expected minimum size for a meaningful swap-propose WASM component.
/// The actual artifact is ~640KB; this lower bound catches stub/empty outputs.
const MIN_WASM_SIZE: u64 = 50_000;
/// The WASM file name produced by the build (kebab → snake_case).
const WASM_FILE: &str = "swap_propose.wasm";

/// Locate the workspace root by climbing from CARGO_MANIFEST_DIR.
fn workspace_root() -> std::path::PathBuf {
    std::env::var("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap())
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("target").exists())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap())
}

/// Find the compiled WASM binary in the workspace target directory.
fn find_wasm() -> Option<std::path::PathBuf> {
    let target_dir = workspace_root().join("target").join("wasm32-wasip2").join("release");
    let wasm_path = target_dir.join(WASM_FILE);
    if wasm_path.exists() {
        Some(wasm_path)
    } else {
        // Try finding anywhere under target/
        find::find(&target_dir, WASM_FILE)
    }
}

/// Simple file-find helper (avoids regex/fnmatch deps).
mod find {
    use std::path::{Path, PathBuf};

    pub fn find(dir: &Path, name: &str) -> Option<PathBuf> {
        if dir.is_dir() {
            for entry in std::fs::read_dir(dir).ok()? {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.is_dir() {
                    if let Some(found) = find(&path, name) {
                        return Some(found);
                    }
                } else if path.file_name().and_then(|s| s.to_str()) == Some(name) {
                    return Some(path);
                }
            }
        }
        None
    }
}

#[test]
fn test_wasm_artifact_exists() {
    let wasm_path = find_wasm();
    assert!(
        wasm_path.is_some(),
        "WASM artifact '{}' not found. Run `cargo build --target wasm32-wasip2 --release -p swap-propose` first.",
        WASM_FILE
    );
}

#[test]
fn test_wasm_artifact_has_reasonable_size() {
    let wasm_path = find_wasm().expect("WASM artifact not found — run build.sh first");
    let metadata = std::fs::metadata(&wasm_path).expect("cannot read WASM file metadata");
    let size = metadata.len();
    assert!(
        size >= MIN_WASM_SIZE,
        "WASM artifact too small: {} bytes (min {}). Was it built correctly?",
        size,
        MIN_WASM_SIZE
    );
    assert!(
        size < 10_000_000,
        "WASM artifact suspiciously large: {} bytes",
        size
    );
}

#[test]
fn test_wasm_artifact_valid_component_model() {
    let wasm_path = find_wasm().expect("WASM artifact not found — run build.sh first");

    // Try using wasm-tools if installed
    let result = Command::new("wasm-tools")
        .args(["validate", "--features", "component-model"])
        .arg(&wasm_path)
        .output();

    match result {
        Ok(output) => {
            let status = output.status;
            assert!(
                status.success(),
                "wasm-tools validate failed for {}:\n{}\n{}",
                wasm_path.display(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }
        Err(e) => {
            // wasm-tools not installed — skip validation, only check size/existence
            eprintln!(
                "⚠️  wasm-tools not available ({}). Install with: cargo install wasm-tools",
                e
            );
        }
    }
}
