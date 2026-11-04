#!/usr/bin/env bash
# Launch Aster Qt with the accessibility tree enabled for local UI QA.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/aster-qt-target}"
export CARGO_PROFILE_DEV_DEBUG="${CARGO_PROFILE_DEV_DEBUG:-0}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
exec cargo run -q -p aster-qt
