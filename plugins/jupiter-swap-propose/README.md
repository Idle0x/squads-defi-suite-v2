# jupiter-swap-propose

Receives a Jupiter quote and swap instructions from the host, checks six configurable guardrails, and returns an unsigned Squads v4 multisig proposal transaction. The proposal must be approved and executed through [app.squads.so](https://app.squads.so).

Source: [`plugins/jupiter-swap-propose/src/`](src/) ([lib.rs](src/lib.rs), [propose.rs](src/propose.rs), [guardrails.rs](src/guardrails.rs), [config.rs](src/config.rs), [error.rs](src/error.rs))

## Installation

See [GETTING_STARTED.md](../../GETTING_STARTED.md) for full setup instructions.

## Configuration

The plugin receives its configuration through the host's `__config` injection at runtime. Add an entry to the `[[plugins.entries]]` array in `~/.zeroclaw/config.toml`:

```toml
[[plugins.entries]]
name = "jupiter-swap-propose"
config.rpc_url = "https://api.mainnet-beta.solana.com"
config.squads_vault = "GfQkesR7PGJP7etL6scmp8R1SaHLBcryCUaHehgLjmCW"
config.mint_allowlist = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v,Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"
config.max_slippage_bps = "100"
config.max_notional_usd = "1000"
config.per_day_cap_usd = "500"
```

### Configuration keys

| Key | Required | Default | Description |
|-----|----------|---------|-------------|
| `rpc_url` | Yes | — | Solana JSON-RPC endpoint URL |
| `squads_vault` | Yes | — | Squads multisig vault address |
| `mint_allowlist` | Yes | (empty, denies all) | Comma-separated list of allowed output token mint addresses |
| `max_slippage_bps` | No | `100` | Maximum allowed slippage in basis points (100 = 1%) |
| `max_notional_usd` | No | `1000` | Maximum notional value per swap in USD |
| `per_day_cap_usd` | No | `10000` | Cumulative daily spending cap in USD |
| `proposal_expiry_hours` | No | `24` | Proposal validity window in hours (range: 1-168) |
| `creator` | No | `squads_vault` | Transaction creator pubkey |
| `jupiter_url` | No | `https://quote-api.jup.ag/v6` | Jupiter API base URL |
| `squads_program_id` | No | Mainnet v4 | Squads program ID |

The values are enforced by the plugin code and cannot be modified through LLM prompts. See [`guardrails.rs`](src/guardrails.rs) for the implementation.

## Parameters

The agent calls this plugin with a JSON object containing the following fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `quote_json` | string | yes | Full Jupiter quote API response as JSON |
| `swap_instructions_json` | string | yes | Full Jupiter swap instructions response as JSON |
| `daily_volume_usd` | string | yes | Cumulative daily swap volume in USD (tracked by the host) |
| `usd_per_unit` | number | no | USD price per base unit of the input token (for notional guardrail) |

The `__config` field is injected by the host at runtime. The LLM visible parameter schema does not include `__config`, defined in [`src/lib.rs`](src/lib.rs#L94).

## Guardrails

The plugin checks six conditions before building a proposal. Each check is enforced in [`guardrails.rs`](src/guardrails.rs) and [`squads-defi-core/src/jupiter.rs`](../../squads-defi-core/src/jupiter.rs), and returns a denial message if the condition is not met:

1. **Output mint** — The output token mint address must be in the configured `mint_allowlist`. An empty allowlist denies all mints.
2. **Slippage** — The quote slippage in basis points must not exceed `max_slippage_bps`.
3. **Price impact** — The price impact percentage must not exceed 5.0% (hardcoded in [`propose.rs`](src/propose.rs#L131)).
4. **Route hops** — The number of routing hops must not exceed 5 (hardcoded in [`propose.rs`](src/propose.rs#L132)).
5. **Notional value** — The swap value in USD must not exceed `max_notional_usd`.
6. **Daily cap** — The cumulative spent today plus this swap's notional value must not exceed `per_day_cap_usd`.

## Output

On success, the plugin returns a JSON object:

```
{
  "meta_tx_base64": "...",
  "summary": "Swap 10 SOL → ~2300 USDC. Slippage: 1%.",
  "proposal_expires_at": 1711324800,
  "proposal_address": "...",
  "status": "created"
}
```

- `meta_tx_base64` — base64-encoded unsigned versioned transaction
- `summary` — human-readable swap description
- `proposal_expires_at` — Unix timestamp when the proposal expires
- `proposal_address` — Squads proposal PDA on-chain
- `status` — always `"created"` on success

On guardrail denial, the plugin returns:

```
{
  "success": false,
  "output": "",
  "error": "Denied: Notional value $150,000 exceeds max notional $1,000"
}
```

On RPC or configuration error, the error field contains the specific error message.

## Permissions

The manifest declares `["http_client", "config_read"]`:
- `http_client` — permits outgoing HTTP requests to the Jupiter API and Solana RPC
- `config_read` — permits receiving the operator-configured values from the host

## Source

- Plugin entry point: [`src/lib.rs`](src/lib.rs)
- Proposal builder: [`src/propose.rs`](src/propose.rs)
- Guardrail checks: [`src/guardrails.rs`](src/guardrails.rs)
- Configuration parsing: [`src/config.rs`](src/config.rs)
- Error types: [`src/error.rs`](src/error.rs)
- Integration tests: [`tests/`](tests/)
