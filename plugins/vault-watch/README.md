# vault-watch

> **Tier:** T0 (read-only) — queries on-chain data. No transactions built, no keys accessed.

Returns a daily treasury briefing for a Squads multisig vault: pending proposals, token balances (SOL + SPL), and lending health factors across Kamino, MarginFi, and Drift protocols.

- [Quick install](#quick-install)
- [Configuration](#configuration)
- [Output format](#output-format)
- [Example](#example)
- [Security](#security)
- [Source](#source)

---

## Quick install

```bash
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh) vault-watch
```

Or build from source: see [GETTING_STARTED.md](../../GETTING_STARTED.md#step-4-install-the-plugins).

---

## Configuration

```bash
# Required
zeroclaw config set plugins.entries.vault-watch.config.rpc_url https://api.mainnet-beta.solana.com
zeroclaw config set plugins.entries.vault-watch.config.squads_vault YOUR_SQUADS_VAULT
```

### Configuration keys

| Key | Required | Default | Description |
|-----|----------|---------|-------------|
| `rpc_url` | Yes | — | Solana JSON-RPC endpoint. Used for all on-chain queries. |
| `squads_vault` | Yes | — | Squads multisig vault address to monitor. Must be a valid base58 pubkey. |

---

## Output format

The plugin returns a structured briefing:

```
 Vault Status — [vault_address]
Proposals
 Executed: 12
 Active/Voting: 1
 Draft: 0
 Expires soon: 0

Token Balances
 3.45 SOL
 $ 1,250.00 USDC (1,250,000,000 units)
 $ 0.00 BONK (0 units)

Lending Health
 Kamino: Health factor 1.32 — deposit $3,000, borrow $1,800
```

---

## Example

```
You: check my vault
Agent:
 Vault Status — GfQkesR7PGJP7etL6scmp8R1SaHLBcryCUaHehgLjmCW

 Proposals
 Executed: 12
 Active/Voting: 1 — "Swap 10 SOL → USDC" (expires in 48h)
 Draft: 0
 Expires soon: 0

 Token Balances
 3.45 SOL
 $ 1,250.00 USDC (1,250,000,000 units)

 Lending Health
 Kamino: Health factor 3.45 — deposit $10,000, borrow $3,500
 Drift: Not available (requires drift-program crate)
```

---

## Security

### T0 custody

This plugin is read-only (T0 capsule tier):
- Queries public Solana RPC endpoints
- Returns formatted on-chain data
- Never builds transactions, accesses keys, or signs anything

### Config jail

The `rpc_url` and `squads_vault` values come from `__config` and cannot be modified by the LLM. An LLM cannot redirect the plugin to query a different vault or a malicious RPC endpoint. See [ARCHITECTURE.md](../../ARCHITECTURE.md#the-__config-jail).

### On-chain data only

The plugin does not trust any external oracle. All data is parsed directly from on-chain account layouts:
- SOL balance via `getBalance`
- SPL token balances via ATA derivation + `getTokenAccountBalance`
- Squads proposals via Anchor-deserialized `Proposal` accounts
- Kamino lending positions via raw account data parsing

### Permission model

The plugin declares `["http_client", "config_read"]` in its [manifest](manifest.toml):
- `http_client` — outbound HTTPS to the configured Solana RPC
- `config_read` — receives the vault address and RPC URL from the host

---

## Source

| Component | File | Description |
|-----------|------|-------------|
| Entry point | [`src/lib.rs`](src/lib.rs) | WASM shim, briefing orchestration |
| Balance queries | [`src/balances.rs`](src/balances.rs) | SOL + SPL token balance fetching via RPC |
| Proposal parsing | [`src/proposals.rs`](src/proposals.rs) | Squads Anchor account deserialization |
| Lending health | [`src/health.rs`](src/health.rs) | Kamino obligation parsing (MarginFi / Drift: unimplemented) |
| Briefing formatting | [`src/briefing.rs`](src/briefing.rs) | Output shaping, token budgeting |
| Integration tests | [`tests/`](tests/) | Host-run tests with mocked RPC |

---

## See also

- [ARCHITECTURE.md](../../ARCHITECTURE.md) — WIT contracts, `__config` jail, capsule tiers
- [GETTING_STARTED.md](../../GETTING_STARTED.md) — End-to-end setup walkthrough
- [config-template.toml](../../config-template.toml) — Complete config reference
