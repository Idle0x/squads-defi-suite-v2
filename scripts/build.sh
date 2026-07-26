#!/usr/bin/env bash
# Build all ZeroClaw WASM tool plugins.
# Run from project root: ./scripts/build.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PLUGINS=(
  "plugins/jupiter-swap-propose"
  "plugins/vault-watch"
  "plugins/solana-pay-request"
  "plugins/token-risk-check"
)

echo "=============================================="
echo "  ZeroClaw WASM Plugin Build"
echo "  Target: wasm32-wasip2 (release)"
echo "=============================================="

for plugin in "${PLUGINS[@]}"; do
  name="$(basename "$plugin")"
  echo ""
  echo "--- Building $name ---"
  cargo build --package "$name" --target wasm32-wasip2 --release
done

echo ""
echo "=============================================="
echo "  Artifact Verification"
echo "=============================================="

all_ok=true
for plugin in "${PLUGINS[@]}"; do
  name="$(basename "$plugin")"
  wasm_path=$(grep '^wasm_path' "$plugin/manifest.toml" | head -1 | cut -d'"' -f2)
  found=$(find "$ROOT/target" -path "*/wasm32-wasip2/release/*" -name "$wasm_path" -type f 2>/dev/null | head -1)
  if [ -n "$found" ]; then
    size=$(stat --format=%s "$found" 2>/dev/null || stat -f%z "$found" 2>/dev/null)
    echo "  ✅ $name  → $wasm_path  ($(numfmt --to=iec "$size" 2>/dev/null || echo "${size}B"))"
  else
    echo "  ❌ $name  → $wasm_path  NOT FOUND"
    all_ok=false
  fi
done

echo ""
if $all_ok; then
  echo "✅ All plugins built successfully."
else
  echo "❌ Some plugins failed — check output above."
  exit 1
fi
