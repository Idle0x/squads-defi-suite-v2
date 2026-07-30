#!/usr/bin/env bash
set -euo pipefail

# Install all Squads DeFi Suite plugins at once
REPO="https://raw.githubusercontent.com/Idle0x/squads-defi-suite-v2/main/scripts/install-plugin.sh"

for plugin in token-risk-check solana-pay-request vault-watch swap-propose; do
  echo ""
  echo "═══════════════════════════════════════════"
  echo "  Installing $plugin..."
  echo "═══════════════════════════════════════════"
  bash <(curl -sSf "$REPO") "$plugin"
  echo ""
done

echo ""
echo "═══════════════════════════════════════════"
echo "  All 4 plugins installed!"
echo "═══════════════════════════════════════════"
echo ""
echo "Next: configure each plugin with your RPC and vault addresses."
echo "See: https://github.com/Idle0x/squads-defi-suite-v2#config"
