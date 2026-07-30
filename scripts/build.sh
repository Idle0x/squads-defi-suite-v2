#!/usr/bin/env bash
# Build all ZeroClaw WASM tool plugins.
# Run from project root: ./scripts/build.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PLUGIN_NAMES=(
  "swap-propose"
  "vault-watch"
  "solana-pay-request"
  "token-risk-check"
)

echo "=============================================="
echo "  ZeroClaw WASM Plugin Build"
echo "  Target: wasm32-wasip2 (release)"
echo "  Workspace: unified"
echo "=============================================="

for name in "${PLUGIN_NAMES[@]}"; do
  echo ""
  echo "--- Building $name ---"
  cargo build -p "$name" --target wasm32-wasip2 --release
done

echo ""
echo "=============================================="
echo "  Artifact Verification"
echo "=============================================="

all_ok=true
for name in "${PLUGIN_NAMES[@]}"; do
  # Convert kebab-case name to snake_case for WASM file name
  wasm_name=$(echo "$name" | tr '-' '_')
  wasm_path="${wasm_name}.wasm"
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
echo "=============================================="
echo "  Precompiled .cwasm (if wasmtime available)"
echo "=============================================="
if command -v wasmtime &>/dev/null; then
  for name in "${PLUGIN_NAMES[@]}"; do
    wasm_name=$(echo "$name" | tr '-' '_')
    wasm_path="${wasm_name}.wasm"
    found=$(find "$ROOT/target" -path "*/wasm32-wasip2/release/*" -name "$wasm_path" -type f 2>/dev/null | head -1)
    if [ -n "$found" ]; then
      cwasm_path="${found%.wasm}.cwasm"
      echo "  Compiling $name..."
      wasmtime compile "$found" -o "$cwasm_path" 2>/dev/null && \
        echo "  ✅ $name  → $(basename $cwasm_path)" || \
        echo "  ⚠️  $name  — wasmtime compile skipped (not supported for this target)"
    fi
  done
else
  echo "  wasmtime not installed — skipping .cwasm generation"
  echo "  Install with: cargo install wasmtime-cli"
fi

echo ""
if $all_ok; then
  echo "✅ All plugins built successfully."
else
  echo "❌ Some plugins failed — check output above."
  exit 1
fi
