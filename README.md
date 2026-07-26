# Squads DeFi Suite

ZeroClaw plugins for Solana DeFi operations through Squads multisig vaults. Each plugin compiles to a WASM component and registers as a callable tool in the ZeroClaw agent runtime.

## Plugins

| Plugin | Function | Access |
|--------|----------|--------|
| [jupiter-swap-propose](plugins/jupiter-swap-propose/README.md) | Receives a Jupiter quote via the host, checks configurable guardrails, and returns an unsigned Squads v4 proposal transaction | Read-only (RPC URL) |
| [vault-watch](plugins/vault-watch/README.md) | Returns treasury briefings: pending proposals, token balances, and lending health factors for a Squads vault | Read-only (RPC URL) |
| [solana-pay-request](plugins/solana-pay-request/README.md) | Builds a `solana:` payment URL for SOL or SPL token payment requests with a config-enforced recipient | Build (no secrets) |
| [token-risk-check](plugins/token-risk-check/README.md) | Reads on-chain mint account data and returns risk flags: mint authority, freeze authority, Token-2022 extensions | Read-only (RPC URL) |

All plugins are defined in the [`wit/v0/`](wit/v0/) interface directory and implement the `tool-plugin` world. The WIT bindings are generated at compile time by `wit-bindgen` 0.46.

## Security

Configuration values (RPC URLs, vault addresses, mint allowlists, limits) are injected by the ZeroClaw host at runtime through a `__config` field. The host strips any value the LLM supplies for `__config` and substitutes the operator-configured values before the plugin executes. The LLM-visible parameter schema never declares `__config`.

Each plugin declares its required capabilities in `manifest.toml`:
- `http_client` — permits outgoing HTTP requests to the Solana RPC endpoint for on-chain data queries
- `config_read` — permits receiving configuration from the host

The runtime grants these capabilities at plugin load time. Source: [zeroclaw-plugins runtime.rs](https://github.com/zeroclaw-labs/zeroclaw/blob/main/crates/zeroclaw-plugins/src/runtime.rs).

## Quick start

### Prerequisites

- [ZeroClaw v0.8+](https://docs.zeroclawlabs.ai/master/en/setup/index.html) installed and configured
- A Solana RPC endpoint (see [helius.dev](https://helius.dev) for a free tier)

### Install from source

See [GETTING_STARTED.md](GETTING_STARTED.md) for a step-by-step walkthrough.

```bash
git clone https://github.com/Idle0x/squads-defi-suite
cd squads-defi-suite
rustup target add wasm32-wasip2
./scripts/build.sh
```

Copy each plugin's `.wasm` file and `manifest.toml` to `~/.zeroclaw/plugins/<name>/`, or use the distribution zips produced by `./scripts/package.sh`.

### Configure

Add entries to `~/.zeroclaw/config.toml`:

```toml
schema_version = 3
locale = "en"

[plugins]
enabled = true
auto_discover = true

[[plugins.entries]]
name = "jupiter-swap-propose"
config.rpc_url = "https://api.mainnet-beta.solana.com"
config.squads_vault = "GfQkesR7PGJP7etL6scmp8R1SaHLBcryCUaHehgLjmCW"
config.mint_allowlist = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v,Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"
config.max_slippage_bps = "100"
config.max_notional_usd = "1000"
config.per_day_cap_usd = "500"
```

Each plugin's README documents its required and optional config keys.

## Development

- [Getting Started Guide](GETTING_STARTED.md) — full walkthrough for new users
- [Contributing](CONTRIBUTING.md) — how to add plugins or submit changes
- [Source code](plugins/) — per-plugin source directories under `plugins/`
- [Shared core](squads-defi-core/) — `squads-defi-core` crate with shared types and RPC logic

### Build

```bash
./scripts/build.sh      # compiles all plugins to wasm32-wasip2
./scripts/verify.sh     # cross-checks manifest.toml vs Cargo.toml
./scripts/package.sh    # produces distribution zips in dist/
```

### Tests

```bash
cargo test --workspace
```

## License

MIT
