---
name: vault-watch
description: >-
  Returns a structured summary of a Squads vault's on-chain state: pending
  proposals, token balances, and lending health factors. Suitable for
  manual queries and daily cron-scheduled briefings.
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

# vault-watch

Call this tool when the user asks about their Squads vault status, proposals, balances, or portfolio health. Suitable for scheduled triggers (ZeroClaw cron jobs).

Source: [`plugins/vault-watch/src/lib.rs`](https://github.com/Idle0x/squads-defi-suite/blob/main/plugins/vault-watch/src/lib.rs)

## Parameters

This tool accepts no user-facing parameters. The vault address and RPC URL are read from the host-injected configuration.

## Usage

### Calling the tool

Pass empty or minimal arguments:

```json
{}
```

### Handling the response

The response is a text summary:

```text
Proposals: 2 pending, 1 ready, 0 executed, 1 expiring within 24h
Balances: 45230 USDC | 124 SOL
Health: Kamino (HF=1.32)
```

Present the information to the user in a readable format. For scheduled/daily briefings, keep the output concise.

If any health factor is below 1.1, note it as actionable information. If a proposal expires within 6 hours, notify the user.

On error:

```
Error: RPC error: ...
```

The RPC endpoint may be unreachable or the vault address may not exist on-chain. Report the error to the user.

## Rules

- This tool is read-only. It cannot create proposals, move funds, or sign transactions.
- If the RPC is unreachable, report the error. Do not fabricate or estimate balance data.
- For scheduled cron usage, the output is automatically delivered to the configured channel.
