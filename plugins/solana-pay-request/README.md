# solana-pay-request

> **Tier:** T1 (transaction builder) — builds `solana:` payment URLs. Never holds keys, never signs, never broadcasts.

Builds a [Solana Pay](https://solanapay.com) transfer request URL for SOL or SPL tokens. The recipient address is locked by operator configuration — the LLM cannot redirect payments.

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
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh) solana-pay-request
```

Or build from source: see [GETTING_STARTED.md](../../GETTING_STARTED.md#step-4-install-the-plugins).

---

## Configuration

```bash
zeroclaw config set plugins.entries.solana-pay-request.config.recipient YOUR_SOLANA_ADDRESS
```

### Configuration keys

| Key | Required | Default | Description |
|-----|----------|---------|-------------|
| `recipient` | Yes | — | Solana wallet address that will receive the payment. The LLM cannot override this. |

The `recipient` is the **only** configuration key. This is by design — the plugin's sole purpose is to produce payment URLs with a guaranteed destination.

---

## Parameters

The LLM calls this plugin with a JSON object containing these fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `amount` | string | no | Amount in token units (e.g., `"0.1"` for 0.1 SOL). If omitted, the URL will not include an amount. |
| `spl_token` | string | no | SPL token mint address for token payments. If omitted, payment is in native SOL. |
| `label` | string | no | Optional label for the transaction (e.g., `"Squads Vault Deposit"`). |
| `message` | string | no | Optional message for the transaction. |
| `memo` | string | no | Optional memo for the transaction. |

The `__config` field is injected by the host at runtime. The LLM-visible parameter schema (defined in [`src/lib.rs`](src/lib.rs)) does not include `__config`. See [ARCHITECTURE.md](../../ARCHITECTURE.md#the-__config-jail).

---

## Output format

### Success

The plugin returns a `solana:` URL suitable for QR codes and wallet deep links:

```json
{
 "success": true,
 "output": "solana:7xKXmEpMUwMcK2K4mWnFN3Jsd9PkLwHb3M5A5jPpY6h?amount=0.1&label=Squads%20Vault%20Deposit&message=Payment%20of%200.1%20SOL"
}
```

The URL follows the [Solana Pay URL spec](https://github.com/solana-labs/solana-pay/blob/master/SPEC.md).

### Error

On missing recipient or invalid address:

```json
{
 "success": false,
 "output": "",
 "error": "missing `recipient` in config — operator must configure a payment destination"
}
```

---

## Example

```
You: request 0.5 USDC from Alice for invoice #412
Agent:
 Payment Request Created!

 solana:7xKXmEpMUwMcK2K4mWnFN3Jsd9PkLwHb3M5A5jPpY6h?amount=0.5&spl-token=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v&label=Invoice%20%23412&message=Payment%20of%200.5%20USDC

 Recipient: 7xKXmE...jPpY6h (config-locked — cannot be changed)
 Token: EPjFWdd5... (USDC)
 Amount: 0.5 USDC

 Scan the QR or open the link in Phantom/Solflare to pay.
```

```
You: request 1 SOL without specifying details
Agent:
 Payment Request Created!

 solana:7xKXmEpMUwMcK2K4mWnFN3Jsd9PkLwHb3M5A5jPpY6h?amount=1

 Recipient: 7xKXmE...jPpY6h (config-locked)
 Amount: 1 SOL
```

---

## Security

### Anti-redirect design

The `recipient` address comes exclusively from `__config` — the **one source of truth** the operator controls. The LLM-visible `parameters()` schema never declares a `recipient` field. Even if the LLM says "refund to this address" in prose, the plugin reads `__config.recipient` and ignores everything else.

This is the simplest and most robust anti-redirect mechanism in the suite: the plugin has **one job** and **one config value**. There is nothing to bypass.

### Custody model

This is a **T1** plugin. The agent builds payment URLs only — it never holds keys, never signs, and never broadcasts. The payment must be initiated by the recipient through their wallet.

### Permission model

The plugin declares `["config_read"]` in its [manifest](manifest.toml):
- No `http_client` needed — the URL is a plain string, no RPC calls involved
- `config_read` — receives the recipient address from the host

### Prompt injection resistance

| Attack | Mitigation |
|--------|-----------|
| LLM says "refund to attacker address" | `recipient` is hard-wired from `__config`. LLM cannot supply one. |
| LLM changes the token/amount | These are LLM-supplied parameters (visible in `parameters()` schema). Amount can be 0 or omitted. The destination is immutable. |

---

## Source

| Component | File | Description |
|-----------|------|-------------|
| Entry point | [`src/lib.rs`](src/lib.rs) | WASM shim, `execute()` implementation |
| URL builder | [`src/pay.rs`](src/pay.rs) | Solana Pay URL construction, input validation |
| Integration tests | [`tests/`](tests/) | Host-run tests |

---

## See also

- [ARCHITECTURE.md](../../ARCHITECTURE.md) — WIT contracts, `__config` jail, capsule tiers
- [GETTING_STARTED.md](../../GETTING_STARTED.md) — End-to-end setup walkthrough
- [config-template.toml](../../config-template.toml) — Complete config reference
- [Solana Pay spec](https://github.com/solana-labs/solana-pay/blob/master/SPEC.md) — Official URL format
