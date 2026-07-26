---
name: jupiter-swap-propose
description: >-
  Builds an unsigned Squads v4 multisig proposal from a Jupiter swap.
  Requires a real Jupiter quote and swap instructions fetched by the agent
  via http_request. Enforces six guardrails in Rust code.
version: "0.1.0"
author: ZeroClaw Labs
license: MIT
category: tools
tags:
  - Community
  - DeFi
  - Solana
permissions: []
---

# jupiter-swap-propose

Call this tool when the user requests a token swap through Jupiter. The plugin does not fetch the Jupiter quote itself — the agent must fetch the quote and swap instructions via http_request, then pass them to this plugin for guardrail checking and Squads proposal building.

Source: [`plugins/jupiter-swap-propose/src/lib.rs`](https://github.com/Idle0x/squads-defi-suite/blob/main/plugins/jupiter-swap-propose/src/lib.rs)
Guardrails: [`plugins/jupiter-swap-propose/src/guardrails.rs`](https://github.com/Idle0x/squads-defi-suite/blob/main/plugins/jupiter-swap-propose/src/guardrails.rs)

## Prerequisites

Before calling this tool, run token-risk-check on the output mint address. If token-risk-check returns a High risk level, inform the user and do not proceed with the swap.

## Complete workflow

The swap flow requires three sequential tool calls. Do not skip steps.

### Step 1: Fetch a Jupiter quote

Call the built-in `http_request` tool with the quote API endpoint.

```
GET https://quote-api.jup.ag/v6/quote?
    inputMint={source_mint}&
    outputMint={destination_mint}&
    amount={amount_in_smallest_unit}&
    slippageBps={max_slippage_bps}
```

Parameters to construct from the user's request:

| Parameter | Source | Format |
|-----------|--------|--------|
| `inputMint` | Source token address | Base58 mint address |
| `outputMint` | Destination token address | Base58 mint address |
| `amount` | Amount in source token's smallest unit | Integer as string (e.g., "1000000000" for 1 SOL) |
| `slippageBps` | User-specified or operator-configured default | Basis points (50 = 0.5%, 100 = 1%) |

Known mint addresses:
- SOL: `So11111111111111111111111111111111111111112`
- USDC: `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`
- USDT: `Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB`
- JitoSOL: `J1toso1uCk3QLmjYXTh8uU9kysiD8L6VZKUq7RqQ3R3`

Example request:

```
GET https://quote-api.jup.ag/v6/quote?inputMint=So11111111111111111111111111111111111111112&outputMint=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v&amount=10000000000&slippageBps=100
```

Save the full JSON response body. You will need it as `quote_json` in Step 3.

### Step 2: Fetch swap instructions

Call `http_request` with a POST to the swap-instructions endpoint. The request body is the quote response from Step 1 wrapped in a `quoteResponse` field.

```
POST https://quote-api.jup.ag/v6/swap-instructions
Content-Type: application/json

{
  "quoteResponse": { ... full quote JSON from Step 1 ... },
  "userPublicKey": "{vault_address_from_config}",
  "wrapAndUnwrapSol": true,
  "dynamicComputeUnitLimit": true
}
```

Save the full JSON response body. You will need it as `swap_instructions_json` in Step 3.

For the `userPublicKey`, use the Squads vault address. Do not use a personal wallet address — the unsigned transaction will be signed by the Squads multisig.

### Step 3: Call jupiter-swap-propose

Pass both Jupiter API responses to the plugin:

```json
{
  "quote_json": "{full JSON string from Step 1}",
  "swap_instructions_json": "{full JSON string from Step 2}",
  "daily_volume_usd": "0",
  "usd_per_unit": 23.45
}
```

- `daily_volume_usd`: Track this across calls. Start at "0" for the first swap of the day. Add the swap's notional value to it for subsequent swaps. If unknown, pass "0" — the daily cap guardrail will be undercounted, not over-counted.
- `usd_per_unit`: The USD price of one unit of the input token. If unknown, omit or set to null — the notional guardrail will be skipped for that swap.

## Handling the response

On success, the plugin returns:

```json
{
  "meta_tx_base64": "...",
  "summary": "Swap 10 SOL -> ~2300 USDC. Slippage: 1%.",
  "proposal_address": "...",
  "proposal_expires_at": 1711324800,
  "status": "created"
}
```

Tell the user to open https://app.squads.so in their wallet, find the pending proposal, review the transaction details, and approve it. The proposal expires at the Unix timestamp in `proposal_expires_at`.

On guardrail denial:

```json
{
  "success": false,
  "error": "Denied: Notional value $150,000 exceeds max notional $1,000"
}
```

Explain which guardrail triggered. The six guardrails are:
1. Output mint not in allowlist
2. Slippage exceeds configured maximum
3. Price impact exceeds 5%
4. Route hops exceed 5
5. Notional value exceeds configured maximum
6. Daily cap exceeded

Do not attempt to override guardrails. They are enforced in Rust code.

On fetch error (Step 1 or Step 2 fails):

```
Error fetching Jupiter quote: {error details}
```

Report the error to the user. Possible causes: network issue, RPC endpoint unreachable, or the requested swap pair is not available.

On config error:

```json
{
  "success": false,
  "error": "config error: missing rpc_url"
}
```

Explain that the operator needs to configure the missing key in `~/.zeroclaw/config.toml`.

## Rules

- Fetch a real Jupiter quote via http_request. Do not fabricate or estimate values.
- Run token-risk-check on the output mint before Step 1. If the risk level is High, do not proceed.
- Convert amounts to the smallest unit (lamports for SOL, 10^decimals for SPL tokens).
- If the user requests a swap with a specific slippage, validate it against reasonable bounds (0-5000 bps) before passing to the Jupiter API.
- Do not modify the source token, destination token, or amount after extracting them from the user's message.
- If the plugin returns a guardrail denial, report the specific guardrail that triggered. Do not attempt to work around the guardrails.
- The output is an unsigned transaction. The user must approve it in the Squads app. The agent cannot execute the transaction.
