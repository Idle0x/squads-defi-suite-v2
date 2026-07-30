#!/usr/bin/env bash
# Verify every manifest.toml is consistent with its Cargo.toml.
# Run from project root: ./scripts/verify.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PLUGINS=(
  "plugins/swap-propose"
  "plugins/vault-watch"
  "plugins/solana-pay-request"
  "plugins/token-risk-check"
)

PLUGIN_NAMES=(
  "swap-propose"
  "vault-watch"
  "solana-pay-request"
  "token-risk-check"
)

errors=0
warnings=0

echo "=============================================="
echo "  Plugin Manifest Verification"
echo "  Workspace: unified"
echo "=============================================="

for plugin in "${PLUGINS[@]}"; do
  name="$(basename "$plugin")"

  # ---- file existence ----
  if [ ! -f "$plugin/manifest.toml" ]; then
    echo "  ❌ $name:  manifest.toml missing"
    errors=$((errors+1))
    continue
  fi
  if [ ! -f "$plugin/Cargo.toml" ]; then
    echo "  ❌ $name:  Cargo.toml missing"
    errors=$((errors+1))
    continue
  fi

  # ---- extract values ----
  m_name=$(grep  '^name\b'         "$plugin/manifest.toml" | head -1 | sed 's/.*= *"\(.*\)"/\1/')
  m_ver=$(grep  '^version\b'       "$plugin/manifest.toml" | head -1 | sed 's/.*= *"\(.*\)"/\1/')
  m_wasm=$(grep '^wasm_path\b'     "$plugin/manifest.toml" | head -1 | sed 's/.*= *"\(.*\)"/\1/')
  m_caps=$(grep '^capabilities\b'   "$plugin/manifest.toml" | head -1)
  m_perms=$(grep '^permissions\b'   "$plugin/manifest.toml" | head -1)

  c_name=$(grep '^name\b'          "$plugin/Cargo.toml" | head -1 | sed 's/.*= *"\(.*\)"/\1/')
  c_ver=$(grep '^version\b'        "$plugin/Cargo.toml" | head -1 | sed 's/.*= *"\(.*\)"/\1/')

  # ---- cross-checks ----
  if [ "$m_name" != "$c_name" ]; then
    echo "  ❌ $name:  name mismatch  — manifest='$m_name'  cargo='$c_name'"
    errors=$((errors+1))
  fi
  if [ "$m_ver" != "$c_ver" ]; then
    echo "  ❌ $name:  version mismatch — manifest='$m_ver'  cargo='$c_ver'"
    errors=$((errors+1))
  fi
  if echo "$m_caps" | grep -qv '"tool"'; then
    echo "  ❌ $name:  capabilities must include \"tool\"  (got: $m_caps)"
    errors=$((errors+1))
  fi
  if echo "$m_perms" | grep -qv '"config_read"'; then
    echo "  ❌ $name:  permissions must include \"config_read\"  (got: $m_perms)"
    errors=$((errors+1))
  fi
  if [ -z "$m_wasm" ]; then
    echo "  ❌ $name:  wasm_path is empty or missing"
    errors=$((errors+1))
  else
    # Check the wasm file in the unified workspace target directory
    wasm_name=$(echo "$name" | tr '-' '_')
    found=$(find "$ROOT/target" -path "*/wasm32-wasip2/release/*" -name "$wasm_name.wasm" -type f 2>/dev/null | head -1)
    if [ -z "$found" ]; then
      echo "  ⚠️  $name:  wasm_path='$m_wasm' not yet built (run build.sh first)"
      warnings=$((warnings+1))
    fi
  fi

  echo "  ✅ $name  — all checks passed"
done

echo ""
echo "=============================================="
echo "  Summary:  ${errors} error(s),  ${warnings} warning(s)"
echo "=============================================="

if [ "$errors" -gt 0 ]; then
  exit 1
fi
