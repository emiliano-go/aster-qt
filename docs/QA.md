# QA checklist

## Automated

- `cargo test -p aster-client-core`
- `cargo test -p aster-theme`
- `cargo build -p aster-qt`
- `cargo build -p aster-mcp`
- `scripts/smoke-mcp.sh`
- Playwright CSS token contrast check

## Verified manually

- Device sign in completes and real mail syncs.
- Message list renders real mail and thread bundles.
- Thread count badge and sibling pills work.
- Mailing lists appear in the sidebar and filter the list.
- HTML mail renders through WebEngine with inlined CSS.
- Reader close and reopen work.
- Settings opens, tabs switch, appearance persists.
- Custom CSS theme applies across the UI.
- Contacts save, list, and delete.
- Delete confirmation dialog appears and requires typed confirmation.
- Compose edits fields and queues a Markdown message through the outbox.
- MCP initialize, tools list, status, mail list, search, rules, contacts, PGP, lists, and stdio bridge.

## Still to test exhaustively

- Every Settings tab at every window size.
- Every keyboard shortcut and focus order.
- Attachment save on large files and duplicate names.
- Offline startup, session expiry, plan failure, and revoked device.
- Rule create, delete, run, and failure paths from the visual builder.
- MCP send with approval disabled and enabled.
- Light, dark, and custom themes across all dialogs.
