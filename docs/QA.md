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
- Starred, All Mail, Scheduled, global tag, and alias views render.
- Reader sender-authentication summary, remote-content report, spam/restore, and attachment preview.
- Composer recipient suggestions, markdown formatting actions, emoji/link popups, and server signature/template fallback.
- Scheduled messages are created, listed, decrypted, and cancelled through the server API.

## Still to test exhaustively

- Every keyboard shortcut and focus order (Escape key injection still needs a physical-keyboard pass).
- Attachment save on large files and duplicate names.
- Offline startup, session expiry, plan failure, and revoked device.
- Rule create, delete, run, and failure paths from the visual builder.
- MCP send with approval disabled and enabled.
- Tags create/list/delete via MCP.
- Folder create/rename/move/delete via MCP.
- Snooze, unsnooze, spam, restore, and view listing via MCP.
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

## Rune UI (migration, parallel to Qt until Phase 7)

Requirements: Rust 1.99, a Vulkan ICD (`vulkan-swrast` on Arch for headless).

```sh
cargo build -p aster-rune
cargo test -p aster-rune -p aster-mailhtml
cargo clippy -p aster-rune -p aster-mailhtml --no-deps --all-targets -- -D warnings
scripts/run-aster-rune.sh
```

Headless checks (same renderer, virtual clock):

```sh
cargo run -p aster-rune -- --snap /tmp/shell.png --scene demo --size 1440x900
cargo run -p aster-rune -- --scene compose --tree /tmp/compose.json
cargo run -p aster-rune -- --scene compose --script scripts/rune/compose.json --frames 30 --tree /tmp/sent.json
ASTER_MAILHTML_FIXTURE=/tmp/real-mail.html cargo test -p aster-mailhtml
```

Scenes: `demo`, `fixture`, `compose`, `command`, `menu`, `settings`,
`settings-mail`, `settings-rules`, `settings-aliases`, `settings-contacts`,
`settings-security`, `settings-mcp`, `settings-diagnostics`.

MCP: the server writes `<data>/mcp.json` when `aster-rune` starts. Permanent
delete from MCP is blocked until the Rune confirmation flow lands; use the
message menu.

## Rune button + backend sweep (2026-11-09)

Rune tooling only: `--scene`, `--tree`, `--script`, `--snap`.

- `scripts/rune/qa_buttons.py` taps every visible button/link/menu item in every
  scene (demo, compose, command, menu, filters, snooze, delete, preview, outbox
  and all eight settings tabs). First sweep: **906 controls, 0 failures** (no
  panics/timeouts). A second pass with longer overlay settling is used to review
  controls whose tree is unchanged.
- MCP read/write checks against the live account: status, mailboxes, identities,
  mail list/search/get/get-remote/export, read/star/pin toggles, snooze/
  unsnooze, archive/trash/restore, spam/restore, attachment list/save/fetch,
  mailing lists, rules create/update/reorder/delete, contacts save/delete, tags
  create/assign/remove/delete, folders create/rename/move/delete, mailbox create/
  delete, templates/signatures create/delete, schedule/cancel, queued send/undo,
  recipient key, GPG and PGP status, PGP key info. All passed.
- Fixed a backend bug found by the sweep: archive and trash used system folder
  names as label tokens (`bulk/labels` rejects them). `move_messages` now maps
  `archive`/`trash`/`spam` to the `is_archived`/`is_trashed`/`is_spam` metadata
  flags; restore clears all three.
- Real send test: MCP `mail_send` to `e@emiliano-go.com` queued and delivered;
  the message appears in both Sent and Inbox. Test data (rules, tags, contacts,
  folders, mailbox, template, signature, scheduled and queued messages) was
  created and deleted/cancelled; MCP settings were restored.
- Toast feedback was missing from the Rune shell; validation and action results
  now render as a bottom-center toast above the status bar and expire after four
  seconds (verified by creating a rule with no condition).
- Settings/sidebar rows now expose `button` roles and stable `nav-*` keys so the
  headless script runner can target them.
