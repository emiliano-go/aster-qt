#!/usr/bin/env bash
# Local preview launcher for the Rune UI (Phase 6; Qt is still the default until parity).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
exec cargo run -q -p aster-rune
