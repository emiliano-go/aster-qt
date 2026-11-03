#!/usr/bin/env bash
# Re-sync the vendored upstream sources at pinned commits.
# Usage: scripts/sync-upstream.sh
# Requires network access and git. Review the diff before committing.
set -euo pipefail

BRIDGE_COMMIT="0c0e7066a46758be8f0761adb67d8bc986dc0721"
MAIL_COMMIT="4d942b2298679f21386f2331d1a6335bcb4f9a1f"

root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

git clone --quiet https://github.com/Aster-Privacy/Aster-Bridge "$tmp/Aster-Bridge"
git -C "$tmp/Aster-Bridge" checkout --quiet "$BRIDGE_COMMIT"
git clone --quiet https://github.com/Aster-Privacy/Aster-Mail "$tmp/Aster-Mail"
git -C "$tmp/Aster-Mail" checkout --quiet "$MAIL_COMMIT"

rm -rf "$root/vendor/Aster-Bridge/src-tauri/crates/aster_bridge_core"
mkdir -p "$root/vendor/Aster-Bridge/src-tauri/crates"
cp -a "$tmp/Aster-Bridge/src-tauri/crates/aster_bridge_core" \
  "$root/vendor/Aster-Bridge/src-tauri/crates/"
cp "$tmp/Aster-Bridge/LICENSE" "$root/vendor/Aster-Bridge/LICENSE"

rm -rf "$root/vendor/Aster-Mail/aster-crypto"
mkdir -p "$root/vendor/Aster-Mail"
cp -a "$tmp/Aster-Mail/aster-crypto" "$root/vendor/Aster-Mail/"

# Re-apply documented local patch: avoid the huge vendored OpenSSL build.
sed -i 's/bundled-sqlcipher-vendored-openssl/bundled-sqlcipher/' \
  "$root/vendor/Aster-Bridge/src-tauri/crates/aster_bridge_core/Cargo.toml"

echo "Vendored sources updated. Review vendor/UPSTREAM.md and run cargo test."
