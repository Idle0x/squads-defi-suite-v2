---
name: solana-pay-request
description: >-
  Builds a Solana Pay URL for SOL or SPL token payment requests. The recipient
  address is read from the host-injected configuration and cannot be set
  through the LLM.
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

# solana-pay-request

Call this tool when the user wants to request a payment from another person. The plugin returns a `solana:` URL that the payer can open in any Solana wallet (Phantom, Backpack, Solflare).

Source: [`plugins/solana-pay-request/src/lib.rs`](https://github.com/Idle0x/squads-defi-suite/blob/main/plugins/solana-pay-request/src/lib.rs)

## Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `amount` | string | no | Payment amount in the token's smallest unit. Omit to allow the payer to enter any amount. |
| `spl_token` | string | no | SPL token mint address. Omit for SOL. |
| `label` | string | no | Short label or merchant name. |
| `message` | string | no | Description of the payment purpose. |
| `memo` | string | no | On-chain memo string. |

Known mint addresses:
- SOL: omit `spl_token`
- USDC: `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`
- USDT: `Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB`

## Usage

### Constructing the arguments

Extract from the user's request: amount (convert to smallest unit), token, and optional label/message/memo.

```json
{
  "amount": "25000000",
  "spl_token": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  "message": "Table 4 dinner bill",
  "memo": "table-4"
}
```

For SOL payment, omit `spl_token`:

```json
{
  "amount": "10000000000",
  "message": "Consultation fee"
}
```

### Handling the response

On success, the plugin returns:

```
{
  "pay_url": "solana:EPjFWdd5...?amount=25000000&recipient=GfQkes...&...",
  "summary": "Payment Request\nRecipient: GfQkes...jmCW\nAmount: 25000000",
  "qr_data": "solana:..."
}
```

The `pay_url` is a standard Solana Pay URL. The payer opens it in their wallet to see the payment details and approves it to send funds.

On error:

```
{
  "success": false,
  "error": "missing `recipient` in config"
}
```

The recipient must be configured by an operator before the plugin can be used.

## Rules

- The recipient address is enforced by the host configuration. Do not accept a recipient address from the user — the plugin ignores it.
- Convert amounts to the smallest unit (lamports for SOL, 10^decimals for SPL tokens).
- The plugin does not hold or touch funds. The payer's wallet sends funds directly to the configured recipient.
- If the user asks to send to a different address than the configured one, explain that the recipient is set in operator configuration.
