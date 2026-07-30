# Scripts

Build, package, install, and verify tools for the Squads DeFi Suite plugins.

| Script | Purpose | When to use |
|--------|---------|-------------|
| [`build.sh`](build.sh) | Compile all plugins to `wasm32-wasip2` | Development, CI |
| [`package.sh`](package.sh) | Produce distribution zips in `dist/` | Release, registry submission |
| [`install-plugin.sh`](install-plugin.sh) | One-liner: clone, build, install a single plugin | End users |
| [`install-all.sh`](install-all.sh) | Install all four plugins in sequence | End users |

---

## `build.sh`

Compiles every plugin to a `wasm32-wasip2` WebAssembly component.

```bash
./scripts/build.sh
```

Each plugin is built as part of the unified workspace from the project root. The output `.wasm` files are placed in the root `target/wasm32-wasip2/release/` directory. Packaging scripts copy them alongside each plugin's `manifest.toml` for `zeroclaw plugin install`.

### Prerequisites

- `rustup target add wasm32-wasip2`
- Rust 1.87+

### Output

```
plugins/token-risk-check/token_risk_check.wasm
plugins/token-risk-check/manifest.toml
plugins/solana-pay-request/solana_pay_request.wasm
plugins/solana-pay-request/manifest.toml
plugins/vault-watch/vault_watch.wasm
plugins/vault-watch/manifest.toml
plugins/swap-propose/swap_propose.wasm
plugins/swap-propose/manifest.toml
```

---

## `package.sh`

Produces compressed distribution archives for each plugin in `dist/`.

```bash
./scripts/package.sh
```

Each `.zip` contains the `.wasm` binary and `manifest.toml`, ready for:
```bash
zeroclaw plugin install dist/token-risk-check-0.1.0.zip
```

Files named as `<plugin-name>-<version>.zip`.

---

## `install-plugin.sh`

One-liner installer for a single plugin. Clones the repository, builds the plugin, and installs it into ZeroClaw.

```bash
# Via curl (no clone needed):
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh) token-risk-check
```

**What it does:**
1. Ensures `wasm32-wasip2` target is installed
2. Clones the repo with `--depth 1`
3. `cd`s into the plugin directory and runs `cargo build --target wasm32-wasip2 --release`
4. Copies `.wasm` + `manifest.toml` to a temp directory
5. Runs `zeroclaw plugin install`
6. Prints the required `zeroclaw config set` commands for that plugin

**Supported plugins:** `token-risk-check`, `solana-pay-request`, `vault-watch`, `swap-propose`

---

## `install-all.sh`

Installs all four plugins in sequence using `install-plugin.sh`:

```bash
bash <(curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-all.sh)
```

This is equivalent to running `install-plugin.sh` four times, once for each plugin.

---

## See also

- [GETTING_STARTED.md](../GETTING_STARTED.md) — End-to-end setup walkthrough
- [ARCHITECTURE.md](../ARCHITECTURE.md) — Plugin build and loading lifecycle
