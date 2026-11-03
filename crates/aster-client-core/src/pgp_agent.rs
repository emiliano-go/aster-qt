// SPDX-License-Identifier: AGPL-3.0-or-later
//! Detection and import helpers for the system GnuPG agent.
//!
//! The Aster private key is imported into the user's existing gpg-agent. No
//! export file is created and no passphrase is passed on a command line.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GpgKey {
    pub fingerprint: String,
    pub uid: String,
    pub created: String,
    pub expires: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GpgStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub agent_socket: Option<String>,
    pub secret_keys: Vec<GpgKey>,
    pub error: Option<String>,
}

pub fn detect() -> GpgStatus {
    detect_with_home(None)
}

pub fn import_secret_key(armored: &str) -> Result<GpgStatus, String> {
    import_secret_key_with_home(armored, None)
}

fn detect_with_home(home: Option<&Path>) -> GpgStatus {
    let version = run_gpg(home, &["--version"])
        .ok()
        .and_then(|out| out.lines().next().map(str::to_string));
    if version.is_none() {
        return GpgStatus {
            installed: false,
            version: None,
            agent_socket: None,
            secret_keys: Vec::new(),
            error: Some("gpg was not found on PATH".into()),
        };
    }
    let agent_socket = Command::new("gpgconf")
        .args(["--list-dirs", "agent-socket"])
        .envs(home_env(home))
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|value| !value.is_empty());
    let secret_keys = run_gpg(home, &["--batch", "--with-colons", "--list-secret-keys"])
        .map(|out| parse_secret_keys(&out))
        .unwrap_or_default();
    GpgStatus {
        installed: true,
        version,
        agent_socket,
        secret_keys,
        error: None,
    }
}

fn import_secret_key_with_home(armored: &str, home: Option<&Path>) -> Result<GpgStatus, String> {
    if !armored.contains("BEGIN PGP PRIVATE KEY BLOCK") {
        return Err("not an armored OpenPGP private key".into());
    }
    let mut file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    file.write_all(armored.as_bytes())
        .map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o600));
    }
    let output = Command::new("gpg")
        .args(["--batch", "--yes", "--import"])
        .arg(file.path())
        .envs(home_env(home))
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("run gpg: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "gpg import failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(detect_with_home(home))
}

fn run_gpg(home: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let output = Command::new("gpg")
        .args(args)
        .envs(home_env(home))
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn home_env(home: Option<&Path>) -> Vec<(String, PathBuf)> {
    match home {
        Some(path) => vec![("GNUPGHOME".to_string(), path.to_path_buf())],
        None => Vec::new(),
    }
}

fn parse_secret_keys(output: &str) -> Vec<GpgKey> {
    let mut keys = Vec::new();
    let mut pending = false;
    let mut created = String::new();
    let mut expires = String::new();
    let mut uid = String::new();
    for line in output.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        match fields.first().copied() {
            Some("sec") => {
                pending = true;
                created = fields.get(5).copied().unwrap_or("").to_string();
                expires = fields.get(6).copied().unwrap_or("").to_string();
                uid.clear();
            }
            Some("fpr") if pending => {
                let fingerprint = fields.get(9).copied().unwrap_or("").to_string();
                if !fingerprint.is_empty() {
                    keys.push(GpgKey {
                        fingerprint,
                        uid: uid.clone(),
                        created: created.clone(),
                        expires: expires.clone(),
                    });
                }
                pending = false;
            }
            Some("uid") => {
                let value = fields.get(9).copied().unwrap_or("").to_string();
                if let Some(last) = keys.last_mut() {
                    if last.uid.is_empty() {
                        last.uid = value;
                    }
                } else {
                    uid = value;
                }
            }
            _ => {}
        }
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_does_not_panic() {
        let status = detect();
        if status.installed {
            assert!(status.version.is_some());
        }
    }

    #[test]
    fn imports_into_temp_home() {
        if Command::new("gpg").arg("--version").output().is_err() {
            return;
        }
        let home = tempfile::tempdir().unwrap();
        let generated = Command::new("gpg")
            .args([
                "--batch",
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "test-passphrase",
                "--quick-generate-key",
                "Test User <test@example.com>",
                "default",
                "default",
                "1d",
            ])
            .env("GNUPGHOME", home.path())
            .output()
            .unwrap();
        assert!(generated.status.success(), "{}", String::from_utf8_lossy(&generated.stderr));
        let exported = Command::new("gpg")
            .args([
                "--batch",
                "--no-tty",
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "test-passphrase",
                "--armor",
                "--export-secret-keys",
            ])
            .env("GNUPGHOME", home.path())
            .output()
            .unwrap();
        assert!(exported.status.success());
        let armored = String::from_utf8_lossy(&exported.stdout).to_string();
        assert!(armored.contains("BEGIN PGP PRIVATE KEY BLOCK"));
        let status = import_secret_key_with_home(&armored, Some(home.path())).unwrap();
        assert!(status.installed);
        assert!(!status.secret_keys.is_empty());
    }

    #[test]
    fn parses_colon_output() {
        let keys = parse_secret_keys(
            "sec:u:255:22:ABCDEF:1700000000:1800000000:::\nfpr:::::::::0123456789ABCDEF0123456789ABCDEF01234567:\nuid:u::::1700000000::HASH::Test User <test@example.com>::::::::::0:\n",
        );
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].fingerprint, "0123456789ABCDEF0123456789ABCDEF01234567");
        assert_eq!(keys[0].uid, "Test User <test@example.com>");
    }
}
