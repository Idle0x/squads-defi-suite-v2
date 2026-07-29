#!/usr/bin/env bash
# Sign a plugin package with Ed25519.
# Usage: ./scripts/sign.sh <package.zip> [private_key_file]
#
# If no private key file is specified, uses the PLUGIN_SIGNING_KEY
# environment variable. If neither is available, prints a warning
# and skips signing (non-fatal).
#
# Output: <package.zip>.sig (detached signature, base64-encoded)
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "Usage: $0 <package.zip> [private_key_file]"
  exit 1
fi

PACKAGE="$1"
KEY_FILE="${2:-}"

if [ ! -f "$PACKAGE" ]; then
  echo "❌ Package not found: $PACKAGE"
  exit 1
fi

# Resolve signing key
if [ -n "$KEY_FILE" ]; then
  if [ ! -f "$KEY_FILE" ]; then
    echo "❌ Key file not found: $KEY_FILE"
    exit 1
  fi
  KEY=$(cat "$KEY_FILE")
elif [ -n "${PLUGIN_SIGNING_KEY:-}" ]; then
  KEY="$PLUGIN_SIGNING_KEY"
else
  echo "  ⚠️  No signing key available — skipping signature for $(basename $PACKAGE)"
  echo "     Set PLUGIN_SIGNING_KEY env var or pass a key file path."
  exit 0
fi

# Generate Ed25519 signature using OpenSSL
SIG_FILE="${PACKAGE}.sig"
openssl dgst -sha256 -sign <(echo "$KEY" | base64 -d) -out "$SIG_FILE.tmp" "$PACKAGE" 2>/dev/null || {
  echo "  ⚠️  Signing failed — OpenSSL Ed25519 not available or key invalid"
  rm -f "$SIG_FILE.tmp"
  exit 0
}

# Base64-encode the signature for safe transport
base64 < "$SIG_FILE.tmp" > "$SIG_FILE"
rm -f "$SIG_FILE.tmp"

echo "  ✅ Signed $(basename $PACKAGE) → $(basename $SIG_FILE)"
