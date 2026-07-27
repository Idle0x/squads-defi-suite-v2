# token-risk-check

> **Tier:** T0 (read-only) — queries on-chain mint data. No transactions built, no keys accessed.

Reads an SPL token mint account on-chain and returns a risk assessment: mint authority status, freeze authority status, Token-2022 extension detection, and an overall risk level.

- [Quick install](#quick-install)
- [Configuration](#configuration)
- [Parameters](#parameters)
- [Output format](#output-format)
- [Example](#example)
- [Security](#security)
- [Source](#source)

---

## Quick install

```bash
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh) token-risk-check
```

Or build from source: see [GETTING_STARTED.md](../../GETTING_STARTED.md#step-4-install-the-plugins).

---

## Configuration

```bash
zeroclaw config set plugins.entries.token-risk-check.config.rpc_url https://api.mainnet-beta.solana.com
```

### Configuration keys

| Key | Required | Default | Description |
|-----|----------|---------|-------------|
| `rpc_url` | Yes | — | Solana JSON-RPC endpoint. Must support `getAccountInfo` with base64 encoding. |

---

## Parameters

The LLM calls this plugin with a JSON object containing:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `mint_address` | string | yes | The SPL token mint address to analyze (base58). |

The `__config` field is injected by the host at runtime. The LLM-visible parameter schema (defined in [`src/lib.rs`](src/lib.rs)) does not include `__config`. See [ARCHITECTURE.md](../../ARCHITECTURE.md#the-__config-jail).

---

## Output format

### Success

```json
{
 "success": true,
 "output": "EPjFWdd5 — Low Risk\n Mint Authority: Revoked\n Freeze Authority: None\n Token-2022: No\n Transfer Hook: No\n Transfer Fee: No\n Permanent Delegate: No\n Standard SPL token. No mint/freeze risk detected."
}
```

### Error

```json
{
 "success": false,
 "output": "",
 "error": "Config error: missing rpc_url"
}
```

### Risk levels

| Level | Score | Meaning |
|-------|-------|---------|
| **Low** | 0–1 | Standard mint, authorities revoked or absent |
| **Medium** | 2–5 | Mint or freeze authority active, or Token-2022 extensions present |
| **High** | 6+ | Multiple risk factors active, including permanent delegate or transfer hook |

### Parsed risk factors

The plugin checks these conditions from the raw mint account data:

| Factor | Risk score | Source |
|--------|-----------|--------|
| Mint authority active | +2 | Byte 0–35 of mint account |
| Freeze authority active | +3 | Byte 36–71 of mint account |
| Holder concentration >80% | +3 | (not currently implemented — requires holder enumeration) |
| Holder concentration >50% | +1 | (not currently implemented) |
| Transfer hook present | +2 | Token-2022 extension (type code 0x01) |
| Permanent delegate present | +3 | Token-2022 extension (type code 0x02) |
| Transfer fee present | +1 | Token-2022 extension (type code 0x03) |
| No LP exists | +1 | (not currently implemented — requires DEX lookup) |

---

## Example

```
You: is EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v safe?
Agent:
 Token Risk Analysis — EPjFWdd5...
 Risk Level: Low 

 Mint Authority: Revoked
 Freeze Authority: None
 Token-2022: No
 Transfer Hook: No
 Transfer Fee: No
 Permanent Delegate: No

 Standard SPL token. No mint/freeze risk detected.
 This is the official USDC mint.
```

```
You: check this token: 4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R
Agent:
 Token Risk Analysis — 4k3Dyjz...
 Risk Level: High 

 Mint Authority: Active (can mint unlimited new tokens)
 Freeze Authority: Active
 Token-2022: Yes
 Transfer Hook: Yes
 Permanent Delegate: Yes

 Multiple risk factors detected. Use extreme caution.
```

---

## Security

### T0 custody

This plugin is read-only (T0 capsule tier):
- Queries a public Solana RPC
- Parses raw mint account bytes
- Returns a risk assessment string
- Never builds transactions, accesses keys, or signs anything

### On-chain data only

The plugin does not trust any external oracle or off-chain API. It reads the raw mint account via `getAccountInfo` and parses the binary layout directly:

1. Fetches `getAccountInfo(mint_address, base64)`
2. Decodes base64 → raw bytes
3. Parses SPL Token layout (offsets 0..165 for standard mints)
4. Parses Token-2022 TLV extensions (offsets 82+ for extension-aware mints)

The binary layout is defined in the [SPL Token program](https://github.com/solana-labs/solana-program-library/tree/master/token/program) and [Token-2022](https://spl.solana.com/token-2022) specifications.

### Config jail

The `rpc_url` comes from `__config` and cannot be modified by the LLM. An attacker cannot make the plugin query their own RPC that returns forged data.

### Permission model

The plugin declares `["http_client", "config_read"]` in its [manifest](manifest.toml):
- `http_client` — outbound HTTPS to the configured Solana RPC
- `config_read` — receives the RPC URL from the host

---

## Source

| Component | File | Description |
|-----------|------|-------------|
| Entry point | [`src/lib.rs`](src/lib.rs) | WASM shim, `execute()` implementation |
| Risk parser | [`src/token.rs`](src/token.rs) | Mint account layout parsing, risk scoring, formatting |
| Integration tests | [`tests/`](tests/) | Host-run tests |

---

## See also

- [ARCHITECTURE.md](../../ARCHITECTURE.md) — WIT contracts, `__config` jail, capsule tiers
- [GETTING_STARTED.md](../../GETTING_STARTED.md) — End-to-end setup walkthrough
- [config-template.toml](../../config-template.toml) — Complete config reference
- [SPL Token Program](https://github.com/solana-labs/solana-program-library/tree/master/token/program) — Mint account layout specification
- [Token-2022 Extensions](https://spl.solana.com/token-2022/extensions) — Extension types and TLV encoding
