#!/usr/bin/env bash
set -euo pipefail

# Squads DeFi Suite — One-liner plugin installer
# Usage: bash install-plugin.sh <plugin-name>
#   or: curl -sSf https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh | bash -s -- <plugin-name>
#
# Supported plugins:
#   solana-pay-request     Build Solana Pay URLs with config-locked recipient
#   vault-watch            Daily Squads vault treasury briefing
#   token-risk-check       Analyze SPL token mint risks
#   swap-propose   Build guarded Jupiter swap → Squads proposals

PLUGIN="${1:-}"
REPO="https://github.com/Idle0x/squads-defi-suite-v2"
TARGET="wasm32-wasip2"

if [ -z "$PLUGIN" ]; then
  echo "Usage: $0 <plugin-name>"
  echo ""
  echo "Available plugins:"
  echo "  solana-pay-request     Build Solana Pay URLs"
  echo "  vault-watch            Daily Squads vault briefing"
  echo "  token-risk-check       Analyze SPL token risks"
  echo "  swap-propose   Build Jupiter swap → Squads proposals"
  exit 1
fi

# Validate plugin name
case "$PLUGIN" in
  solana-pay-request|vault-watch|token-risk-check|swap-propose)
    ;;
  *)
    echo "Unknown plugin: $PLUGIN"
    echo "Supported: solana-pay-request, vault-watch, token-risk-check, swap-propose"
    exit 1
    ;;
esac

echo "==> Installing $PLUGIN for ZeroClaw..."

# 1. Ensure wasm target
if ! rustup target list --installed 2>/dev/null | grep -q "$TARGET"; then
  echo "==> Installing $TARGET target..."
  rustup target add "$TARGET"
fi

# 2. Clone the plugin
BUILD_DIR=$(mktemp -d)
trap 'rm -rf "$BUILD_DIR"' EXIT
echo "==> Cloning $REPO..."
git clone --depth 1 "$REPO" "$BUILD_DIR/squads-defi-suite" 2>/dev/null

cd "$BUILD_DIR/squads-defi-suite"

# 3. Build the WASM component
echo "==> Building WASM component... (this may take a few minutes the first time)"
cargo build -p "$PLUGIN" --target "$TARGET" --release 2>&1

# 4. Install into ZeroClaw
WASM_FILE=$(find "$BUILD_DIR/squads-defi-suite/target/$TARGET/release" -name "*.wasm" -type f | head -1)
PLUGIN_DIR=$(mktemp -d)

# The WASM crate produces a file named with underscores (e.g. solana_pay_request.wasm)
PLUGIN_WASM="${PLUGIN//-/_}.wasm"
# If the found file doesn't match, look specifically for the plugin's WASM
if [ "$(basename "$WASM_FILE")" != "$PLUGIN_WASM" ]; then
  WASM_FILE="$BUILD_DIR/squads-defi-suite/target/$TARGET/release/$PLUGIN_WASM"
fi

cp "plugins/$PLUGIN/manifest.toml" "$PLUGIN_DIR/"
cp "$WASM_FILE" "$PLUGIN_DIR/"

echo "==> Installing into ZeroClaw..."
zeroclaw plugin install "$PLUGIN_DIR"

echo ""
echo "===================================="
echo "✅ $PLUGIN installed successfully!"
echo "===================================="
echo ""

# 5. Print required config keys
case "$PLUGIN" in
  solana-pay-request)
    echo "Required config:"
    echo "  zeroclaw config set plugins.entries.solana-pay-request.config.recipient <YOUR_SOLANA_ADDRESS>"
    ;;
  vault-watch)
    echo "Required config:"
    echo "  zeroclaw config set plugins.entries.vault-watch.config.rpc_url <RPC_URL>"
    echo "  zeroclaw config set plugins.entries.vault-watch.config.squads_vault <SQUADS_VAULT_ADDRESS>"
    ;;
  token-risk-check)
    echo "Required config:"
    echo "  zeroclaw config set plugins.entries.token-risk-check.config.rpc_url <RPC_URL>"
    ;;
  swap-propose)
    echo "Required config:"
    echo "  zeroclaw config set plugins.entries.swap-propose.config.rpc_url <RPC_URL>"
    echo "  zeroclaw config set plugins.entries.swap-propose.config.squads_vault <SQUADS_VAULT_ADDRESS>"
    echo "  zeroclaw config set plugins.entries.swap-propose.config.creator <AUTHORITY_PUBKEY>"
    echo "  zeroclaw config set plugins.entries.swap-propose.config.mint_allowlist <ALLOWED_MINT>"
    echo ""
    echo "Optional config:"
    echo "  zeroclaw config set plugins.entries.swap-propose.config.max_slippage_bps 50"
    echo "  zeroclaw config set plugins.entries.swap-propose.config.max_notional_usd 1000"
    echo "  zeroclaw config set plugins.entries.swap-propose.config.per_day_cap_usd 5000"
    echo "  zeroclaw config set plugins.entries.swap-propose.config.proposal_expiry_hours 72"
    echo "  zeroclaw config set plugins.entries.swap-propose.config.jupiter_url https://api.jup.ag"
    echo "  zeroclaw config set plugins.entries.swap-propose.config.squads_program_id SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"
    echo "  zeroclaw config set plugins.entries.swap-propose.config.transaction_index 0"
    ;;
esac

echo ""
echo "Then restart the daemon:"
echo "  zeroclaw daemon"
echo ""
echo "Message your agent to test it."
