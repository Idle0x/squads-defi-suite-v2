---
name: token-risk-check
description: >-
  Reads an SPL token mint account from a Solana RPC endpoint and returns
  parsed account data: mint authority, freeze authority, Token-2022
  extensions, and derived risk flags.
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

# token-risk-check

Call this tool when the user wants to inspect an SPL token before interacting with it. The plugin fetches the mint account from the Solana RPC and parses its binary layout.

Source: [`plugins/token-risk-check/src/lib.rs`](https://github.com/Idle0x/squads-defi-suite/blob/main/plugins/token-risk-check/src/lib.rs)
Risk assessment: [`plugins/token-risk-check/src/token.rs`](https://github.com/Idle0x/squads-defi-suite/blob/main/plugins/token-risk-check/src/token.rs)

## Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `mint_address` | string | yes | Base58-encoded SPL token mint address |

For SOL (wrapped), use `So11111111111111111111111111111111111111112`.

Known token symbols for mint resolution:
- SOL (wrapped): `So11111111111111111111111111111111111111112`
- USDC: `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`
- USDT: `Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB`

If the user provides a token symbol you do not recognize, ask for the full mint address. Do not guess mint addresses from unknown symbols.

## Usage

### Constructing the arguments

```json
{
  "mint_address": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
}
```

If the user provides a token symbol (e.g. "USDC"), resolve it to a known mint address. If the symbol is not recognized, ask the user for the full mint address.

### Handling the response

The response is a text summary:

```
Token: EPjFWdd5... | Risk: LOW
- Mint authority: Revoked
- Freeze authority: None
- Token-2022: No
- Transfer hook: No
- Transfer fee: No
- Permanent delegate: No
Safe for general use
```

Risk levels:
- **Low** — Mint authority revoked, no freeze authority, no dangerous Token-2022 extensions.
- **Medium** — Some flags active (active mint authority, transfer fee). Review before interacting.
- **High** — Active freeze authority, permanent delegate, transfer hook, or multiple flags combined.

On error:

```
Error: mint account query failed: ...
```

Possible causes: invalid mint address, RPC unreachable, or the mint does not exist on-chain.

## Rules

- Do not guess mint addresses from symbols you do not recognize. Ask the user for the full address.
- Present the specific on-chain data that drove the risk level. Do not return the risk level alone.
- This tool is read-only. It cannot move funds or sign transactions.
