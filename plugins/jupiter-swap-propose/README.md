# jupiter-swap-propose

> **Tier:** T1 (transaction builder) — builds unsigned Squads v4 proposals. Never holds keys, never signs, never broadcasts.

Receives a Jupiter quote and swap instructions from the host, checks six configurable guardrails, and returns an unsigned Squads v4 multisig proposal transaction. The proposal must be approved and executed through [app.squads.so](https://app.squads.so).

- [Quick install](#quick-install)
- [Configuration](#configuration)
- [Parameters](#parameters)
- [Guardrails](#guardrails)
- [Output format](#output-format)
- [Example](#example)
- [Security](#security)
- [Source](#source)

---

## Quick install

```bash
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh) jupiter-swap-propose
```

Or build from source: see [GETTING_STARTED.md](../../GETTING_STARTED.md#step-4-install-the-plugins).

---

## Configuration

The plugin receives its configuration through the host's `__config` injection at runtime. The LLM cannot see or modify these values. See [ARCHITECTURE.md](../../ARCHITECTURE.md#the-__config-jail) for the security design.

```bash
# Required
zeroclaw config set plugins.entries.jupiter-swap-propose.config.rpc_url https://api.mainnet-beta.solana.com
zeroclaw config set plugins.entries.jupiter-swap-propose.config.squads_vault YOUR_SQUADS_VAULT
zeroclaw config set plugins.entries.jupiter-swap-propose.config.creator YOUR_AUTHORITY_PUBKEY
zeroclaw config set plugins.entries.jupiter-swap-propose.config.mint_allowlist EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
```

### Configuration keys

| Key | Required | Default | Description |
|-----|----------|---------|-------------|
| `rpc_url` | Yes | — | Solana JSON-RPC endpoint URL (mainnet or devnet) |
| `squads_vault` | Yes | — | Squads multisig vault address. All proposals are created under this vault. |
| `creator` | Yes | — | Transaction creator pubkey. Must be a signer on the vault. |
| `mint_allowlist` | Yes | (empty, denies all) | Comma-separated list of allowed output token mint addresses. An empty list blocks every swap. |
| `max_slippage_bps` | No | `100` | Maximum allowed slippage in basis points (100 = 1%). Rejected if the Jupiter quote exceeds this. |
| `max_notional_usd` | No | `1000` | Maximum notional value per swap in USD. |
| `per_day_cap_usd` | No | `10000` | Cumulative daily spending cap in USD across all swaps. |
| `proposal_expiry_hours` | No | `24` | Proposal validity window in hours (range: 1–168). |
| `jupiter_url` | No | `https://quote-api.jup.ag/v6` | Jupiter API base URL. Change only if using a custom Jupiter endpoint. |
| `squads_program_id` | No | Mainnet v4 (`SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf`) | Squads program ID. Change only for devnet or custom deployments. |
| `transaction_index` | No | `0` | Next transaction index for the vault. Increments automatically after approval. |
| `squads_program_id` | No | `SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf` | Squads program ID |

---

## Parameters

The LLM calls this plugin with a JSON object containing these fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `quote_json` | string | yes | Full Jupiter quote API response as JSON (from `api.jup.ag/quote`) |
| `swap_instructions_json` | string | yes | Full Jupiter swap instructions response as JSON (from `api.jup.ag/swap-instructions`) |
| `daily_volume_usd` | string | yes | Cumulative daily swap volume in USD, tracked by the host |
| `usd_per_unit` | number | no | USD price per base unit of the input token (required if `max_notional_usd > 0`) |

The `__config` field is injected by the host at runtime. The LLM-visible parameter schema (defined in [`src/lib.rs`](src/lib.rs)) does not include `__config`. See [ARCHITECTURE.md](../../ARCHITECTURE.md#the-__config-jail).

---

## Guardrails

The plugin checks six conditions before building a proposal. Each is enforced in [`src/guardrails.rs`](src/guardrails.rs) and the shared [`squads-defi-core`](https://crates.io/crates/squads-defi-core) Jupiter client. A denial returns an error message explaining why.

| # | Guardrail | Source | Configured by |
|---|-----------|--------|--------------|
| 1 | **Output mint** must be in `mint_allowlist` | [`guardrails.rs`](src/guardrails.rs) | `mint_allowlist` in config |
| 2 | **Slippage** must not exceed `max_slippage_bps` | [`guardrails.rs`](src/guardrails.rs) | `max_slippage_bps` in config |
| 3 | **Price impact** must not exceed 5% | [`propose.rs`](src/propose.rs#L131) | Hardcoded (not configurable) |
| 4 | **Route hops** must not exceed 5 | [`propose.rs`](src/propose.rs#L132) | Hardcoded (not configurable) |
| 5 | **Notional value** must not exceed `max_notional_usd` | [`squads-defi-core/src/jupiter.rs`](../../squads-defi-core/src/jupiter.rs) | `max_notional_usd` in config |
| 6 | **Daily cap** cumulative spent + current swap must not exceed `per_day_cap_usd` | [`squads-defi-core/src/jupiter.rs`](../../squads-defi-core/src/jupiter.rs) | `per_day_cap_usd` in config |

---

## Output format

### Success

```json
{
 "meta_tx_base64": "AQAAAAABJwEAAAAAAA...",
 "summary": "Swap: 10 SOL → ~2,300 USDC. Slippage: 50 bps | Price Impact: 0.05%. Route: 3 hops. Open Squads app to review and sign.",
 "proposal_expires_at": 1711324800,
 "proposal_address": "EcHjSmTAkQjT7YZXx6F7qGaNN2ZmKGsNGhXjSWRWaN5H",
 "status": "created"
}
```

| Field | Description |
|-------|-------------|
| `meta_tx_base64` | Base64-encoded unsigned versioned transaction. Paste into Squads to review and approve. |
| `summary` | Human-readable swap description (≤200 tokens per bounty requirement). |
| `proposal_expires_at` | Unix timestamp (seconds) when the proposal expires. |
| `proposal_address` | The Squads proposal PDA on-chain. |
| `status` | Always `"created"` on success. |

### Guardrail denial

```json
{
 "success": false,
 "output": "",
 "error": "Denied: Notional value $150,000 exceeds max notional $1,000"
}
```

### RPC or config error

```json
{
 "success": false,
 "output": "",
 "error": "Config error: missing rpc_url"
}
```

---

## Example

```
You: swap 10 SOL for USDC
Agent:
 Swap Proposal Created!

 Swap: 10 SOL → ~2,300 USDC
 Slippage: 50 bps | Price Impact: 0.05%
 Route: Jupiter → Orca → Raydium (3 hops)
 Expires: +72 hours

 Open Squads app to review and sign:
 → https://app.squads.so/proposal/EcHjSmTAkQjT7YZXx6F7qGaNN2ZmKGsNGhXjSWRWaN5H
```

```
You: swap 1000 SOL for a sketchy token
Agent:
 Denied: The output token mint is not in the configured allowlist.
 Contact your operator to add it to mint_allowlist.
```

---

## Security

### Attack surfaces and mitigations

| Attack | Mitigation | Source |
|--------|-----------|--------|
| Prompt injection redirects funds | Recipient/output mint locked by `__config` — LLM cannot override | [`config.rs`](src/config.rs) |
| LLM overrides slippage, caps | Guardrails enforced below the model in pure Rust | [`guardrails.rs`](src/guardrails.rs) |
| LLM fabricates quote data | Plugin rebuilds the transaction from Jupiter's own swap instructions, not the quote | [`swap.rs`](src/swap.rs) |
| LLM bypasses custody | T1 — plugin never holds keys, never signs, never broadcasts | [`lib.rs`](src/lib.rs) |

### Custody model

This is a **T1** plugin. The agent:
- Builds unsigned transactions only
- Never stores, accesses, or derives private keys
- Never signs any message
- Never broadcasts to the network

All outputs must be approved through the Squads multisig UI before execution.

### Permission model

The plugin declares `["http_client", "config_read"]` in its [manifest](manifest.toml):
- `http_client` — permissions HTTPS outbound to the configured Jupiter API and Solana RPC
- `config_read` — permits receiving the operator-configured values from the host

See [ARCHITECTURE.md](../../ARCHITECTURE.md#permissions-model) for details.

---

## Source

| Component | File | Description |
|-----------|------|-------------|
| Plugin entry point | [`src/lib.rs`](src/lib.rs) | WASM shim, WIT bindgen, `execute()` implementation |
| Proposal builder | [`src/propose.rs`](src/propose.rs) | Jupiter quote → protocol checks → Squads meta-transaction |
| Guardrail checks | [`src/guardrails.rs`](src/guardrails.rs) | Slippage, notional, allowlist, price impact validation |
| Config parsing | [`src/config.rs`](src/config.rs) | `__config` HashMap → typed `PluginConfig` struct |
| Swap tx builder | [`src/swap.rs`](src/swap.rs) | Rebuilds the swap transaction from Jupiter's `swapInstructions` response |
| Error types | [`src/error.rs`](src/error.rs) | `PluginError` enum with `Guardrail`, `Config`, `Rpc`, `Swap` variants |
| Integration tests | [`tests/`](tests/) | Host-run tests with mocked RPC responses |

---

## See also

- [ARCHITECTURE.md](../../ARCHITECTURE.md) — WIT contracts, `__config` jail, permissions model
- [GETTING_STARTED.md](../../GETTING_STARTED.md) — End-to-end setup walkthrough
- [config.toml.template](../../config.toml.template) — Complete config reference
- [`squads-defi-core`](https://crates.io/crates/squads-defi-core) — Published shared core crate
