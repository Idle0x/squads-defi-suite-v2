# Getting Started

This guide walks through setting up the Squads DeFi Suite plugins with ZeroClaw. It covers creating a Solana wallet, setting up a Squads vault, configuring ZeroClaw, installing the plugins, and testing the setup.

## Prerequisites

| Item | Source | Notes |
|------|--------|-------|
| Solana wallet | [Phantom](https://phantom.app/) or [Backpack](https://backpack.app/) | Must hold ~0.01 SOL for transaction fees |
| Squads vault | [app.squads.so](https://app.squads.so) | Create a multisig with at least one signer |
| RPC URL | [helius.dev](https://helius.dev) (free tier) | Used for all on-chain queries |
| ZeroClaw | [Installation guide](https://docs.zeroclawlabs.ai/master/en/setup/index.html) | v0.8 or later |
| LLM API key | OpenRouter, OpenAI, or Anthropic | Required by ZeroClaw for agent operation |

---

## Step 1: Create a Solana wallet

1. Install Phantom or Backpack as a browser extension.
2. Create a new wallet and store the 12-word seed phrase offline (paper, hardware wallet).
3. The wallet address is a base58 string starting with a capital letter, for example `7xKX...`.
4. Fund the wallet with approximately 0.01 SOL for network fees.

## Step 2: Create a Squads vault

1. Go to [app.squads.so](https://app.squads.so) and connect your wallet.
2. Create a new multisig vault.
3. Add at least one signer (your wallet address).
4. Set the threshold to 1 (single-signer) for testing, or higher for production.
5. Note the vault address. It will be used in plugin configuration.

## Step 3: Install ZeroClaw

Follow the [ZeroClaw installation guide](https://docs.zeroclawlabs.ai/master/en/setup/index.html) for your platform.

After installation, run the quickstart wizard:

```bash
zeroclaw quickstart
```

The wizard guides you through:
- Selecting an LLM provider (OpenRouter, Anthropic, OpenAI, or Ollama)
- Configuring an API key
- Setting up Telegram or Discord channels
- Naming the agent

## Step 4: Build the plugins

Clone the repository and build the WASM components:

```bash
git clone https://github.com/Idle0x/squads-defi-suite
cd squads-defi-suite
rustup target add wasm32-wasip2
./scripts/build.sh
```

This produces `.wasm` files in `target/wasm32-wasip2/release/`.

## Step 5: Install the plugins

Create a plugin directory for each plugin and copy the files:

```bash
mkdir -p ~/.zeroclaw/plugins/jupiter-swap-propose
cp target/wasm32-wasip2/release/jupiter_swap_propose.wasm ~/.zeroclaw/plugins/jupiter-swap-propose/
cp plugins/jupiter-swap-propose/manifest.toml ~/.zeroclaw/plugins/jupiter-swap-propose/

mkdir -p ~/.zeroclaw/plugins/vault-watch
cp target/wasm32-wasip2/release/vault_watch.wasm ~/.zeroclaw/plugins/vault-watch/
cp plugins/vault-watch/manifest.toml ~/.zeroclaw/plugins/vault-watch/

mkdir -p ~/.zeroclaw/plugins/solana-pay-request
cp target/wasm32-wasip2/release/solana_pay_request.wasm ~/.zeroclaw/plugins/solana-pay-request/
cp plugins/solana-pay-request/manifest.toml ~/.zeroclaw/plugins/solana-pay-request/

mkdir -p ~/.zeroclaw/plugins/token-risk-check
cp target/wasm32-wasip2/release/token_risk_check.wasm ~/.zeroclaw/plugins/token-risk-check/
cp plugins/token-risk-check/manifest.toml ~/.zeroclaw/plugins/token-risk-check/
```

## Step 6: Configure the plugins

Edit `~/.zeroclaw/config.toml` and add the following:

```toml
schema_version = 3
locale = "en"

[plugins]
enabled = true
auto_discover = true

[[plugins.entries]]
name = "jupiter-swap-propose"
config.rpc_url = "https://mainnet.helius-rpc.com/?api-key=YOUR_KEY"
config.squads_vault = "YOUR_SQUADS_VAULT_ADDRESS"
config.mint_allowlist = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v,Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"
config.max_slippage_bps = "100"
config.max_notional_usd = "100"
config.per_day_cap_usd = "50"

[[plugins.entries]]
name = "vault-watch"
config.rpc_url = "https://mainnet.helius-rpc.com/?api-key=YOUR_KEY"
config.squads_vault = "YOUR_SQUADS_VAULT_ADDRESS"

[[plugins.entries]]
name = "solana-pay-request"
config.recipient = "YOUR_WALLET_ADDRESS"

[[plugins.entries]]
name = "token-risk-check"
config.rpc_url = "https://mainnet.helius-rpc.com/?api-key=YOUR_KEY"
```

All configuration values are injected by the ZeroClaw host at runtime. The LLM does not have access to these values.

## Step 7: Restart ZeroClaw

```bash
zeroclaw daemon restart
# or
zeroclaw service restart
```

Then check the daemon logs for plugin loading:

```bash
zeroclaw daemon logs | grep plugin
```

## Step 8: Test

Send the following messages to your ZeroClaw agent:

| Message | Expected behavior |
|---------|-------------------|
| `swap 0.01 SOL for USDC` | The agent fetches a Jupiter quote, passes it to jupiter-swap-propose with the configured guardrails, and returns a Squads proposal. The proposal then requires approval in app.squads.so. |
| `check my vault` | The agent calls vault-watch, which queries on-chain data and returns a briefing. |
| `is EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v safe?` | The agent calls token-risk-check, which reads the mint account data and returns risk flags. |
| `request 1 USDC payment` | The agent calls solana-pay-request and returns a solana: URL. |

## Troubleshooting

| Symptom | Likely cause |
|---------|-------------|
| `Plugin not found` | The plugin directory or manifest.toml is missing from `~/.zeroclaw/plugins/` |
| `Config key missing` | The plugin's `[[plugins.entries]]` section is not in `~/.zeroclaw/config.toml` |
| `RPC error` | The RPC URL is incorrect, unreachable, or has depleted credits |
| `Guardrail denied` | The swap exceeds a configured limit (mint allowlist, slippage, notional, or daily cap) |
| ZeroClaw not responding | The daemon is not running. Check `zeroclaw daemon status`. |

---

## Next steps

- [README](README.md) — architecture and plugin overview
- Plugin READMEs in `plugins/<name>/README.md` — per-plugin configuration and details
- [CONTRIBUTING.md](CONTRIBUTING.md) — adding plugins or submitting changes
