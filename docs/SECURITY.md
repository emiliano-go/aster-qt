# Security model

## Trust boundaries

- Aster Mail servers and message content are untrusted.
- The local device, operating system keyring, and local data directory are trusted.
- MCP clients are untrusted callers with a loopback token.

## Controls in place

- Sign in uses the Aster device approval flow and a device key.
- Mail is decrypted by the bridge core and stored in a SQLCipher database.
- The app does not open IMAP, SMTP, JMAP, POP3, or CardDAV listeners.
- HTML mail is sanitized with ammonia after CSS inlining.
- HTML mail runs in WebEngine with JavaScript, plugins, local storage, remote URL access, and image auto load disabled.
- A Content Security Policy blocks scripts, frames, objects, connections, and remote images.
- Remote images are hidden to avoid tracking pixels and broken image placeholders.
- Links open in the system browser; the embedded view ignores navigation.
- MCP binds to 127.0.0.1 only, uses a random port, and requires a per start bearer token stored in a 0600 file.
- MCP calls are written to `mcp-audit.log`.
- MCP send is disabled unless enabled in Settings.
- Permanent delete always opens a UI dialog and requires typing `delete`.
- Attachment saves use sanitized filenames under Downloads.

## Known gaps

- Private PGP key export is not exposed in the app or MCP yet. It requires password and TOTP re-authentication. The preferred design is to keep private key operations in the system gpg-agent and never export the key at all.
- MCP tokens are not rotated while the app runs. They rotate on restart.
- Audit logs are local and not signed.
- No independent security audit has been performed.

## Reporting

Report security issues privately to the repository owner.
