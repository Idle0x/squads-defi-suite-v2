#!/usr/bin/env bash
# Package each plugin as a flat .zip for registry installation.
# Run AFTER ./scripts/build.sh
# Output: dist/<name>-<version>.zip
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

DIST="${ROOT}/dist"
mkdir -p "$DIST"

PLUGINS=(
  "plugins/jupiter-swap-propose"
  "plugins/vault-watch"
  "plugins/solana-pay-request"
  "plugins/token-risk-check"
)

echo "=============================================="
echo "  Plugin Packaging"
echo "=============================================="

for plugin in "${PLUGINS[@]}"; do
  name="$(basename "$plugin")"
  m_ver=$(grep '^version\b' "$plugin/manifest.toml" | head -1 | sed 's/.*= *"\(.*\)"/\1/')
  m_wasm=$(grep '^wasm_path\b' "$plugin/manifest.toml" | head -1 | sed 's/.*= *"\(.*\)"/\1/')
  archive="${DIST}/${name}-${m_ver}.zip"

  # Resolve the compiled wasm binary in the plugin's own workspace target
  wasm_file=$(find "$ROOT/$plugin/target" -path "*/wasm32-wasip2/release/*" -name "$m_wasm" -type f 2>/dev/null | head -1)
  if [ -z "$wasm_file" ]; then
    echo "  ❌ $name:  wasm binary '$m_wasm' not found. Run build.sh first."
    exit 1
  fi

  # Build staging dir with flat structure
  staging=$(mktemp -d)
  cp "$wasm_file"                  "$staging/$m_wasm"
  cp "$plugin/manifest.toml"       "$staging/"
  cp "$plugin/README.md"           "$staging/"
  [ -f "$plugin/SKILL.md" ] && cp "$plugin/SKILL.md" "$staging/"

  # Zip from inside staging so paths are flat
  (cd "$staging" && zip -q -X "$archive" ./* 2>/dev/null)

  sha=$(sha256sum "$archive" | cut -d' ' -f1)
  size=$(stat --format=%s "$DIST/${name}-${m_ver}.zip" 2>/dev/null || stat -f%z "$DIST/${name}-${m_ver}.zip" 2>/dev/null)

  echo "  ✅ $name $m_ver → ${archive##*/}"
  echo "     size: $(numfmt --to=iec "$size" 2>/dev/null || echo "${size}B")   sha256: ${sha:0:16}…"

  rm -rf "$staging"
done

echo ""
echo "=============================================="
echo "  Packages ready in dist/"
echo "=============================================="
ls -lh "$DIST/"
