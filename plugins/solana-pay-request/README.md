# solana-pay-request

Builds a `solana:` URL for SOL or SPL token payment requests according to the [Solana Pay](https://solanapay.com) specification. The recipient address is read from the host-injected configuration and cannot be set through the LLM.

Source: [`plugins/solana-pay-request/src/`](src/) ([lib.rs](src/lib.rs), [pay.rs](src/pay.rs))

## Installation

See [GETTING_STARTED.md](../../GETTING_STARTED.md) for full setup instructions.

## Configuration

The plugin receives its configuration through the host's `__config` injection at runtime. Add an entry to the `[[plugins.entries]]` array in `~/.zeroclaw/config.toml`:

```toml
[[plugins.entries]]
name = "solana-pay-request"
config.recipient = "GfQkesR7PGJP7etL6scmp8R1SaHLBcryCUaHehgLjmCW"
```

### Configuration keys

| Key | Required | Default | Description |
|-----|----------|---------|-------------|
| `recipient` | Yes | — | The Solana wallet address that receives payments. This is the only address the plugin will encode. |

## Parameters

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `amount` | string | no | Payment amount in the token's smallest unit (lamports for SOL, 10^decimals for SPL). Omit to allow the payer to enter any amount. |
| `spl_token` | string | no | SPL token mint address for token payments. Omit for SOL. |
| `label` | string | no | Short label or merchant name displayed in the payer's wallet. |
| `message` | string | no | Human-readable description of the payment purpose. |
| `memo` | string | no | On-chain memo string. |

## Output

On success, the plugin returns a JSON object:

```
{
  "pay_url": "solana:EPjFWdd5...?amount=25000000&recipient=GfQkes...&...",
  "summary": "Payment Request\nRecipient: GfQkes...jmCW\nAmount: 25000000",
  "qr_data": "solana:EPjFWdd5...?amount=25000000&recipient=GfQkes...&..."
}
```

- `pay_url` — the full `solana:` URL conforming to the [Solana Pay spec](https://github.com/nicksolana/solana-pay/tree/master)
- `summary` — a human-readable description for the agent to display
- `qr_data` — identical to `pay_url`, suitable for QR code generation

The `pay_url` and `qr_data` fields contain the same value. The recipient address is truncated in the `summary` field for readability (first 8 and last 4 characters).

On error (for example, a missing recipient in config), the plugin returns:

```
{
  "success": false,
  "output": "",
  "error": "missing `recipient` in config"
}
```

## Recipient enforcement

The recipient address is read from the `__config` field injected by the ZeroClaw host. The LLM-visible parameter schema does not include a recipient field. If the host does not supply a `recipient` value in `__config`, the plugin returns an error.

## Permissions

The manifest declares `["config_read"]`:
- `config_read` — permits receiving the recipient address from the host

The plugin does not declare `http_client` because it makes no outgoing HTTP requests.

## Source

- Plugin entry point: [`src/lib.rs`](src/lib.rs)
- Pay URL builder: [`src/pay.rs`](src/pay.rs)
- Tests: [`tests/pay_test.rs`](tests/pay_test.rs)
