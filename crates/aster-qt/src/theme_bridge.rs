// SPDX-License-Identifier: AGPL-3.0-or-later
use std::path::{Path, PathBuf};

use aster_client_core::ClientConfig;

pub fn default_theme_dir() -> PathBuf {
    let dir = ClientConfig::default().data_dir;
    let _ = std::fs::create_dir_all(aster_theme::themes_dir(&dir));
    dir
}

pub fn themes_json(dir: &Path) -> String {
    serde_json::to_string(&aster_theme::themes_json(dir)).unwrap_or_else(|_| "[]".into())
}

pub fn tokens_json(dir: &Path, id: &str) -> Option<String> {
    aster_theme::all_themes(dir)
        .into_iter()
        .find(|theme| theme.id == id)
        .and_then(|theme| serde_json::to_string(&theme.tokens).ok())
}

pub fn theme_exists(dir: &Path, id: &str) -> bool {
    aster_theme::all_themes(dir).iter().any(|theme| theme.id == id)
}

pub fn theme_is_light(dir: &Path, id: &str) -> Option<bool> {
    aster_theme::all_themes(dir)
        .into_iter()
        .find(|theme| theme.id == id)
        .map(|theme| theme.light)
}

pub fn create_template_theme(dir: &Path) -> Option<PathBuf> {
    let themes = aster_theme::themes_dir(dir);
    let _ = std::fs::create_dir_all(&themes);
    let path = themes.join("my-theme.css");
    if path.exists() {
        return Some(path);
    }
    std::fs::write(&path, aster_theme::template_css()).ok()?;
    Some(path)
}
