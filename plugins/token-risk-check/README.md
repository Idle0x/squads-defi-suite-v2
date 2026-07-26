# token-risk-check

Reads an SPL token mint account from a Solana JSON-RPC endpoint and returns the parsed account data — mint authority, freeze authority, Token-2022 extensions, and derived risk flags. The RPC URL is read from the host-injected configuration.

Source: [`plugins/token-risk-check/src/`](src/) ([lib.rs](src/lib.rs), [token.rs](src/token.rs))

## Installation

See [GETTING_STARTED.md](../../GETTING_STARTED.md) for full setup instructions.

## Configuration

The plugin receives its configuration through the host's `__config` injection at runtime. Add an entry to the `[[plugins.entries]]` array in `~/.zeroclaw/config.toml`:

```toml
[[plugins.entries]]
name = "token-risk-check"
config.rpc_url = "https://api.mainnet-beta.solana.com"
```

### Configuration keys

| Key | Required | Default | Description |
|-----|----------|---------|-------------|
| `rpc_url` | Yes | — | Solana JSON-RPC endpoint URL |

## Parameters

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `mint_address` | string | yes | Base58-encoded SPL token mint address |

## Output

On success, the plugin returns a text summary:

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

The risk classification is based on the following scoring in [`token.rs`](src/token.rs#L176):

| Score | Level | Criteria |
|-------|-------|----------|
| 0 | Low | Mint authority revoked or doesn't exist, no freeze authority, no dangerous Token-2022 extensions |
| 1-2 | Medium | Some flags active (active mint authority, transfer fee) |
| 3+ | High | Active freeze authority, permanent delegate, transfer hook, or multiple flags combined |

On error (for example, an invalid mint address or unreachable RPC):

```
Error: mint account query failed: ...
```

### Mint account fields checked

The plugin parses the SPL Token mint account layout from the bytes returned by `getAccountInfo`. The layout is documented in the [SPL Token source](https://github.com/solana-labs/solana-program-library/tree/master/token/program). The plugin reads:

- **Mint authority** — offset 0, a COption<Pubkey>. If set to Some, the authority can mint new tokens.
- **Freeze authority** — offset 46 (standard SPL Token), a COption<Pubkey>. If set to Some, the authority can freeze token accounts.
- **Token-2022 extensions** — TLV records starting at offset 82. Detected extension types:
  - `0x0001` — TransferFeeConfig
  - `0x0002` — TransferHook
  - `0x0004` — PermanentDelegate

Holder concentration is not evaluated. The on-chain holder distribution requires enumerating token accounts, which is not implemented.

## Permissions

The manifest declares `["http_client", "config_read"]`:
- `http_client` — permits outgoing HTTP requests to the Solana RPC
- `config_read` — permits receiving the RPC URL from the host

## Source

- Plugin entry point: [`src/lib.rs`](src/lib.rs)
- Risk assessment: [`src/token.rs`](src/token.rs)
- Tests: [`tests/token_test.rs`](tests/token_test.rs)
