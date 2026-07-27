# Squads DeFi Suite 

**AI-powered Solana DeFi through your chat app.** Swap tokens, monitor multisig vaults, request payments, and check token risks — all through a secure, self-hosted [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) agent. The agent proposes. Your Squads multisig disposes.

- [Quick Start (5 minutes)](#-quick-start-5-minutes)
- [Plugins at a Glance](#-plugins-at-a-glance)
- [Architecture](#-for-developers)
- [Contributing](#-contributing)

---

## Quick Start (5 minutes)

### 1. Install ZeroClaw

```bash
curl -fsSL https://raw.githubusercontent.com/zeroclaw-labs/zeroclaw/master/install.sh | bash
```

This installs the ZeroClaw binary. Follow the [official installation guide](https://docs.zeroclawlabs.ai/v0.8.3/en/) if you hit issues.

> **Important:** The release binary does not include the WASM plugin host. You need a source build with `--features plugins-wasm,plugins-wasm-cranelift`. See [GETTING_STARTED.md](GETTING_STARTED.md#building-zeroclaw-from-source) for exact instructions.

### 2. Set up your agent

```bash
zeroclaw quickstart
```

You'll need:
- An **LLM API key** (OpenAI, Anthropic, or any [supported provider](https://github.com/zeroclaw-labs/zeroclaw#-features))
- A **Telegram bot token** (from [@BotFather](https://t.me/BotFather)) or another [channel](https://github.com/zeroclaw-labs/zeroclaw#channels)

### 3. Install the plugins

Each plugin is a self-contained WebAssembly component. Install them individually:

```bash
# token-risk-check — no config dependencies, just an RPC URL
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh) token-risk-check

# solana-pay-request — needs a recipient address
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh) solana-pay-request

# vault-watch — needs a vault address and RPC
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh) vault-watch

# jupiter-swap-propose — needs vault, creator, mint allowlist
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh) jupiter-swap-propose
```

Or install all four at once:

```bash
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-all.sh)
```

### 4. Configure

Each plugin needs a small set of configuration values. Use `zeroclaw config set` to add them:

```bash
# token-risk-check
zeroclaw config set plugins.entries.token-risk-check.config.rpc_url https://api.mainnet-beta.solana.com

# solana-pay-request
zeroclaw config set plugins.entries.solana-pay-request.config.recipient YOUR_SOLANA_ADDRESS

# vault-watch
zeroclaw config set plugins.entries.vault-watch.config.rpc_url https://api.mainnet-beta.solana.com
zeroclaw config set plugins.entries.vault-watch.config.squads_vault YOUR_SQUADS_VAULT

# jupiter-swap-propose
zeroclaw config set plugins.entries.jupiter-swap-propose.config.rpc_url https://api.mainnet-beta.solana.com
zeroclaw config set plugins.entries.jupiter-swap-propose.config.squads_vault YOUR_SQUADS_VAULT
zeroclaw config set plugins.entries.jupiter-swap-propose.config.creator YOUR_AUTHORITY_PUBKEY
zeroclaw config set plugins.entries.jupiter-swap-propose.config.mint_allowlist EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
```

See [config-template.toml](config-template.toml) for a complete reference with all optional keys.

### 5. Restart and verify

```bash
# Kill any running daemon
pkill -f "zeroclaw daemon" 2>/dev/null

# Start fresh
zeroclaw daemon

# Verify the plugins loaded
zeroclaw plugin list
```

You should see all four plugins listed. Now message your agent:

```
You: What tools do you have?
Bot: token-risk-check, solana-pay-request, vault-watch, jupiter-swap-propose
```

---

## Plugins at a Glance

| Plugin | Tier | What it does | Try saying... | Permissions |
|--------|------|-------------|---------------|-------------|
| [`jupiter-swap-propose`](plugins/jupiter-swap-propose/) | T1 | Fetches a Jupiter quote, checks six configurable guardrails, returns an unsigned Squads v4 proposal | `"swap 10 SOL for USDC"` | `http_client`, `config_read` |
| [`vault-watch`](plugins/vault-watch/) | T0 | Returns a treasury briefing: pending proposals, token balances, lending health for a Squads vault | `"check my vault"` | `http_client`, `config_read` |
| [`solana-pay-request`](plugins/solana-pay-request/) | T1 | Builds a `solana:` payment URL with a config-locked recipient that the agent cannot redirect | `"request 25 USDC from Alice"` | `config_read` |
| [`token-risk-check`](plugins/token-risk-check/) | T0 | Reads on-chain mint account data and returns risk flags: mint authority, freeze authority, Token-2022 extensions | `"is EPjFWdd5 safe?"` | `http_client`, `config_read` |

**Custody tiers:**
- **T0** — Read-only. No transactions built, no keys accessed.
- **T1** — Transaction builder. Builds unsigned transactions only. The agent never holds keys, never signs, never broadcasts. All transactions must be approved through the Squads UI.

See [ARCHITECTURE.md](ARCHITECTURE.md#capsule-tiers) for a full explanation of capsule tiers and why they matter.

---

## For Developers

### Prerequisites

- **Rust** 1.87+ ([rustup.rs](https://rustup.rs))
- **wasm32-wasip2 target** — `rustup target add wasm32-wasip2`
- **ZeroClaw** built from source with WASM plugin support (see [GETTING_STARTED.md](GETTING_STARTED.md#building-zeroclaw-from-source))

### Repository structure

```
squads-defi-suite/
├── Cargo.toml # Workspace root (squads-defi-core only)
├── scripts/
│ ├── install-plugin.sh # One-liner installer for a single plugin
│ ├── install-all.sh # Installs all four plugins
│ ├── build.sh # Compiles all plugins to wasm32-wasip2
│ └── package.sh # Produces distribution zips in dist/
├── plugins/
│ ├── jupiter-swap-propose/ # Self-contained WASM plugin
│ │ ├── Cargo.toml # Standalone workspace, deps from crates.io
│ │ ├── manifest.toml # Plugin metadata and permissions
│ │ ├── wit/v0/ # Vendored WIT contract (identical to ZeroClaw v0.8.3)
│ │ ├── src/lib.rs # WASM component shim
│ │ └── src/*.rs # Pure Rust core, wasm-independent, host-testable
│ ├── vault-watch/ # Same structure
│ ├── solana-pay-request/
│ └── token-risk-check/
├── squads-defi-core/ # Published crate on crates.io
│ └── src/ # Shared types: Pubkey, Blockhash, Jupiter types, Squads types
├── wit/v0/ # Reference WIT files (each plugin has its own copy)
├── ARCHITECTURE.md # Full architecture documentation
├── GETTING_STARTED.md # Detailed setup walkthrough
├── config-template.toml # Complete config reference with all keys
└── README.md # This file
```

### Build all plugins

```bash
./scripts/build.sh
```

This compiles each plugin to `wasm32-wasip2` and places the `.wasm` binary alongside its `manifest.toml` for direct installation.

### Run tests

```bash
# All plugins + core library
cargo test --workspace

# Individual plugin
cargo test -p squads-defi-core
```

Tests use mocked RPC responses — no live network required.

### Package for distribution

```bash
./scripts/package.sh # produces dist/*.zip
```

Each zip contains the `.wasm` binary and `manifest.toml`, ready for `zeroclaw plugin install`.

### Dependency model

The shared core is published as [`squads-defi-core`](https://crates.io/crates/squads-defi-core) on crates.io. Plugins depend on it by version, not by path — this means:

1. **Each plugin is a standalone workspace** — anyone can build it without cloning the full repo
2. **No workspace coupling** — version bumps don't cascade through path dependencies
3. **Reproducible builds** — the exact same dependency graph is resolved every time

See [ARCHITECTURE.md](ARCHITECTURE.md#dependency-model) for the rationale.

---

## Cross-Reference Map

| Document | What it covers |
|----------|---------------|
| [`GETTING_STARTED.md`](GETTING_STARTED.md) | End-to-end walkthrough: ZeroClaw source build → plugin install → agent config → first message |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | WIT contracts, `__config` jail design, permission model, capsule tiers, dependency rationale |
| [`config-template.toml`](config-template.toml) | Complete config template with every plugin key documented |
| [`plugins/jupiter-swap-propose/README.md`](plugins/jupiter-swap-propose/README.md) | Swap plugin: guardrails, parameters, output format, security |
| [`plugins/vault-watch/README.md`](plugins/vault-watch/README.md) | Vault plugin: on-chain data sources, briefing format, cron scheduling |
| [`plugins/solana-pay-request/README.md`](plugins/solana-pay-request/README.md) | Payment plugin: URL format, config-locked recipient, anti-redirect |
| [`plugins/token-risk-check/README.md`](plugins/token-risk-check/README.md) | Risk plugin: mint data parsing, Token-2022 extensions, risk scoring |
| [`squads-defi-core/README.md`](squads-defi-core/README.md) | Published crate: features, versioning, usage from other plugins |
| [`scripts/README.md`](scripts/README.md) | Build, package, and install scripts reference |

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on adding new plugins, submitting changes, and the CI pipeline.

## Security

Each plugin enforces its configuration at runtime through the ZeroClaw `__config` injection mechanism. The host strips any value the LLM supplies for `__config` and substitutes the operator-configured values before the plugin executes. The agent cannot see, modify, or bypass these values. See [ARCHITECTURE.md](ARCHITECTURE.md#config-jail) for the full design.

## License

MIT
