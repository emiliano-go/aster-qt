// SPDX-License-Identifier: AGPL-3.0-or-later
//! Single source of truth for persisted application settings.
//!
//! Everything user-configurable lives in `<data>/settings.json`. Feature
//! modules must not create their own settings files.

use std::path::Path;

use serde_json::{json, Value};

pub fn defaults() -> Value {
    json!({
        "appearance": {
            "mode": "system",
            "light": "aster-light",
            "dark": "aster-dark"
        },
        "mail": {
            "reader_open": true,
            "poll_interval_secs": 30,
            "full_history": true,
            "preview_lines": 1
        },
        "security": {
            "require_post_quantum": false,
            "encrypt_emails": true,
            "gpg_sign_outgoing": false,
            "gpg_key_fingerprint": ""
        },
        "mcp": {
            "enabled": false,
            "allow_send": false,
            "allow_destructive": false
        }
    })
}

pub fn load(data_dir: &Path) -> Value {
    let path = data_dir.join("settings.json");
    let mut value = defaults();
    if let Some(stored) = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    {
        merge(&mut value, &stored);
    }
    value
}

pub fn save(data_dir: &Path, value: &Value) -> Result<(), String> {
    let path = data_dir.join("settings.json");
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("write {}: {}", path.display(), e))
}

pub fn apply_patch(data_dir: &Path, patch: &Value) -> Result<Value, String> {
    let mut value = load(data_dir);
    merge(&mut value, patch);
    save(data_dir, &value)?;
    Ok(value)
}

/// Recursive object merge. Scalars and arrays in `patch` replace the base.
pub fn merge(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                match base_map.get_mut(key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        base_map.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base_slot, patch_value) => *base_slot = patch_value.clone(),
    }
}

pub fn string(value: &Value, path: &[&str], fallback: &str) -> String {
    let mut cursor = value;
    for key in path {
        cursor = match cursor.get(*key) {
            Some(next) => next,
            None => return fallback.to_string(),
        };
    }
    cursor.as_str().unwrap_or(fallback).to_string()
}

pub fn boolean(value: &Value, path: &[&str], fallback: bool) -> bool {
    let mut cursor = value;
    for key in path {
        cursor = match cursor.get(*key) {
            Some(next) => next,
            None => return fallback,
        };
    }
    cursor.as_bool().unwrap_or(fallback)
}

pub fn integer(value: &Value, path: &[&str], fallback: i64) -> i64 {
    let mut cursor = value;
    for key in path {
        cursor = match cursor.get(*key) {
            Some(next) => next,
            None => return fallback,
        };
    }
    cursor.as_i64().unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let value = load(dir.path());
        assert_eq!(value["appearance"]["mode"], "system");
        assert_eq!(boolean(&value, &["mail", "full_history"], false), true);
    }

    #[test]
    fn patch_merges_nested_objects() {
        let dir = tempfile::tempdir().unwrap();
        let value = apply_patch(dir.path(), &json!({"appearance": {"dark": "dusk"}})).unwrap();
        assert_eq!(value["appearance"]["dark"], "dusk");
        assert_eq!(value["appearance"]["mode"], "system");
        let reloaded = load(dir.path());
        assert_eq!(reloaded["appearance"]["dark"], "dusk");
    }

    #[test]
    fn scalar_replaces() {
        let dir = tempfile::tempdir().unwrap();
        apply_patch(dir.path(), &json!({"mail": {"poll_interval_secs": 90}})).unwrap();
        assert_eq!(integer(&load(dir.path()), &["mail", "poll_interval_secs"], 0), 90);
    }
}
