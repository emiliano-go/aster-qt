#!/usr/bin/env sh
set -eu
cd "$(dirname "$0")/.."
git config core.hooksPath .githooks
chmod +x .githooks/pre-commit scripts/secret_scan.py
echo "Installed git hooks from .githooks"
