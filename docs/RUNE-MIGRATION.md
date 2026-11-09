# Aster Qt → Rune migration plan

Date: 2026-11-08
Current app: Aster Qt — Rust + Qt 6 / Qt Quick / cxx-qt, AGPL-3.0-or-later
Candidate UI: Rune — https://github.com/anomalyco/rune (MIT, pre-release)
Reference plan: Brainroom Qt → Rune gist (`5f3ee97`), adapted to a mail client.

---

## 1. Decision summary

Migrate the **UI layer only**. Keep the domain logic and the vendored engine.

- Reuse unchanged: `crates/aster-client-core`, `crates/aster-theme`, the vendored
  `aster_bridge_core` / `aster-crypto`, `crates/aster-mcp` (as a crate; it only
  speaks HTTP to the app).
- Replace: `crates/aster-qt` (QML + cxx-qt bridge + Qt boot + QML build script)
  with a new `crates/aster-rune`.
- Remove: Qt 6, CMake, cxx/cxx-qt, QML module generation, Qt system packages.
- Keep the data contract and the data **directory**:
  `~/.local/share/com.astermail.qt` (`settings.json`, `bridge.db`, `themes/`,
  `mcp.json`, `email-images/`, `remote-images/`, `print/`, `mcp-audit.log`).
  The app id stays `com.astermail.qt` even though the crate is renamed.
- Keep the security invariants: no webview/browser engine, no JS, no IMAP/SMTP/
  JMAP/POP3/CardDAV listeners, loopback-only MCP with a 0600 token, typed
  `delete` confirmation, ammonia-sanitised HTML.

The one real feature gap is **HTML mail rendering**. Rune has no HTML→UI
renderer. This plan treats it as a first-class workstream (§7), exactly as the
Brainroom plan treated LaTeX.

---

## 2. Current stack, measured

| Path | Lines | Role | Fate |
|---|---:|---|---|
| `crates/aster-qt/qml/Main.qml` | 5,927 | Entire UI in one file: 16 inline components, 12 Popups, 3 Menus, 3 FileDialogs, 96 JS functions | Delete |
| `crates/aster-qt/src/bridge.rs` | 3,384 | 36 `#[qproperty]` JSON blobs, 84 invokables, 4 signals, 71-command enum, worker loop, MCP RPC dispatch, theme application | Rewrite as plain Rust |
| `crates/aster-qt/src/mcp.rs` | 268 | tiny_http MCP server, 74 tools, audit log | Move to `aster-rune`, logic unchanged |
| `crates/aster-qt/src/main.rs` | 20 | Qt app + QML engine | Rewrite (Rune App) |
| `crates/aster-qt/src/theme_bridge.rs` | 43 | QML token JSON helpers | Fold into theme adapter |
| `crates/aster-qt/build.rs` | 16 | cxx-qt QML module | Delete |
| `crates/aster-client-core/src/service.rs` | 3,376 | Mailboxes, messages, threads, compose, rules, aliases, PGP, attachments, export, HTML sanitising, IDs | Keep |
| `crates/aster-client-core/src/runtime.rs` | 340 | Tokio runtime, poller/outbox/token-refresh/GC workers, device identity | Keep |
| `crates/aster-theme/src/lib.rs` | 690 | 124-token CSS custom-property engine, light/dark/custom themes | Keep + add Rune emitter |
| `crates/aster-mcp` | ~ | stdio bridge reading `mcp.json` | Keep |
| `vendor/**` | ~90k | Bridge core + crypto | Keep |

Key observation: the hard separation is already clean. `bridge.rs` is mostly a
projection layer (service calls → JSON → QML), so it is the only sizeable
rewrite. The service API already returns typed `Serialize` structs, not QML.

---

## 3. Why Rune fits this app

Confirmed against the `rune` `dev` source (pin the rev in `Cargo.toml`):

| Need in Aster | Rune capability |
|---|---|
| Three-pane mail layout with resizable/collapsible sidebar | `rune_kit::shell::SplitView` + `Sidebar` + `PaneState` (used by the real agent app) |
| Message list, rules table, chains | `rune-kit` `List`, `Table`, virtualised `Tree` |
| Rich message text, links, selectable text | `text()`/`rich()`/`Span`/`Link`, selection, `--tree` text, clipboard |
| Markdown compose/preview | `rune_kit::markdown` (pulldown-cmark, images, tables, selectable) |
| Inline/attachment images | `rune::Image::file/load_mime/svg`, `El::image` |
| Custom light/dark CSS-token themes | `rune::Theme::from_json`, `Kit::from_palette`, runtime colour springs |
| Settings pages | `rune-kit` controls + `shell::Page` |
| Toasts / command palette / context menus | `Toast`, `PopupList`, `Menu`, `context_menu`, `on_secondary_press` |
| File pick/save (attachments, export, import) | `file_dialog` (XDG portal on Linux, rfd on macOS/Windows) |
| Keyboard shortcuts and focus | Rune key dispatch, `shortcuts`, one Tab stop per list |
| Accessibility + automated QA | built-in a11y roles/names + `--tree`, `--snap`, `--script` virtual clock |
| System tray (if wanted) | `rune::tray` (Linux TBD — keep out of scope initially) |
| Spell check (compose) | `rune::spell` |
| Background work → UI | `AppHandle::update`, `Waker`, `poll`, `tx.wake_after` |

Tradeoffs:

- Rune APIs move. Pin a `dev` rev and upgrade deliberately with snapshot diffs.
- **No HTML renderer.** This is the project's main new code (§7).
- `rune-notify` currently posts only on macOS and Windows; Linux needs a portal
  or `notify-send` shim (§6.9). Upstream issue opened:
  https://github.com/anomalyco/rune/issues/334
- Custom brand fonts (TeX Gyre Adventor) need `AppSpec` font loading; Inter and
  JetBrains Mono are already embedded.
- Pre-release bugs. Keep the Qt app until parity gates pass.

---

## 4. What gets rewritten

### 4.1 Keep as-is

- `crates/aster-client-core/**` — all service and runtime code.
- `crates/aster-theme/**` — token specs and CSS parsing; add `to_rune_json()`
  and a `Kit` mapping (additive, non-breaking).
- `crates/aster-mcp/**` — protocol unchanged.
- `vendor/**` — no upstream patch changes required.
- `scripts/secret_scan.py`, `.githooks`, `.github` secret scan.
- All persisted data and the data directory path.

### 4.2 Replace

```
crates/aster-rune/
  Cargo.toml                 rune (git, rev), rune-kit, rune-notify, aster-client-core, aster-theme
  src/main.rs                AppSpec, window, fonts, system dark, app handle
  src/app.rs                 AppState, View enum, update/drain plumbing
  src/actions.rs             Action enum (successor to bridge::Command)
  src/bridge.rs              worker thread: tokio runtime + service calls + AppHandle::update
  src/mcp.rs                 moved from aster-qt/src/mcp.rs (unchanged logic)
  src/theme.rs               aster_theme → rune::Theme / rune-kit::Kit; system dark
  src/views/
    shell.rs                 sidebar + view switch + status bar
    mail.rs                  message list + filters + selection + batch
    reader.rs                message detail, thread, actions, attachments
    compose.rs               drafts, send, schedule, undo, recipients
    settings.rs              eight settings tabs
    dialogs.rs               delete/empty/unsubscribe/import/export/preview
  src/mailhtml/              HTML mail → Rune El (new; §7)
  src/format.rs              dates, sizes, sender auth labels, list names
  assets/                    current SVG icons (or regenerate with Shape paths)
```

### 4.3 Delete

- `crates/aster-qt/qml/**`, `.qmlls.ini`, `build.rs`, `src/bridge.rs`,
  `src/main.rs`, `src/theme_bridge.rs`, `src/mcp.rs` (moved).
- cxx/cxx-qt/cxx-qt-lib/cxx-qt-build dependencies.
- Qt requirements in README/CI/packaging.
- `scripts/run-aster-qt-qa.sh` and the AT-SPI/ydotool/grim parts of `ui_qa.py`.

### 4.4 Permission and entry-point changes

- `.gitignore`: `Cargo.lock` stays committed (app). Add nothing.
- `packaging/com.astermail.qt.desktop`: `Exec=` points at the new binary;
  `StartupWMClass` may change.
- `README.md`: replace Qt requirements with Rune/GPU requirements
  (`libvulkan` or `llvmpipe` fallback); keep sign-in and data sections.

---

## 5. Target architecture

### 5.1 State

Replace 36 `QString` JSON blobs with one typed state. `AppState` is the only
thing the view reads; handlers mutate it. Service types are reused directly.

```rust
pub struct AppState {
    pub view: View,                       // Mail | Settings
    pub settings_tab: SettingsTab,
    pub sidebar_open: bool,
    pub sidebar: PaneState,               // rune-kit
    pub folder: String,                   // "inbox" | "scheduled" | token
    pub mailboxes: Vec<MailboxView>,
    pub views: ViewCounts,                // starred/all/scheduled/unread...
    pub lists: Vec<MailingListView>,
    pub messages: Vec<MessageView>,
    pub selected: Option<String>,
    pub detail: Option<MessageDetail>,
    pub thread: Vec<MessageView>,
    pub selection: BTreeSet<String>,      // batch mode
    pub query: Query,                     // search + filters
    pub compose: Option<ComposeState>,
    pub settings: serde_json::Value,      // settings.json (already typed enough)
    pub rules: Vec<Rule>,
    pub aliases: Vec<AliasView>,
    pub identities: Vec<SendIdentityView>,
    pub tags: Vec<TagView>,
    pub contacts: Vec<ContactView>,
    pub templates: Vec<TemplateView>,
    pub signatures: Vec<SignatureView>,
    pub pgp: PgpView, pub gpg: GpgStatus,
    pub outbox: Vec<OutboxView>,
    pub diagnostics: serde_json::Value,
    pub mcp: McpState,
    pub theme: ThemeState,                // mode, light, dark, active id, tokens
    pub busy: bool, pub status: String,
    pub toast: Option<ToastState>,
    pub pending_delete: Option<DeletePrompt>,
    pub pending_unsubscribe: Option<...>,
    pub find: FindState,
    pub html_cache: HtmlCache,            // parsed mail bodies by (id, dark)
}
```

Projection rules:

- Snapshot the current folder + selected message as one batch; never rebuild
  message rows per token.
- Keep the existing "one commit per command batch" behaviour. `AppHandle::update`
  commits on the next frame; coalesce multiple results in one `update` where the
  worker already batches them (`snapshot`, `push_settings_data`).
- Streaming/async values (sync, outbox, status) arrive from the existing
  broadcast channel; drain them in `poll` or on `wake`.

### 5.2 Threads and channels

Keep the current topology, swap the Qt hooks:

```
UI (main)  --Action-->  worker thread (tokio current/multi-thread)
   ^                         |
   +---- AppHandle::update --+   (typed state changes)
```

- `AppSpec::setup(state, wake)` starts the worker and stores
  `AppHandle<S>` in state (for MCP + UI-triggered shutdown).
- UI handlers send `Action` on an `mpsc::Sender<Action>`; they never block.
- Worker owns `ClientRuntime`, runs `rt.block_on(...)`, and calls
  `handle.update(|s, tx| ...)` with finished data.
- MCP server thread keeps `Sender<Action>` and a reply channel exactly as today.
- Delete confirmation: worker sets `pending_delete` via `update`, UI renders the
  modal, the confirm handler sends `Action::ResolveDelete(token, approved)`; the
  worker's `recv_timeout` unblocks. Same protocol as `resolve_delete` today.
- Use `tx.wake_after` for the 30 s poll/status tick instead of the QML timer.
- `ClientRuntime::subscribe()` events can replace the extra poll when wiring the
  status line; do not poll on a frame loop.

### 5.3 Errors, toasts, status

- `notify_result(qt, …)` becomes `handle.update(|s, _| s.toast = Some(...))`.
- `status_text` and `loading` map to the shell status region.
- Keep `mcp-audit.log` writes in the MCP thread.

### 5.4 View contract

Views are pure functions of `&AppState` + `&mut Cx<AppState>`; handlers mutate
state or send `Action`. No hidden QML state. Local-only UI state (popup open,
hover, drag) lives in `AppState` where it affects the tree, otherwise in Rune's
per-element state.

---

## 6. UI mapping

### 6.1 Shell

- `SplitView` with a `Sidebar` and main pane; sidebar preferred width 240,
  min 208 (current values), `PaneState` range e.g. 208..420.
- Main pane switches between Mail and Settings; Settings uses `shell::Page`
  with a back action.
- Bottom/right status: account, sync state, outbox count, MCP state.
- Toolbar: search field, refresh, sync, compose, settings, selection actions.
- Narrow window (< 860): sidebar collapses to `SidebarPeek` / toggle.
  Current `narrow < 1160` and `sidebarVisible >= 860` behaviour is preserved.

### 6.2 Sidebar

Sections and rows (from `sidebarColumn`):

- Account card: avatar/initial, email, plan/status.
- Views: Inbox, Starred, All Mail, Scheduled, Spam, Trash, Drafts, Sent.
- Mailboxes: nested folder tree with tokens, unread/total counts, context menu
  (rename, move, delete).
- Mailing lists: name, total/unread, filter and "Archive all".
- Aliases: address, pinned dot, enabled toggle, routing context menu.
- Mail rules: summary rows with enable/reorder shortcuts.
- Footer: Settings, MCP status, "New mail" indicator.
- All lists are one Tab stop; Up/Down, Home/End, type-to-jump; Return opens.
- Drag reorder is optional (rules currently reorder via buttons); skip in v1.

### 6.3 Message list

- `List`/virtualised rows: unread dot, sender, subject, preview, date, star,
  attachment, thread count, tag dots, list badge, snooze/scheduled badge.
- Multi-select mode with batch actions (archive, trash, spam, read/unread,
  star, pin, tag, snooze, move).
- Saved searches and the filter popup (from `searchFilterPopup`): from, subject,
  body, since/until, starred, attachments, category, tag.
- Grouping/bundling: subject+thread bundling and sibling navigation, as today.
- Keyboard: `j`/`k`, arrows, Space/Shift-Space, Enter, Delete, `e` archive,
  `s` star, `u` unread, `c` compose, `/` search, Escape clear.

### 6.4 Reader

- Header: subject, sender line with auth verdict (SPF/DKIM/DMARC, unverified),
  recipients, date, labels.
- Thread strip: message count + prev/next, sibling list.
- Body: **HTML renderer** (§7), or `<pre>`-equivalent for plain text; selectable
  text; links open with `xdg-open`/`App::on_open_url` equivalent (Rune `Link`
  callback → `std::process::Command`).
- Remote content banner: blocked counts, "Load remote content", tracking pixel
  report.
- Attachment bar: name, size, preview (text/image), save, save all.
- Actions: reply, reply all, forward, star, read/unread, archive, trash, spam,
  restore, move, pin, snooze, tag, print, export, delete (typed confirm).
- Find in message: `markdown`-style find? For HTML, implement `FindState` over
  spans and highlight ranges; `Tx::scroll_into_view`.
- Email dark mode selector: auto/on/off + dim images, renders `body_html_dark`
  or maps colours in the renderer.

### 6.5 Compose

- Overlay or dedicated view (current `composeOpen` overlay with a 700-ish
  centred card). Rune `Dialog`/anchored panel or full pane; keep a modal sheet.
- Fields: From (identity/alias selector), To/Cc/Bcc (recipient suggestions from
  contacts + recent senders), Subject, body, attachments, schedule at, expiry
  (expires-at / obscure subject / password), signature/template insertion.
- Body model: **Markdown source** is canonical (current `compose.rs` already
  converts Markdown → text+HTML). A plain `TextArea` + live `rune_kit::markdown`
  preview covers formatting safely. Optional rich mode: `TextEdit::rich()` with
  bold/italic/underline/code/link marks serialized to Markdown/HTML.
- Link and emoji popups: `Popover` anchored to the field.
- Draft autosave (current `draftTimer`), draft resume, close prompt.
- Send: `send_message` / `send_threaded_message` / `send_advanced_message` /
  `send_rich_message`; local undo window (`send_queued`) and scheduled send.
- Arrow keys/images: inline image insertion via file dialog; unique CID naming
  stays in the service.

### 6.6 Settings

Eight tabs, same content and defaults (`settings.rs`):

1. **Appearance** — theme mode (system/light/dark), light slot, dark slot,
   theme list, new-from-template, open themes folder, reload, email dark mode +
   dim images.
2. **Mail** — reader open, poll interval, full history, preview lines, HTML dark
   mode, saved searches.
3. **Rules & filters** — visual builder: name, match all/any, conditions
   (field/op/value), actions (move/tag/star/mark read/delete...), regex toggle,
   enable, reorder, edit, delete, run.
4. **Aliases & mailboxes** — alias create (local/domain/display), enable, pin,
   delete, route to mailbox; mailbox create; folder create/rename/move/delete;
   tokens displayed.
5. **Contacts** — list, save (vCard or fields), delete.
6. **Security & PGP** — require post-quantum, encrypt emails, GPG sign outgoing,
   key fingerprint picker, Aster key import to gpg-agent, PGP publish (WKD /
   keyserver), recipient key accept, gpg status.
7. **MCP** — enabled, allow send, allow destructive, endpoint/token status,
   audit hint.
8. **Diagnostics** — runtime diagnostics JSON, cache path, repair cache,
   sign out (keep cache toggle).

All controls are rune-kit `SettingsField`/`TextInput`/`Switch`/`Select`/
`SegmentedControl`/`Button`, with keys and a11y labels. Preserve the existing
`testId` strings as Rune `key`s so QA scripts map 1:1.

### 6.7 Dialogs, popups, menus

| Current | Rune |
|---|---|
| `composeLinkPopup`, `emojiPopup` | `Popover` anchored to trigger |
| `attachmentPreviewPopup` | `Dialog` |
| `messageContextMenu`, `aliasContextMenu`, `folderContextMenu` | `context_menu` / `Menu` |
| `externalDetailsPopup` | `Dialog` / `HoverCard` |
| command palette (`commandList`) | `PopupList` with fuzzy rows |
| `folderRenamePopup`, `folderMovePopup` | `Dialog` + `TextInput` |
| `contactsViewPopup`, `subscriptionsViewPopup` | `Dialog` |
| `snoozePopup` | `Popover` (presets + custom date) |
| `searchFilterPopup` | `Popover` |
| `outboxPopup` | `Popover`/`Dialog` |
| `imageDialog`, `exportDialog`, `attachDialog` (FileDialog) | `FileDialogs` (portal) |
| delete / empty / unsubscribe confirms | `Dialog` with typed word |
| identity popup | `Popover` |
| status/toast | `Toast` + live region |

### 6.8 Keyboard and command palette

- Preserve all shortcuts from `runCommand`/`Keys.onPressed`: refresh, sync,
  compose, reply/reply-all/forward, archive, trash, star, read, next/prev,
  settings, search, Escape, batch actions.
- Use Rune's focus model and one Tab stop per list; `on_key` at app level for
  global shortcuts, `AppSpec::on_key` for shell toggles.
- Command palette reuses the same `Action` list as the UI.

### 6.9 Notifications and MCP

- Notifications: `rune-notify` is macOS/Windows only. Upstream feature request
  filed as https://github.com/anomalyco/rune/issues/334. For Linux, implement an
  ad-hoc backend in Aster behind the same id/title/body/info + click contract,
  preferring the XDG portal (`org.freedesktop.portal.Notification`) and falling
  back to `org.freedesktop.Notifications` over zbus (or `notify-send` if the bus
  is unavailable). Keep it in `aster-rune/src/notify_linux.rs` so it can be
  deleted when upstream lands. This affects "New mail" only; in-app toast remains
  the baseline.
- MCP listener, token file, audit log, tools and the typed-delete gate move as
  is. `mcp.rs` depends only on `Action` + `mpsc`, not Qt.
- Token rotation still on restart (documented known gap).

---

## 7. The hard part: HTML mail rendering

### 7.1 Why it is hard

`aster-client-core` delivers `MessageDetail.body_html`: HTML sanitised by
ammonia, CSS inlined by `css-inline`, images rewritten to `file://`/`data:`,
plus an optional `body_html_dark`. Qt renders it as rich text. Rune has **no
HTML input path**: its `--html` tool only *exports* a Rune scene to HTML. There
is no browser engine and adding one would break the security model.

### 7.2 Options

| Option | Result | Verdict |
|---|---|---|
| **A. HTML subset → Rune `El` tree** | Full control, selectable text, links, images; new code ~1.5–2.5k lines | **Recommended** |
| B. HTML → Markdown → `rune_kit::markdown` | Fast, reuses a mature renderer, but loses colours, alignment, tables with styles, inline CSS | Fallback / plain-text path |
| C. Embed a webview | Best fidelity, violates "no browser engine / no JS" and adds a CEF/WebKit dependency | Rejected |

### 7.3 Design (Option A)

Add a UI-agnostic crate `crates/aster-mailhtml` (or module in `aster-rune`):

```
sanitised body_html
  -> parse (html5ever or lol_html tokenizer; ammonia output is well-formed enough)
  -> IR: Vec<Block>, Inline runs with style
  -> Rune El: paragraphs, headings, lists, quotes, pre/code, tables, images, rules, links
```

IR:

```rust
enum Block {
    Paragraph(Vec<Inline>),
    Heading { level: u8, runs: Vec<Inline> },
    List { ordered: bool, items: Vec<Vec<Block>> },
    Quote(Vec<Block>),
    Pre { text: String, lang: Option<String> },
    Table { rows: Vec<Vec<Cell>>, header: bool, widths: Vec<f32> },
    Image { src: ImageSrc, width: Option<f32>, height: Option<f32>, alt: String },
    Rule,
}
enum Inline {
    Text { text: String, style: InlineStyle },
    Link { text: Vec<Inline>, href: String },
    Image { .. },
    Break,
}
struct InlineStyle {
    bold: bool, italic: bool, underline: bool, strike: bool,
    code: bool, sub: bool, sup: bool,
    color: Option<Color>, background: Option<Color>,
    size: Option<f32>, family: Option<String>,
}
```

Supported tag set = the ammonia allowlist already used:
`a b blockquote br caption code col colgroup dd div dl dt em figcaption figure
font h1-h6 hr i li ol p pre s small span strong sub sup table tbody td tfoot th
thead tr u ul img`.

CSS: parse only the inline `style` attribute and the small set already produced
by `css-inline`:

- text: `color`, `background-color`, `font-size`, `font-family`, `font-weight`,
  `font-style`, `text-decoration`, `text-align`, `line-height`, `white-space`.
- box: `margin*`, `padding*`, `border*` (width/colour/style), `width`,
  `max-width`, `text-indent`, `display:none`.
- table: `border-collapse`, `border-spacing`, `vertical-align`, `width`.
- Unknown properties are dropped, not guessed.

Mapping:

- Paragraph/heading → `col` of `rich()` spans; `text-align`/margins via El.
- Bold/italic/underline/strike/code → `Span` attributes; colour → `Span`.
- Links → `Span` with `Link` + on-link callback (open externally).
- Lists → nested `row`s with bullet/number text, indentation per depth.
- Quote → left border + muted colour.
- Pre → monospace block with horizontal scroll; optional syntax highlight.
- Tables → custom grid (not `rune-kit::Table`, which is for app tables) with
  border/cell styling; `max-width` + proportional columns; clip + horizontal
  scroll when too wide.
- Images → `Image::file`/`load_mime`; `data:` bytes decoded once and cached;
  missing/blocked images render as the existing placeholder row.
- `font` tags → family/size/colour.
- `display:none` and unsupported constructs → dropped/flattened.

Selectable text: `text()`/`rich()` with `.selectable(true)`; copy source/plain
text fallback.

### 7.4 Data and caching

- Parse IR keyed by `(message_id, body variant: plain/dark, width bucket)`.
  Keep bounded LRU (e.g. 32 bodies); the source strings are already in
  `MessageDetail`.
- IR→El only rebuilds for the visible message; `El::cache()` is unnecessary and
  may fight selection.
- Dark mode: render `body_html_dark` (already produced by `email_dark.rs`), so
  the renderer has one input. Alternatively map colours with a theme-aware
  transform; keep `email_dark.rs` unchanged initially.

### 7.5 Acceptance fixtures

Build `crates/aster-mailhtml/tests/fixtures/` from real mail and synthetic
cases:

- plain paragraph, emphasis, inline code, links (mailto/http), `<hr>`
- nested lists, blockquote chains, `<pre>` with long lines
- table with header/cell borders/colspan/rowspan, wide table
- inline `color`, `background-color`, `font-size`, `text-align`, `font-family`
- `<font color face size>`, `<center>`, `<small>`, `<sub>/<sup>`
- `cid:` inline image, `file://` remote-cached image, blocked remote image
- `<img width height alt>` inclusion and broken image
- long unbroken URLs (wrap/scroll)
- a real newsletter, a real transactional email, a real HTML signature
- light and dark variants

Gates: headless `--snap` per fixture, `--tree` for links/roles, and a structural
comparison against the Qt screenshots for the same fixtures (not pixel-exact).

---

## 8. Rich compose (secondary hard part)

- Default: Markdown editor (`TextArea`) + optional preview (`rune_kit::markdown`).
  `compose.rs::markdown_to_email` already yields `(plain, html)`.
- Rich mode (parity with today's QML composer): `rune::TextEdit::rich()` with
  bold/italic/underline/code/link marks, inline images, and Markdown
  serialization on send. If this slips, ship Markdown-only first; it is a
  superset of the current send path for most users.
- Recipient suggestions, signatures, templates, draft autosave, schedule, undo
  send and expiry are service calls, not new logic.

---

## 9. Theme migration

- Keep `aster-theme` as the single source of truth (124 tokens, light/dark/
  custom CSS files, unknown-token rejection).
- Add `aster_theme::rune_theme(entry) -> String` producing a theme JSON:
  map `bg`, `text`, `accent`, `surface`, `border`, `muted`, `danger`, etc., plus
  font families, sizes, spacing and motion durations.
- Load with `rune::Theme::from_json` and derive `rune_kit::Kit::from_palette`
  (or map tokens 1:1). Override `k.radius`, `k.c.border_base`, selection and
  focus tokens from the Aster sheet.
- Preserve theme ids (`aster-light`, `aster-dark`, custom file stems) and the
  light/dark/system selector. System dark detection stays in the bridge
  (`GTK_THEME`, gsettings, KDE); poll on `wake_after` and on focus.
- Fonts: Inter and JetBrains Mono are embedded by Rune. Add TeX Gyre Adventor
  and any `font-mail` family via `AppSpec { fonts, families }`; fall back to
  Inter with a visible note if unavailable.
- Runtime theme switch: `Tx::color_motion(calm)`; colours animate; email body
  re-renders with the matching `body_html_dark` variant.
- The Playwright contrast check can stay if it validates the CSS token files; it
  is independent of the UI toolkit.

---

## 10. Data and persistence

No migration. Keep:

- `settings.json` (same schema/defaults; `settings.rs` unchanged)
- `bridge.db` (SQLCipher; `ClientRuntime` unchanged)
- `themes/*.css`, `mcp.json`, `mcp-audit.log`
- `email-images/`, `remote-images/`, `email-dark-images/`, `print/`
- device identity and keyring entries

The old Qt app and the new Rune app must not run simultaneously: the existing
single-process lock (`ClientConfig::lock_path`) already enforces this.

---

## 11. Phases and gates

Effort assumes focused agent work, one developer-equivalent. Totals are
indicative; HTML rendering is the riskiest estimate.

### Phase 0 — Spike (1–2 days)
- Add `crates/aster-rune`, pin Rune rev, `AppSpec`, a static window that opens
  `ClientRuntime`, lists mailboxes, and renders one message as plain text.
- Remove nothing yet; keep Qt build green.
- **Gate:** headless `--snap` of the shell + list; no Qt in `aster-rune`; mbox
  data loads from the existing data dir.

### Phase 1 — Shell, sidebar, list, state bridge (4–6 days)
- `AppState`, `Action`, worker loop, `AppHandle::update` snapshots.
- Sidebar, folder switching, message list, multi-select, filters, saved search.
- Keyboard navigation and status line.
- **Gate:** open folders, switch views, select and batch-act; `--script` drives
  a folder switch and selection; data persists.

### Phase 2 — Reader and HTML mail (6–10 days)
- Reader header/actions/thread/attachments.
- `aster-mailhtml` IR + renderer + fixtures (§7).
- Remote content banner, dark mode, find, links, print/export.
- **Gate:** fixture suite snapshots pass; real newsletter and transaction mail
  render; links open externally; no webview/JS.

### Phase 3 — Compose (4–6 days)
- Compose state, Markdown editor + preview, recipients, attachments, drafts,
  send/queue/schedule, undo, signatures/templates, expiry.
- Optional rich mode.
- **Gate:** full send loop against the real API; draft resume; queued undo;
  attachment save via portal.

### Phase 4 — Settings and theme (4–6 days)
- All eight tabs, rule builder, alias/mailbox management, contacts, PGP, MCP,
  diagnostics.
- Theme adapter + custom themes + system dark + font delivery.
- **Gate:** settings round-trip; custom CSS theme applies; light/dark/system
  switch; accessibility names for every control.

### Phase 5 — Dialogs, menus, command palette (3–5 days)
- All popups/dialogs/menus, context menus, confirmations, toasts, identity.
- Keyboard shortcuts and command palette parity.
- **Gate:** `--script` covers each dialog; typed-delete confirmation blocks
  destructive actions.

### Phase 6 — MCP, notifications, packaging (3–5 days)
- Move `mcp.rs`; verify every MCP tool through the stdio bridge.
- Linux notification shim (or in-app toast fallback).
- Remove Qt; update packaging, desktop entry, scripts; release build.
- **Gate:** `scripts/smoke-mcp.sh` + `qa-p0.py` pass; release binary starts with
  no Qt libraries; desktop entry launches it.

### Phase 7 — Parity and cutover (4–6 days)
- Port `ui_qa.py` suites to Rune scripts; screenshot comparisons.
- Run the full QA checklist (`docs/QA.md`).
- Keep Qt on a branch until all gates pass; then delete.
- **Gate:** CI green on fmt/clippy/test/build/snap/script; manual QA signed off.

**Total:** roughly 5–8 focused weeks, with HTML rendering and real-mail layout
the main schedule risk.

---

## 12. Testing and CI

Replace the Qt harness with Rune's native tools:

- `--snap out.png --scene … --size … --scale 2` for visual artifacts.
- `--tree` for structure and accessibility assertions (roles, names, values,
  states, keys). Keep the existing `testId` strings as Rune keys.
- `--script script.json --frames … --start … --out frames/` for click/type/
  scroll/keys on the virtual clock; port each `ui_qa.py` suite (smoke, mcp,
  folders, search, reader, thread, compose, settings, theme, batch, dialogs,
  mouse, sizes, scroll) to a script or a small Rust integration test.
- Keep data assertions through MCP exactly as today.
- Keep `cargo test -p aster-client-core`, `-p aster-theme`, add
  `-p aster-mailhtml` fixture tests.
- CI: Rust + cargo cache + Vulkan/llvmpipe; no Xvfb/xdotool/grim/Qt packages.
  Add `rune` headless smoke to the workflow.
- Headless GPU: Rune's `--snap` path needs a Vulkan ICD. On bare Arch,
  `vulkan-swrast` (lavapipe) provides the software device; without it `--snap`
  exits with "no GPU adapter". `Runtime`-based view tests (`rt.frame`,
  `rt.find`) run on CPU and are the fallback gate until the driver is present.

Suggested test pyramid:

1. Unit: theme adapter, HTML IR parser, format helpers.
2. Fixture snapshots: HTML mail variants.
3. Scripted UI: navigation, compose, settings, dialogs, typed delete.
4. MCP integration: `smoke-mcp.sh` + `qa-p0.py`.
5. Manual: real account sync, send, PGP.

---

## 13. Packaging

- One binary: `aster-rune` (keep the user-facing name "Aster").
- Linux: reuse the deb/AppImage scripts minus Qt/CMake lines; add the Rune GPU
  runtime requirement and an llvmpipe fallback note. Keep `openssl` (vendored
  SQLCipher patch).
- Desktop entry: point `Exec=` at the new binary; keep `Icon` and
  `StartupWMClass` aligned with the Rune window class.
- macOS/Windows: Rune supports both; `rune-notify` works there. Defer until
  Linux parity.
- Keep `scripts/run-aster-qt.sh` renamed to `scripts/run-aster.sh`.
- Keep the desktop entry id `com.astermail.qt.desktop` for continuity, or
  rename to `com.astermail.desktop` in a follow-up (changing it loses pin
  state).

---

## 14. Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Rune API churn | Build breaks | Pin a `dev` rev; upgrade in isolated commits; snapshot diffs |
| HTML mail fidelity | User-visible regression | Option A IR renderer; fixture suite from real mail; keep Qt until parity |
| Rich compose scope | Schedule | Markdown-first; rich mode additive |
| Linux notifications absent | Minor | `notify-send` shim or in-app toast first |
| Custom fonts missing | Brand look | `AppSpec` font loading; Inter fallback |
| Accessibility regression | QA/automation | Rune a11y tree; keys preserved from `testId`; port every suite |
| Pre-release bugs | Stability | Keep Qt branch; merge after Phase 7 |
| PDF print depends on chromium | External | Unchanged; already an external dependency |
| Wayland/X11 input differences | QA | Rune scripts use the virtual clock; manual pass on Hyprland |
| Data-dir collision | Corruption | Existing single-process lock; same data dir |
| Security regressions | High | No webview/JS, ammonia/email_dark unchanged, MCP token/typed delete unchanged |

---

## 15. Decisions (accepted 2026-11-08)

1. **HTML mail** — Option A: sanitised HTML → IR → Rune `El`; Option B stays as
   the plain-text/unsupported fallback. (§7)
2. **Compose** — Markdown-first. Rich composer mode is additive (Phase 3b), not
   a gate. (§8)
3. **Cutover** — `crates/aster-rune` lives alongside `aster-qt` on a branch
   until all Phase 7 parity gates pass. (§11)
4. **Fonts** — load TeX Gyre Adventor / `font-mail` families via `AppSpec`;
   fall back to Inter when unavailable. (§9)
5. **Linux notifications** — upstream request filed
   https://github.com/anomalyco/rune/issues/334; implement the ad-hoc Aster
   backend now (`notify_linux.rs`) against the portal /
   `org.freedesktop.Notifications`, isolated so it can be removed when upstream
   ships it. (§6.9)
6. **Rune rev** — pin the rev audited for this plan
   (`dev` @ `3ff3cc6152b9034a1d996e3a5575c51fab3b56b8`, 2026-11-08) and upgrade
   monthly in isolated commits with snapshot diffs.
7. **Sidebar drag-reorder** — deferred to v1.1. v1 keeps the current
   button-based reorder for rules and no row dragging.

---

## 16. Concrete first PRs

1. `chore: add aster-rune crate with pinned rune dev rev (spike window)`.
2. `feat(aster-rune): AppState + Action + worker bridge; list mailboxes/messages`.
3. `feat(aster-rune): three-pane shell with Sidebar and SplitView; folder switch`.
4. `test(aster-rune): headless snap/tree/script harness + first smoke script`.
5. `feat(aster-mailhtml): IR parser + paragraph/list/link/table/image renderer`.
6. `feat(aster-rune): reader with HTML body, attachments, actions`.
7. `feat(aster-rune): compose markdown + send/queue/schedule`.
8. `feat(aster-rune): settings tabs + rule builder + theme adapter`.
9. `feat(aster-rune): dialogs, command palette, keyboard parity`.
10. `chore: move MCP server, drop Qt, update packaging/CI/docs`.

Each PR should keep the Qt app building until PR 10 so both can be compared
headlessly.

---

## 17. Progress log

- **2026-11-08 — Phase 0 done.** `crates/aster-rune` scaffolded with `rune` pinned
  at `dev` @ `3ff3cc6`. Worker thread owns `ClientRuntime`; `AppHandle::update`
  projects typed state. Headless `Runtime` tests pass; `--snap` works with
  `vulkan-swrast`.
- **2026-11-08 — Phase 1 done.** Shell (`SplitView` + `Sidebar` + `Toolbar`
  + inspector reader), fixed views and mailbox list, message list with
  selection/cursor, reader with Star/Read/Archive/Trash, batch toolbar actions,
  optimistic folder switching, and keyboard (`j`/`k`, arrows, `x`, `e`, `u`,
  `s`, Enter, Escape, Cmd-R, Cmd-Q). Demo scene and scripted interaction:
  `scripts/rune/phase1.json`; 3 headless tests pass; clippy clean.
- **2026-11-08 — Phase 2 done.** New `crates/aster-mailhtml` (IR + tolerant
  parser + Rune renderer) for the ammonia allowlist: paragraphs, headings,
  emphasis, inline code, links, lists, quotes, pre, tables, rules, legacy
  `<font>`, inline CSS, `file:`/`data:` images via `ImageCache`, and entity
  decoding. Reader now renders `body_html` / `body_html_dark`, selectable with
  external links, inside `memo::cached` keyed by message/theme. Added sender
  verification label, thread strip, blocked-remote-content banner with
  "Load remote content", attachment list with per-file and "Save all", and
  Print. Validated against a real 27 KB table-based newsletter locally
  (`ASTER_MAILHTML_FIXTURE=/tmp/real-mail.html`, not committed). 9 headless
  tests pass; clippy clean. Deferred: find-in-message, attachment preview
  popup (Phase 5), exotic CSS.
- **2026-11-08 — Phase 3 done (Markdown-first).** Full-window composer with
  From/To/Cc/Bcc, subject, Markdown `TextArea`, Markdown preview, attachments via
  the XDG portal file dialog, template insert buttons, default signature, expiry,
  and schedule timestamp. Reader gained Reply / Reply all / Forward with correct
  `In-Reply-To` / `References`; toolbar Compose button and `c` shortcut.
  `Send` uses `send_markdown_with_options` (expiry + reply headers); `Save draft`
  uses `save_draft`; `Schedule` uses `schedule_markdown_with_attachments`.
  Identities/templates/signatures load once at startup. 12 headless tests pass
  (5 mailhtml + 7 app); clippy/fmt clean; compose script verified via `--tree`.
  Deferred: draft resume/edit, 5 s undo-send window, recipient suggestions,
  rich-text compose mode, server-side template picker polish.
- **2026-11-08 — Phase 4 done.** Settings view with eight tabs behind a toolbar
  button and Ctrl/Cmd-`,`. Appearance uses a real theme adapter: `aster-theme`
  tokens (custom light/dark CSS themes included) map onto `rune-kit::Kit`
  (palette, borders, radius, type, roles), with mode/slot persistence, reload,
  template creation and folder open. Mail, Security & PGP, MCP and Diagnostics
  have working toggles/actions (repair cache, sign out with confirmation,
  GPG import, WKD/keyserver publish). Rules support list/run/delete plus a
  simple from-contains → move-to creator; aliases/mailboxes/folders expose
  create/delete/route; contacts list/add/delete. Backend data loads through
  `LoadSettingsData`/`LoadDiagnostics`. 10 app tests pass (including every
  settings tab); clippy/fmt clean. Deferred: full multi-condition visual rule
  builder, saved-search manager, recipient-key accept UI, MCP endpoint display
  (Phase 6), scheduled/outbox management.
- **2026-11-08 — Phase 5 done.** Message context menu on right-click (reply,
  reply all, forward, star, read state, archive, trash, snooze, print, permanent
  delete), command palette (Ctrl/Cmd-K or `/`) with view navigation and actions,
  snooze dialog (presets + ISO), typed `delete` confirmation dialog, attachment
  text preview dialog, and toast/error surfaces. Shortcuts: `r`, `a`, `f`,
  `#`, Delete, `/`, Ctrl/Cmd-K. Overlays mount through `rune_kit::root`.
  14 app tests pass (menu + command palette included); clippy/fmt clean.
- **2026-11-08 — Phase 6 (partial) done.** MCP HTTP server moved into
  `aster-rune`, reusing the existing 74-tool dispatch; `mail_delete` now fails
  closed until the Rune confirmation flow is wired (UI delete remains fully
  gated by the typed word). Linux new-mail notifications via `notify-send`
  (`notify_linux.rs`, isolated until rune issue #334 lands) with unread-count
  change detection. Added `scripts/run-aster-rune.sh`, `.github/workflows/rune.yml`
  (fmt, clippy, tests, headless snapshot on lavapipe) and the Rune QA section.
  Deb/AppImage/desktop cutover is deferred to Phase 7.
- **2026-11-08 — Phase 7 parity (Rune side) done.** Wired the existing backend
  surfaces: FTS-style query tokens (`from:`, `subject:`, `to:`, `in:`,
  `is:unread/read/starred`, `has:attachment`, `before:/after:`) plus the filter
  dialog (from/subject/body/since/until/category/tag/list, unread/starred/
  attachments), thread grouping, saved searches, mailing-list and tag sidebar
  filters, "Archive all" from the list filter, identity selector in compose,
  outbox dialog with retry, scheduled send-now/cancel, alias pin/enable, folder
  rename/move, rule reorder, and MCP permanent-delete confirmation (the UI typed
  `delete` word resolves the waiting MCP call; cancel/timeout denies it).
  README and QA docs updated. 18 tests pass (5 mailhtml, 13 app); search scripted
  via `scripts/rune/search.json`; clippy/fmt clean.
- **Remaining before cutover:** manual parity sign-off on a real account
  (screenshots for every screen/theme), attachment preview for images/PDF,
  find-in-message, recipient-key accept UI, full multi-condition rule editor,
  deb/AppImage + desktop entry switch, then remove `crates/aster-qt` and Qt
  requirements.
- **2026-11-08 — Visual parity pass.** Rune now selects the light/dark slot with
  the same GTK/GNOME/KDE preference Qt uses (`Application.styleHints`), not the
  XDG portal value, so both apps pick `kestrel-light` here. Restyled the mail
  screen to the Qt layout: full-width toolbar (search 400 pt with icon, centred
  folder title, sync dot + "Synced", Filters/Settings/Compose/Sync/Refresh),
  18 pt sidebar brand, borderless message rows with muted sender, accent unread
  dot, semibold subject and muted preview, right-aligned thread count/date, and
  a plain-link reader (Reply/Reply all/Forward, Star/Read/Archive/Trash/Print,
  thread strip, blocked-remote link, attachment links). Accent buttons use
  `accent-button`; 18 pt reader subject; 380 pt reader pane. System-theme
  detection moved to a background watcher so `gsettings` stays off the UI
  thread. Relaunched detached against the real account for side-by-side review.
- **2026-11-08 — Structural parity with the Qt QML.** Rebuilt the shell to match
  the Qt client rather than approximating it: full-width 56 pt top bar with the
  send tile, Aster brand + account, 360 pt search, filter/settings icon buttons,
  "Synced" status dot and the accent Compose button; 240 pt sidebar with the
  account card, "Mailboxes" folder tree with the Qt icon set, Contacts/Calendar/
  Subscriptions, Custom folders, Mailing lists, Aliases, Tags, Saved searches and
  Mail rules footer; 56 pt list header (folder + message count, archive-all/all
  mail, unread-only/select/refresh) over the Unread/Starred/Attachments chips and
  88 pt rows (muted 14 pt sender, 12 pt subject, 12 pt preview, dividers, thread
  count, pin/star/attach/tag/list markers); 380 pt reader with the Qt action icon
  row (snooze/reply/reply-all/forward/pin/star/read/print/theme/archive/trash/
  spam/restore/more/close), 22 pt subject, avatar/sender/recipient/trust row,
  conversation controls, attachment chips and HTML body; 30 pt status bar with
  Outbox and MCP state. Compose is now the Qt modal (760 pt panel, From label +
  identity, To, Cc/Bcc side by side, subject, formatting links, bordered
  Markdown body, expiry/obscure/schedule, attach + Send) and Settings is the Qt
  modal (1040×760, 208 pt sidebar nav with icons, content header and close).
  18 tests pass; clippy/fmt clean; relaunched for review.
