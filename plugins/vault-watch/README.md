# vault-watch

Returns a structured summary of a Squads vault's on-chain state: pending proposals, token balances, and lending health factors. All data is fetched through a Solana JSON-RPC endpoint.

Source: [`plugins/vault-watch/src/`](src/) ([lib.rs](src/lib.rs), [briefing.rs](src/briefing.rs), [proposals.rs](src/proposals.rs), [balances.rs](src/balances.rs), [health.rs](src/health.rs))

## Installation

See [GETTING_STARTED.md](../../GETTING_STARTED.md) for full setup instructions.

## Configuration

The plugin receives its configuration through the host's `__config` injection at runtime. Add an entry to the `[[plugins.entries]]` array in `~/.zeroclaw/config.toml`:

```toml
[[plugins.entries]]
name = "vault-watch"
config.rpc_url = "https://api.mainnet-beta.solana.com"
config.squads_vault = "GfQkesR7PGJP7etL6scmp8R1SaHLBcryCUaHehgLjmCW"
```

### Configuration keys

| Key | Required | Default | Description |
|-----|----------|---------|-------------|
| `rpc_url` | Yes | — | Solana JSON-RPC endpoint URL |
| `squads_vault` | Yes | — | Squads multisig vault address |

## Parameters

The plugin accepts no user-facing parameters. The vault address and RPC URL are read from the host-injected `__config` field.

## Output

The plugin returns a text summary structured in sections:

```
Proposals: 2 pending, 1 ready, 0 executed, 1 expiring within 24h
Balances: 45230 USDC | 124 SOL
Health: Kamino (HF=1.32)
```

- **Proposals** — counts by status (pending, ready, executed) and any proposals expiring within 24 hours
- **Balances** — per-token balances with symbol labels, pipe-separated
- **Health** — per-protocol health factors from lending positions, semicolon-separated

If the vault is unreachable or the RPC returns an error, the plugin returns:

```
Error: RPC error: ...
```

The output is limited to 200 tokens as measured by the tokenizer in [`squads-defi-core/src/shape.rs`](../../squads-defi-core/src/shape.rs).

## Permissions

The manifest declares `["http_client", "config_read"]`:
- `http_client` — permits outgoing HTTP requests to the Solana RPC
- `config_read` — permits receiving the vault address and RPC URL from the host

## Limitations

- **Balance coverage**: Only SOL and two hardcoded SPL mints (USDC, USDT) are checked. Other SPL tokens in the vault are not included in the output. The mint list is defined in [`balances.rs`](src/balances.rs#L40).
- **Proposal scan depth**: The plugin scans proposal indices 0 through 20. Vaults with more than 20 proposals may have older proposals missed. The scan range is defined in [`proposals.rs`](src/proposals.rs#L126).
- **Lending health parsing**: Kamino Obligation accounts use heuristic Decimal field extraction. MarginFi and Drift positions return errors because their account layouts require protocol-specific crates not included in this plugin. See [`health.rs`](src/health.rs).
- **Squads v4 layout**: Proposal account parsing uses borsh field offsets from the Squads v4 program. Layout changes between Squads program versions may cause parse failures. The parser is in [`proposals.rs`](src/proposals.rs#L173).

## Source

- Plugin entry point: [`src/lib.rs`](src/lib.rs)
- Briefing formatter: [`src/briefing.rs`](src/briefing.rs)
- Proposal fetching: [`src/proposals.rs`](src/proposals.rs)
- Balance fetching: [`src/balances.rs`](src/balances.rs)
- Lending health: [`src/health.rs`](src/health.rs)
- Integration tests: [`tests/`](tests/)
