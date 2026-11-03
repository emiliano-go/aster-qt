#!/usr/bin/env bash
# Non-destructive MCP smoke test. Requires Aster Qt to be running.
set -euo pipefail
DATA="${ASTER_QT_DATA_DIR:-$HOME/.local/share/com.astermail.qt}"
CFG="$DATA/mcp.json"
if [ ! -f "$CFG" ]; then
  echo "FAIL: $CFG not found. Start Aster Qt first."
  exit 1
fi
TOKEN=$(python3 -c "import json;print(json.load(open('$CFG'))['token'])")
URL=$(python3 -c "import json;print(json.load(open('$CFG'))['url'])")
call() {
  local name="$1" args="${2:-}"
  if [ -z "$args" ]; then args='{}'; fi
  curl -sS -X POST "$URL" -H "Authorization: Bearer $TOKEN" -H 'content-type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"$name\",\"arguments\":$args}}"
}
fail=0
check() {
  local name="$1" args="${2:-}"
  if [ -z "$args" ]; then args='{}'; fi
  local out
  out=$(call "$name" "$args")
  if echo "$out" | python3 -c 'import sys,json;d=json.load(sys.stdin);sys.exit(0 if "result" in d and not d["result"].get("isError") else 1)' >/dev/null 2>&1; then
    echo "PASS $name"
  else
    echo "FAIL $name: $out"
    fail=1
  fi
}
check status
check mailboxes_list
check mail_list '{"folder":"inbox","limit":1}'
check mail_search '{"query":"a","limit":1}'
check drafts_list '{"limit":1}'
check rules_list
check contacts_list
check pgp_key_info
check pgp_status
exit "$fail"
