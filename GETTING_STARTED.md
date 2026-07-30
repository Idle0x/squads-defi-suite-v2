# Getting Started

This guide walks through setting up the Squads DeFi Suite plugins end-to-end: from a bare machine to a working agent that can swap tokens, monitor vaults, request payments, and check token risks.

**Total time:** ~30 minutes.

- [Prerequisites](#prerequisites)
- [Step 1: Solana wallet](#step-1-create-a-solana-wallet)
- [Step 2: Squads vault](#step-2-create-a-squads-vault)
- [Step 3: ZeroClaw from source](#step-3-build-zeroclaw-from-source)
- [Step 4: Install plugins](#step-4-install-the-plugins)
- [Step 5: Configure](#step-5-configure)
- [Step 6: Restart and verify](#step-6-restart-and-verify)
- [Step 7: Chat with your agent](#step-7-chat-with-your-agent)
- [Troubleshooting](#troubleshooting)

---

## Prerequisites

| Item | Source | Notes |
|------|--------|-------|
| Solana wallet | [Phantom](https://phantom.app/) or [Backpack](https://backpack.app/) | Must hold ~0.01 SOL for network fees |
| Squads vault | [app.squads.so](https://app.squads.so) | Multisig with at least one signer |
| Solana RPC endpoint | [Helius](https://helius.dev) (free tier), [Triton](https://triton.one), or public endpoint | Used for all on-chain queries |
| Rust toolchain | [rustup.rs](https://rustup.rs) | Rust 1.87+ recommended |
| LLM API key | [OpenRouter](https://openrouter.ai), [OpenAI](https://platform.openai.com), or [Anthropic](https://console.anthropic.com) | Powers the ZeroClaw agent |
| Telegram bot token | [@BotFather](https://t.me/BotFather) on Telegram | Chat channel for the agent |

---

## Step 1: Create a Solana wallet

1. Install [Phantom](https://phantom.app/) or [Backpack](https://backpack.app/) as a browser extension.
2. Create a new wallet. Store the 12-word seed phrase offline (paper or hardware wallet).
3. Fund the wallet with approximately 0.01 SOL from an exchange or faucet.
4. Copy the wallet address — a base58 string like `7xKXmEpMUwMcK2K4mWnFN3Jsd9PkLwHb3M5A5jPpY6h`. You will need this for configuration.

---

## Step 2: Create a Squads vault

1. Go to [app.squads.so](https://app.squads.so) and connect your wallet.
2. Create a new multisig vault.
3. Add at least one signer (your wallet address from Step 1).
4. Set the threshold to 1 (single-signer) for testing, or higher for production.
5. Copy the vault address. It will be used in plugin configuration.

---

## Step 3: Build ZeroClaw from source

> **Why from source?** The ZeroClaw release binary does not include the WASM plugin host. You must build from source with the `plugins-wasm` feature. See [ARCHITECTURE.md](ARCHITECTURE.md#plugin-lifecycle) for why.

```bash
# Install the Rust toolchain if you haven't already
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Add the WASM target
rustup target add wasm32-wasip2

# Clone and build ZeroClaw
git clone https://github.com/zeroclaw-labs/zeroclaw.git
cd zeroclaw
cargo build --release --features plugins-wasm,plugins-wasm-cranelift

# The binary is at target/release/zeroclaw
# Optionally add it to your PATH:
cp target/release/zeroclaw ~/.cargo/bin/
```

### Run the quickstart wizard

```bash
zeroclaw quickstart
```

The wizard guides you through:
- Selecting an LLM provider (OpenRouter, Anthropic, OpenAI, or Ollama)
- Configuring your API key
- Setting up a Telegram channel (create a bot token from [@BotFather](https://t.me/BotFather))
- Naming your agent

### Enable auto-discover (required)

```bash
zeroclaw config set plugins.auto_discover true
```

Ensure `risk_profiles.balanced.level` is set to `full` for initial testing:

```bash
zeroclaw config set risk_profiles.balanced.level full
```

See [ARCHITECTURE.md](ARCHITECTURE.md#plugin-lifecycle) for details on the plugin loading sequence.

---

## Step 4: Install the plugins

Each plugin is a self-contained WebAssembly component. Install them individually or all at once.

### Option A: One-liner install (recommended)

```bash
# Individual plugins
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh) token-risk-check
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh) solana-pay-request
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh) vault-watch
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh) swap-propose
```

### Option B: All at once

```bash
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-all.sh)
```

### Option C: Build from source

```bash
git clone https://github.com/Idle0x/squads-defi-suite-v2
cd squads-defi-suite-v2
./scripts/build.sh
```

Then install each plugin manually:

```bash
zeroclaw plugin install plugins/token-risk-check
zeroclaw plugin install plugins/solana-pay-request
zeroclaw plugin install plugins/vault-watch
zeroclaw plugin install plugins/swap-propose
```

### Verify installation

```bash
zeroclaw plugin list
```

Expected output:

```
Installed plugins:
 swap-propose v0.1.0 — Build a guarded Jupiter swap proposal...
 vault-watch v0.1.0 — Daily treasury briefing...
 solana-pay-request v0.1.0 — Build Solana Pay URLs...
 token-risk-check v0.1.0 — Analyze SPL token risks...
```

---

## Step 5: Configure

Each plugin reads its configuration from `~/.zeroclaw/config.toml` through the `__config` injection mechanism. The LLM cannot see or modify these values. See [ARCHITECTURE.md](ARCHITECTURE.md#the-__config-jail) for the security design.

### Minimal configuration

```bash
# token-risk-check
zeroclaw config set plugins.entries.token-risk-check.config.rpc_url https://api.mainnet-beta.solana.com

# solana-pay-request
zeroclaw config set plugins.entries.solana-pay-request.config.recipient YOUR_SOLANA_ADDRESS

# vault-watch
zeroclaw config set plugins.entries.vault-watch.config.rpc_url https://api.mainnet-beta.solana.com
zeroclaw config set plugins.entries.vault-watch.config.squads_vault YOUR_SQUADS_VAULT

# swap-propose
zeroclaw config set plugins.entries.swap-propose.config.rpc_url https://api.mainnet-beta.solana.com
zeroclaw config set plugins.entries.swap-propose.config.squads_vault YOUR_SQUADS_VAULT
zeroclaw config set plugins.entries.swap-propose.config.creator YOUR_AUTHORITY_PUBKEY
zeroclaw config set plugins.entries.swap-propose.config.mint_allowlist EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
```

### Extended configuration (swap-propose guardrails)

```bash
# Optional — sensible defaults are applied:
zeroclaw config set plugins.entries.swap-propose.config.max_slippage_bps 50
zeroclaw config set plugins.entries.swap-propose.config.max_notional_usd 1000
zeroclaw config set plugins.entries.swap-propose.config.per_day_cap_usd 5000
zeroclaw config set plugins.entries.swap-propose.config.proposal_expiry_hours 72
```

See [config.toml.template](config.toml.template) for every available key with descriptions and defaults.

---

## Step 6: Restart and verify

```bash
# Kill any running daemon
pkill -f "zeroclaw daemon" 2>/dev/null

# Start fresh
zeroclaw daemon

# Check the runtime trace for plugin loading
grep "Loaded WASM plugin tools" ~/.zeroclaw/data/state/runtime-trace.jsonl
```

Expected output:
```
"Loaded WASM plugin tools — count: 4"
```

---

## Step 7: Chat with your agent

Open your Telegram chat with the agent and try these messages:

| Message | What happens | Plugin used |
|---------|-------------|-------------|
| `"what tools do you have?"` | Lists available tools | — |
| `"is EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v safe?"` | Returns mint authority, freeze authority, and risk level | `token-risk-check` |
| `"request 0.1 SOL payment"` | Returns a `solana:` URL | `solana-pay-request` |
| `"check my vault"` | Returns treasury briefing with balances and proposals | `vault-watch` |
| `"swap 0.01 SOL for USDC"` | Returns an unsigned Squads proposal for review | `swap-propose` |

### Example dialog

```
You: is EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v safe?
Bot: Token Risk Analysis — EPjFWdd5...
 Risk Level: Low 
 Mint Authority: Revoked
 Freeze Authority: None
 Token-2022: No
 This is the official USDC mint. No mint/freeze risk detected.
```

```
You: swap 0.01 SOL for USDC
Bot: Swap Proposal Created!
 Swap: 0.01 SOL → ~$0.25 USDC
 Slippage: 50 bps | Price Impact: 0.01%
 Open Squads app to review and sign:
 → https://app.squads.so/proposal/...
```

---

## Cost tracking

ZeroClaw can track API costs and enforce daily/monthly budgets. This is optional but recommended for the $2 test budget.

```bash
# Set a daily budget of $1
zeroclaw config set cost.daily_budget "1.00"

# Set prices for the model provider you're using (example: OpenAI GPT-4o)
zeroclaw config set cost.rates.providers.models.openai '{"gpt-4o": {"input": 2.50, "output": 10.00}}'

# View cost history
zeroclaw cost
```

The cost ledger is append-only and attributed to the originating agent. Budget enforcement blocks calls once the cap is reached. The ledger resets at midnight UTC for daily budgets and on the 1st of the month for monthly budgets.

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|-------------|-----|
| `zeroclaw plugin list` shows nothing | `auto_discover` is not enabled | Run `zeroclaw config set plugins.auto_discover true` and restart |
| Agent says "I don't have that tool" | Risk profile blocking plugin tools | Set `zeroclaw config set risk_profiles.balanced.level full` and restart |
| `Config key missing` error | Required config key not set | Check the plugin's README for required keys |
| `RPC error` | RPC URL is incorrect or unreachable | Verify the URL with `curl $RPC_URL -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'` |
| `Guardrail denied` | Swap exceeds configured limits | Increase `max_notional_usd`, `per_day_cap_usd`, or add the mint to `mint_allowlist` |
| Daemon doesn't start | Port already in use | Run `pkill -f "zeroclaw daemon"` then retry |
| Plugin not found in registry | Plugins are not yet in the public registry | Install from source using `zeroclaw plugin install /path/to/plugin` |

---

## Next steps

| Resource | What it covers |
|----------|---------------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | WIT contracts, `__config` jail, permission model, capsule tiers |
| [plugins/swap-propose/README.md](plugins/swap-propose/README.md) | Guardrails, parameters, output format |
| [plugins/vault-watch/README.md](plugins/vault-watch/README.md) | On-chain data sources, briefing format |
| [plugins/solana-pay-request/README.md](plugins/solana-pay-request/README.md) | URL format, anti-redirect design |
| [plugins/token-risk-check/README.md](plugins/token-risk-check/README.md) | Mint data parsing, Token-2022 extensions |
| [squads-defi-core/README.md](squads-defi-core/README.md) | Published crate features and versioning |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Adding plugins or submitting changes |
