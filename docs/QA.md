# QA checklist

## Automated

- `cargo test -p aster-client-core`
- `cargo test -p aster-theme`
- `cargo build -p aster-qt`
- `cargo build -p aster-mcp`
- `scripts/smoke-mcp.sh`
- `scripts/qa-p0.py` (mutating P0 suite with cleanup)
- `scripts/ui_qa.py` (local AT-SPI + real pointer UI suite)
- Playwright CSS token contrast check

## Local UI automation

Requirements: `python-atspi`, `accerciser` (optional inspector), `ydotool` +
`ydotoold`, `grim`, and Hyprland.

Launch with the accessibility tree enabled:

```sh
scripts/run-aster-qt-qa.sh
```

Run all suites, or one at a time:

```sh
python3 scripts/ui_qa.py
python3 scripts/ui_qa.py --suite settings --suite scroll
python3 scripts/ui_qa.py --no-mouse --suite smoke
```

The harness uses AT-SPI for structure and actions, real cursor clicks via
Hyprland + `ydotool` for hit testing, `grim` for screenshots, and the app's MCP
server for data assertions. Window coordinates are calibrated from the AT-SPI
frame and `hyprctl clients`. Artifacts go to `/tmp/aster-qt-qa/<timestamp>/`;
failures include a screenshot and an accessibility-tree dump.

Suites: `smoke`, `mcp`, `folders`, `search`, `reader`, `thread`, `compose`,
`settings`, `theme`, `batch`, `dialogs`, `mouse`, `sizes`, `scroll`.

## Verified manually

- Device sign in completes and real mail syncs.
- Message list renders real mail and thread bundles.
- Thread count badge and conversation stepper work; subject-only replies bundle with their root.
- Mailing lists appear in the sidebar and filter the list.
- HTML mail renders through Qt rich text with inlined CSS; text stays selectable and links open externally.
- Reader close and reopen work.
- Settings opens, tabs switch, appearance persists.
- Settings consumes mouse-wheel events; scrolling inside does not move the mail list.
- Send-from selector stays attached to the account card, closes on outside press and on window focus loss, and has aligned rows.
- Custom CSS theme applies across the UI.
- Contacts save, list, and delete.
- Delete confirmation dialog appears and requires typed confirmation.
- Compose edits fields and queues a Markdown message through the outbox.
- MCP initialize, tools list, status, mail list, search, rules, contacts, PGP, lists, and stdio bridge.

## Still to test exhaustively

- Every keyboard shortcut and focus order (Escape key injection still needs a physical-keyboard pass).
- Attachment save on large files and duplicate names.
- Offline startup, session expiry, plan failure, and revoked device.
- Rule create, delete, run, and failure paths from the visual builder.
- MCP send with approval disabled and enabled.
- Tags create/list/delete via MCP.
- Folder create/rename/move/delete via MCP.
- Snooze and unsnooze one message via MCP.
- Compose signature/template insertion links render.
- On-demand attachment fetch via MCP.
- Pin and unpin via MCP.
- Sender action list via MCP.
- Empty folder on an empty test folder.
- Saved search controls render in the filter popup.
- Server-synced template create/list/delete.
- Server-synced signature create/list/delete.
- Draft edit path compiles and reuses the composer.
- Local undo send window and scheduled send timer are wired in the composer.
- Mailing list Archive all action is wired.
- Server templates and signatures list in Settings and are preferred by the composer.
- Aster key imported into the real system gpg-agent and listed with `gpg --list-secret-keys`.
- Light, dark, and custom themes across all dialogs.
