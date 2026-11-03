# Vendored upstream

These sources are copied into the workspace so a clean build does not depend on
sibling checkouts outside the repository.

| Vendor path | Upstream | Commit | Date | Version |
|---|---|---|---|---|
| `vendor/Aster-Bridge/src-tauri/crates/aster_bridge_core` | `Aster-Privacy/Aster-Bridge` | `0c0e7066a46758be8f0761adb67d8bc986dc0721` | 2026-11-01 | 1.0.15 |
| `vendor/Aster-Mail/aster-crypto` | `Aster-Privacy/Aster-Mail` | `4d942b2298679f21386f2331d1a6335bcb4f9a1f` | 2026-11-02 | 0.1.0 |

Licenses:

- Aster-Bridge core: AGPL-3.0-or-later (`vendor/Aster-Bridge/LICENSE`)
- aster-crypto: AGPL-3.0-only (`vendor/Aster-Mail/aster-crypto/LICENSE`)

## Why the directory names have capitals

`vendor/Aster-Bridge/src-tauri/crates/aster_bridge_core/Cargo.toml` contains:

```toml
aster-crypto = { path = "../../../../Aster-Mail/aster-crypto" }
```

Keeping the upstream directory names makes that relative path resolve to
`vendor/Aster-Mail/aster-crypto` without editing the dependency line.

## Local patches

Document every patch here. Keep patches minimal so upstream can be re-synced.

1. `aster_bridge_core/Cargo.toml` and `crates/aster-client-core/Cargo.toml`:
   `rusqlite` features changed from `bundled-sqlcipher-vendored-openssl` to
   `bundled-sqlcipher`. The vendored OpenSSL build is huge and failed on a
   full root filesystem. This requires system OpenSSL development headers
   (`openssl` / `libssl-dev`). Switch back to the vendored feature for fully
   self-contained release builds if disk allows.
2. `crates/aster-client-core/src/runtime.rs` intentionally does not start
   `BridgeRuntime`; it starts the poller, outbox, token refresh, and GC workers
   directly so no IMAP/SMTP/JMAP/POP3/CardDAV ports are bound.
