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
- HTML mail is reduced to a Qt rich-text subset; scripts, frames, objects, and images are removed.
- No browser engine, JavaScript runtime, or remote resource loader is used to render mail.
- Remote images are removed to avoid tracking pixels and broken image placeholders.
- Links open in the system browser; no navigation happens inside the app.
- MCP binds to 127.0.0.1 only, uses a random port, and requires a per start bearer token stored in a 0600 file.
- MCP calls are written to `mcp-audit.log`.
- MCP send is disabled unless enabled in Settings.
- Permanent delete always opens a UI dialog and requires typing `delete`.
- Attachment saves use sanitized filenames under Downloads.
- The user can select an existing gpg-agent key in Settings. The selected key is used for optional outgoing PGP signing. The Aster key can also be imported into gpg-agent through a 0600 temp file. No passphrase is passed on a command line and no export file is created.

## Known gaps

- Private PGP key export is not exposed in the app or MCP yet. It requires password and TOTP re-authentication. The implemented safer path is gpg-agent import, which keeps private key operations in gpg-agent and never writes an export file.
- MCP tokens are not rotated while the app runs. They rotate on restart.
- Audit logs are local and not signed.
- No independent security audit has been performed.

## Reporting

Report security issues privately to the repository owner.
