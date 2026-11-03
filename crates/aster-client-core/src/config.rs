// SPDX-License-Identifier: AGPL-3.0-or-later
use std::path::{Path, PathBuf};

use aster_bridge_core::runtime::ClientProfile;

pub const APP_ID: &str = "com.astermail.qt";
pub const DEFAULT_PROFILE: ClientProfile = ClientProfile::Desktop;

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub data_dir: PathBuf,
    pub keyring_service: String,
    pub profile: ClientProfile,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            keyring_service: APP_ID.to_string(),
            profile: DEFAULT_PROFILE,
        }
    }
}

impl ClientConfig {
    pub fn with_data_dir(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            ..Self::default()
        }
    }

    pub fn prepare(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.data_dir)
            .map_err(|e| format!("create data dir {}: {}", self.data_dir.display(), e))
    }

    pub fn lock_path(&self) -> PathBuf {
        self.data_dir.join("instance.lock")
    }
}

pub fn default_data_dir() -> PathBuf {
    match std::env::var_os("XDG_DATA_HOME") {
        Some(dir) => PathBuf::from(dir).join(APP_ID),
        None => home_dir().join(".local/share").join(APP_ID),
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn ensure_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| format!("create {}: {}", path.display(), e))
}


/// True when the data directory sits inside a git worktree. Personal data must
/// never be committed, so the app warns loudly in that case.
pub fn inside_git_worktree(path: &Path) -> bool {
    let mut cursor = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf());
    loop {
        if cursor.join(".git").exists() {
            return true;
        }
        match cursor.parent() {
            Some(parent) if parent != cursor => cursor = parent.to_path_buf(),
            _ => return false,
        }
    }
}
