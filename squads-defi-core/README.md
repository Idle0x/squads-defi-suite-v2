# squads-defi-core

**Published crate:** [crates.io/crates/squads-defi-core](https://crates.io/crates/squads-defi-core)
**Documentation:** [docs.rs/squads-defi-core](https://docs.rs/squads-defi-core)

Hand-rolled Solana types and helpers for WASM plugin development. No `solana-sdk` or `solana-client` dependency — builds cleanly for `wasm32-wasip2`.

ZeroClaw WASM plugins cannot use `solana-sdk` because it depends on native host features (syscalls, memory-mapped files, threading primitives) that the WASM sandbox does not provide. This crate reimplements the minimal subset needed for transaction construction, Jupiter integration, and Squads proposal encoding — all in pure Rust, all WASM-compatible.

- [Modules](#modules)
- [Features](#features)
- [Usage](#usage)
- [Versioning](#versioning)
- [Source](#source)

---

## Modules

| Module | Exports | Description |
|--------|---------|-------------|
| [`types`](src/types.rs) | `Pubkey`, `Blockhash`, `Signature`, `Instruction`, `AccountMeta`, `MessageHeader`, `MessageAddressTableLookup` | Solana wire-format types. All hand-rolled, no SDK dependency. |
| [`tx`](src/tx.rs) | `Transaction`, `Message`, `compile_instructions()` | Versioned transaction construction from instruction vectors. `compile_instructions()` is the primary entry point. |
| [`squads`](src/squads.rs) | `derive_proposal_pda()`, `build_meta_transaction()`, `proposal_expiry_timestamp()`, `unix_now_seconds()` | Squads v4 proposal PDA derivation and meta-transaction building. |
| [`jupiter`](src/jupiter.rs) | `JupiterClient`, `Quote`, `QuoteResponse`, `SwapInstructionsResponse`, `SwapInstructionData`, `validate_quote()`, `describe_route()` | Jupiter quote validation with configurable guardrails (slippage, notional, daily cap). The `validate_quote()` method implements all six guardrail checks. |
| [`rpc`](src/rpc.rs) | `RpcClient`, `MockRpcClient`, `RpcError`, `get_account_info()`, `get_latest_blockhash()`, `send_transaction()` | WASM-compatible JSON-RPC abstraction over `wasi:http`. `MockRpcClient` is used in host-run tests. |
| [`shape`](src/shape.rs) | `MAX_OUTPUT_TOKENS`, `truncate_to_token_budget()` | Response shaping utilities to keep tool output within the `MAX_OUTPUT_TOKENS` limit. |
| [`ed25519`](src/ed25519.rs) | `verify_signature()`, `derive_pubkey()` | Ed25519 signature verification and pubkey derivation (no `ed25519-dalek` dependency). |
| [`test_utils`](src/test_utils.rs) | Test fixtures, mock quote/swap responses | Shared test infrastructure for all plugins. |

---

## Features

| Feature | What it enables | Used by |
|---------|----------------|---------|
| `squads-state` | Imports `squads-multisig-program` for Squads-specific types (proposal structs, vault PDAs) | `jupiter-swap-propose`, `vault-watch` |
| `waki` | WASI HTTP client for `wasm32-wasip2` builds | Plugins that call RPC endpoints |

Plugins select features based on what they need:

```toml
# For plugins that need Squads types:
squads-defi-core = { version = "0.1", features = ["squads-state"] }

# For plugins that only need basic types (Pubkey, Blockhash, etc.):
squads-defi-core = "0.1"
```

---

## Usage

### From a ZeroClaw plugin

```toml
[dependencies]
squads-defi-core = "0.1"
```

```rust
use squads_defi_core::{Pubkey, Blockhash, Transaction};
use squads_defi_core::jupiter::{Quote, JupiterClient};
use squads_defi_core::squads::derive_proposal_pda;
```

### From host tests (no wasm)

```bash
cargo test -p squads-defi-core
```

The core library has zero WASM-only dependencies in its default build. Optional features (`waki`) are gated behind `cfg(target_family = "wasm")`.

---

## Versioning

This crate follows [SemVer](https://semver.org/). The current version is `0.1.1`. Breaking changes to the public API will be reflected in a minor version bump.

All four Squads DeFi Suite plugins depend on a single version of this crate. When the core API changes, all plugins are updated together in a coordinated release.

---

## Source

| File | Description |
|------|-------------|
| [`src/types.rs`](src/types.rs) | Solana wire-format types |
| [`src/tx.rs`](src/tx.rs) | Transaction construction |
| [`src/squads.rs`](src/squads.rs) | Squads v4 integration |
| [`src/jupiter.rs`](src/jupiter.rs) | Jupiter quote validation |
| [`src/rpc.rs`](src/rpc.rs) | RPC abstraction |
| [`src/shape.rs`](src/shape.rs) | Output shaping |
| [`src/ed25519.rs`](src/ed25519.rs) | Ed25519 primitives |
| [`src/test_utils.rs`](src/test_utils.rs) | Test fixtures |

---

## See also

- [ARCHITECTURE.md](../ARCHITECTURE.md#dependency-model) — Why crates.io instead of path deps
- [README.md](../README.md) — Plugin suite overview
- [ZeroClaw WIT contract](../wit/v0/) — The ABI plugins build against
