# Aster Qt

Native Rust and Qt 6 email client for Aster Mail, built on the Aster Bridge core.

Aster Qt runs the bridge engine in process. It opens the local encrypted mail cache directly, so it does not need IMAP, SMTP, JMAP, POP3, or CardDAV loopback servers.

## Status

Early preview. It already signs in to a real Aster account, syncs mail, shows mailboxes and messages, renders message bodies, reads themes from CSS, and exposes server side mail rules and alias routing in Settings.

Not finished yet: compose and send, contacts, PGP key management, MCP server, packaging, and the full visual rule builder.

## Features

- Native Qt Quick interface with a Rust service layer.
- In process Aster Bridge core, no local protocol ports.
- Device code sign in with a device fingerprint.
- SQLCipher mail cache with full text search.
- Server side rules with regex support, nested conditions, and actions.
- Dedicated mailboxes that can be routed from an alias.
- Resizable three pane layout with a closable message reader.
- CSS token themes with separate light and dark slots.
- Central settings store in `<data>/settings.json`.
- Keyboard navigation with j and k, compose shortcut, and settings shortcut.
- Mailing list detection with a sidebar list filter.
- Thread bundling with message counts and sibling navigation.

## Architecture

```
Qt Quick QML
    |
    | cxx-qt properties and invokables
    v
aster-qt Rust controller
    |
    v
aster-client-core service layer
    |
    v
vendored aster-bridge-core and aster-crypto
    |
    v
Aster Mail API
```

The app owns one account, one database, and one process lock. Settings live in one JSON store. Themes are CSS custom property files under `<data>/themes`.

## Requirements

- Rust 1.85 or newer
- Qt 6 with Quick and QuickControls2 development files
- OpenSSL development files
- CMake and a C++ compiler

On Arch Linux:

```bash
sudo pacman -S --needed rust cmake ninja qt6-base qt6-declarative openssl
```

On Debian or Ubuntu:

```bash
sudo apt install rustc cargo cmake ninja-build qt6-base-dev qt6-declarative-dev libssl-dev
```

## Build

```bash
cargo build -p aster-qt
```

If disk space is tight, keep build output outside the source tree:

```bash
CARGO_TARGET_DIR=/tmp/aster-qt-target cargo build -p aster-qt
```

## Run

```bash
cargo run -p aster-qt
```

The repository also includes a local preview launcher and desktop entry:

- `scripts/run-aster-qt.sh`
- `packaging/com.astermail.qt.desktop`

Install the desktop entry for the current user:

```bash
sed "s|^Exec=.*|Exec=$PWD/scripts/run-aster-qt.sh|" packaging/com.astermail.qt.desktop \
  > ~/.local/share/applications/com.astermail.qt.desktop
update-desktop-database ~/.local/share/applications
```

## Sign in

A linked Aster account and a Star plan or higher are required by Aster Bridge.

Run the headless login helper after building:

```bash
cargo run -p aster-client-core --bin aster-headless login
```

Then open the app.

## Data and settings

- Data directory: `~/.local/share/com.astermail.qt`
- Settings: `~/.local/share/com.astermail.qt/settings.json`
- Themes: `~/.local/share/com.astermail.qt/themes`
- Mail cache: `~/.local/share/com.astermail.qt/bridge.db`

All application settings are edited in the Settings panel. Appearance is only changed there.

## Themes

Themes are CSS custom property sheets, not selector stylesheets. QML does not run CSS selectors. Every visual value is a token in `:root`.

Start from `themes/template.css` or use Settings > Appearance > New from template. The app watches the theme folder while the Appearance tab is open.

Theme contrast checks use Playwright against the CSS tokens:

```bash
NODE_PATH=<path to playwright node_modules> node scripts/check-theme-contrast.cjs
```

## MCP

Aster Qt hosts a dedicated MCP endpoint on 127.0.0.1 while the app is running. The endpoint details and a 0600 token file are written to `<data>/mcp.json`. Nothing is exposed on the network.

Build and run the stdio bridge:

```bash
cargo build -p aster-mcp
aster-mcp
```

The bridge reads `mcp.json` and proxies MCP JSON-RPC to the running app. Tools cover status, mailboxes, aliases, mail listing, search, reading, moving, read state, stars, trash, rules, mailbox creation, alias routing, and sync.

Permanent delete is intentionally gated. The MCP tool opens a confirmation dialog in the Aster Qt window, and deletion only proceeds after the user types `delete` and confirms. If the app is not running, MCP is unavailable.

## Local data and secrets

This repository contains the application only. Personal data stays in the local data directory.

Never commit:

- `settings.json`
- `mcp.json`
- GPG key material or `*.asc` exports
- anything from `~/.gnupg`

The repository has a pre-commit secret scan plus a GitHub Actions check. To enable the local hook:

```bash
scripts/install-git-hooks.sh
```

Run the scan manually at any time:

```bash
python3 scripts/secret_scan.py --all
```

## Security

- Mail is decrypted locally by the bridge core and stored in a SQLCipher database.
- The client does not open network listeners for IMAP, SMTP, JMAP, POP3, or CardDAV.
- Settings and cached data stay under the local data directory.
- Sign in uses a device key and the Aster device approval flow.
- Rule and alias actions are explicit user actions.
- The project is not security audited yet. Do not treat it as a hardened mail client until an audit is complete.

Report security issues privately to the repository owner.

## Vendored upstream

- Aster Bridge core, version 1.0.15, commit `0c0e7066a46758be8f0761adb67d8bc986dc0721`
- Aster Mail `aster-crypto`, commit `4d942b2298679f21386f2331d1a6335bcb4f9a1f`

See `vendor/UPSTREAM.md` for local patches and sync instructions.

## License

AGPL-3.0-or-later.

Aster Bridge core is AGPL-3.0-or-later. `aster-crypto` is AGPL-3.0-only. This project must remain AGPL when distributed.
