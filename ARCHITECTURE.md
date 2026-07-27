# Architecture

This document describes how the Squads DeFi Suite plugins work under the hood. It is a reference for plugin developers, security reviewers, and anyone extending the suite.

- [Plugin anatomy](#plugin-anatomy)
- [The WIT contract](#the-wit-contract)
- [The `__config` jail](#the-__config-jail)
- [Permissions model](#permissions-model)
- [Capsule tiers](#capsule-tiers)
- [Dependency model](#dependency-model)
- [Plugin lifecycle](#plugin-lifecycle)
- [Cross-reference](#cross-reference)

---

## Plugin anatomy

Every plugin in this suite follows the same structure. Using `jupiter-swap-propose` as an example:

```
plugins/jupiter-swap-propose/
├── Cargo.toml # [workspace] + deps (wit-bindgen, serde, squads-defi-core)
├── manifest.toml # name, version, wasm_path, capabilities[], permissions[]
├── wit/v0/ # Vendored WIT contract files (unmodified from ZeroClaw v0.8.3)
│ ├── tool.wit # The tool-plugin world: execute(), name(), description()
│ ├── plugin-info.wit # Plugin identity: plugin_name(), plugin_version()
│ ├── logging.wit # Structured logging import
│ └── ... # types.wit, memory.wit, sockets.wit, etc.
├── src/
│ ├── lib.rs # WASM component shim — wit_bindgen::generate!() + exports
│ ├── config.rs # PluginConfig: parses __config HashMap into typed structs
│ ├── propose.rs # Core business logic (wasm-independent, host-testable)
│ ├── guardrails.rs # Policy checks (mint allowlist, slippage, notional, etc.)
│ ├── swap.rs # Transaction building from Jupiter swap instructions
│ └── error.rs # PluginError enum with Guardrail, Config, Rpc, Swap variants
└── tests/
 └── integration.rs # Host-run tests with mocked RPC
```

### Cargo.toml design

Each plugin is a **standalone Cargo workspace** (`[workspace]` at the bottom of `Cargo.toml`). This means:

- `cargo build --target wasm32-wasip2 --release` works from inside the plugin directory
- No workspace-level coupling — plugins do not share a root Cargo.toml
- All dependencies come from crates.io (no path dependencies)

The shared core library, [`squads-defi-core`](https://crates.io/crates/squads-defi-core), is published on crates.io and depended on by version. See [Dependency model](#dependency-model).

### `manifest.toml`

This file tells ZeroClaw about the plugin at load time:

```toml
name = "jupiter-swap-propose"
version = "0.1.0"
description = "Build a guarded Jupiter swap proposal wrapped in a Squads v4 multisig proposal"
author = "Squads DeFi Suite"
wasm_path = "jupiter_swap_propose.wasm"
capabilities = ["tool"]
permissions = ["http_client", "config_read"]
```

- **`capabilities`** — always `["tool"]` for tool plugins
- **`permissions`** — declared capabilities the host grants at load time. See [Permissions model](#permissions-model)
- **`wasm_path`** — must match the `.wasm` filename produced by `cargo build`

---

## The WIT contract

WIT (WebAssembly Interface Types) is the ABI between ZeroClaw and all plugins. The contract lives in `wit/v0/` and is vendored **unmodified** from [zeroclaw-labs/zeroclaw/wit/v0](https://github.com/zeroclaw-labs/zeroclaw/tree/main/wit/v0). Each plugin has its own copy of the WIT files so it can build independently.

### Key files

| File | Purpose |
|------|---------|
| [`tool.wit`](wit/v0/tool.wit) | Defines the `tool-plugin` world: `execute(args: string) -> result<tool-result, string>`, `name() -> string`, `description() -> string`, `parameters() -> string` (JSON schema) |
| [`plugin-info.wit`](wit/v0/plugin-info.wit) | Plugin identity: `plugin-name() -> string`, `plugin-version() -> string` |
| [`logging.wit`](wit/v0/logging.wit) | Structured logging: `log-record(level, event)`. Used by all plugins for audit traces |
| [`types.wit`](wit/v0/types.wit) | Shared types: `tool-result` (success + output string), error types |

### The tool-plugin world

Every tool plugin implements this world. The host calls:

1. **`name()`** — Returns the tool name (e.g., `"jupiter-swap-propose"`). Must match `manifest.toml` and the agent-facing name.
2. **`description()`** — Natural-language description the agent uses to decide when to call this tool.
3. **`parameters()`** — JSON Schema object describing the arguments the LLM should supply. **Critically, this schema never declares `__config`** — see [The `__config` jail](#the-__config-jail).
4. **`execute(args)`** — The main entry point. Receives JSON, returns `ToolResult { success: bool, output: string }`.

```wit
// tool.wit (simplified)
world tool-plugin {
 export name: func() -> string
 export description: func() -> string
 export parameters: func() -> string
 export execute: func(args: string) -> result<tool-result, string>
}
```

---

## The `__config` jail

This is the most important security mechanism in every plugin. It prevents prompt injection from redirecting funds, changing RPC endpoints, or modifying guardrails.

### How it works

1. The operator configures values in `~/.zeroclaw/config.toml` under `plugins.entries.<name>.config.*`.
2. When the LLM calls `execute()`, the **host strips** any `__config` field the LLM supplies and **injects** the stored configuration values before passing `args` to the plugin.
3. The LLM-visible `parameters()` JSON Schema **never** declares `__config`. The LLM cannot know it exists.
4. If the LLM injects `__config` into `args`, the host removes it. The plugin always receives the operator's values.

### Code pattern

```rust
// In every plugin:
#[derive(Deserialize)]
struct ExecuteArgs {
 #[serde(default)]
 __config: HashMap<String, String>, // Host-injected, LLM cannot spoof
 // Actual parameters (visible to the LLM):
 mint_address: Option<String>,
 amount: Option<String>,
 // ...
}

fn execute(args: String) -> Result<ToolResult, String> {
 let parsed: ExecuteArgs = serde_json::from_str(&args)
 .map_err(|e| format!("invalid arguments: {e}"))?;

 // Read from __config — these are guaranteed to be operator-configured
 let rpc_url = parsed.config.get("rpc_url")
 .ok_or("missing rpc_url in config")?;
 // ...
}
```

### What this prevents

- **Recipient redirect** — The `recipient` key in `solana-pay-request` is hard-wired from config. The LLM cannot say "refund to this address".
- **RPC substitution** — The `rpc_url` comes from config. An attacker cannot make the plugin query their own RPC.
- **Guardrail bypass** — Slippage caps, notional limits, mint allowlists all come from `__config`. The LLM cannot override them.

---

## Permissions model

Permissions are declared in `manifest.toml` and enforced by the ZeroClaw runtime at plugin load time:

| Permission | What it grants | Used by |
|-----------|---------------|---------|
| `http_client` | Outbound HTTPS requests (TLS performed host-side) | `jupiter-swap-propose`, `vault-watch`, `token-risk-check` |
| `config_read` | Access to the plugin's `__config` section | All plugins |

Sockets and websockets are not available for plugins in ZeroClaw v0.8.x. All network I/O uses the `waki` crate, which wraps `wasi:http`.

At build time, the plugin links against `waki` only when compiled for `wasm32-wasip2`:

```toml
[target.'cfg(target_family = "wasm")'.dependencies]
waki = { version = "0.5.1", features = ["json"] }
```

Host tests run with native Rust and do not compile the wasm-only dependencies:

```bash
cargo test -p jupiter-swap-propose # pure Rust, no wasm required
```

---

## Capsule tiers

The [ZeroClaw Solana bounty](https://superteam.fun/earn/listing/zeroclaw) defines three custody tiers. This suite targets T0 and T1:

### T0 — Read-only (token-risk-check, vault-watch)

- No transactions built
- No keys accessed
- No signatures produced
- Output is informational only
- Protocol: query public RPC endpoints

### T1 — Transaction builder (solana-pay-request, jupiter-swap-propose)

- Builds unsigned transactions only
- Never holds private keys
- Never signs
- Never broadcasts to the network
- All outputs must be approved through a separate signing mechanism (Squads multisig UI for proposals, Phantom/Solflare for Solana Pay URLs)

### What T2 would add

T2 plugins would hold decrypted private keys and sign transactions directly — available but strongly discouraged. This suite does not implement T2.

---

## Dependency model

### Why crates.io instead of path dependencies

The official [zeroclaw-labs/zeroclaw-plugins](https://github.com/zeroclaw-labs/zeroclaw-plugins) registry builds plugins in a CI sandbox that snapshots only `plugins/<name>` and `wit/v0/`. Path dependencies break because the snapshot does not include the parent workspace.

By publishing [`squads-defi-core`](https://crates.io/crates/squads-defi-core) to crates.io, each plugin:

1. Builds standalone — `cargo build` works from inside the plugin directory
2. Resolves the exact same dependency graph every time (lockfile)
3. Can be submitted to the registry without workspace gymnastics

### Features

`squads-defi-core` exposes two optional features:

| Feature | What it enables |
|---------|----------------|
| `squads-state` | `squads-multisig-program` dependency for Squads-specific types (vault PDAs, proposal structs) |
| `waki` | WASI HTTP client for the wasm target (used by plugins that fetch from RPCs) |

Plugins that need Squads types add `features = ["squads-state"]`:

```toml
squads-defi-core = { version = "0.1", features = ["squads-state"] }
```

Plugins that only need basic types (Pubkey, Blockhash) use default:

```toml
squads-defi-core = "0.1"
```

---

## Plugin lifecycle

```
┌─────────────┐ ┌──────────────┐ ┌──────────────┐
│ 1. Install │────>│ 2. Load │────>│ 3. Configure │
│ zeroclaw │ │ plugins │ │ config.toml │
│ plugin │ │ (auto- │ │ │
│ install <dir>│ │ discover) │ │ │
└─────────────┘ └──────┬───────┘ └──────────────┘
 │
 ▼
 ┌──────────────┐ ┌──────────────┐
 │ 4. Agent │<────│ 5. execute() │
 │ calls tool │ │ returns │
 │ by name │ │ ToolResult │
 └──────────────┘ └──────────────┘
```

1. **Install** — `zeroclaw plugin install /path/to/plugin` copies the `.wasm` + `manifest.toml` to `~/.zeroclaw/plugins/<name>/`.
2. **Load** — On daemon start (or `auto_discover = true`), ZeroClaw scans the plugins directory, loads each `.wasm` component, validates the manifest, and registers the tool.
3. **Configure** — The operator sets config values under `plugins.entries.<name>.config.*` in `config.toml`.
4. **Invoke** — The LLM calls the tool by name with JSON arguments. The host injects `__config`, then calls `execute()`.
5. **Return** — The plugin returns `{ success: true/false, output: "..." }`. The host passes the result to the LLM.

---

## Cross-reference

| Topic | See also |
|-------|----------|
| End-to-end setup walkthrough | [`GETTING_STARTED.md`](GETTING_STARTED.md) |
| Plugin configuration reference | [`config-template.toml`](config-template.toml) |
| Published core crate | [`squads-defi-core/README.md`](squads-defi-core/README.md) |
| ZeroClaw plugin documentation | [ZeroClaw docs — Writing a plugin](https://github.com/zeroclaw-labs/zeroclaw/tree/main/docs/plugins) |
| WIT contract specification | [wit/v0/](wit/v0/) (vendored from ZeroClaw) |
| Source code reference | [README.md](README.md#-for-developers) |
