// SPDX-License-Identifier: AGPL-3.0-or-later
//! Maps the Aster CSS token themes onto rune-kit's `Kit`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

use aster_client_core::settings;
use aster_theme::ThemeEntry;
use rune::color::hex;
use rune::{Color, TextStyle};
use rune_kit::{Kit, PaletteSeed, StateColors};

pub struct ThemeState {
    pub mode: String,
    pub light: String,
    pub dark: String,
    pub themes: Vec<ThemeEntry>,
    pub data_dir: PathBuf,
    pub system_dark: bool,
    pub next_check: f64,
}

impl ThemeState {
    pub fn load(data_dir: &Path) -> Self {
        let value = settings::load(data_dir);
        Self {
            mode: settings::string(&value, &["appearance", "mode"], "system"),
            light: settings::string(&value, &["appearance", "light"], "aster-light"),
            dark: settings::string(&value, &["appearance", "dark"], "aster-dark"),
            themes: aster_theme::all_themes(data_dir),
            data_dir: data_dir.to_path_buf(),
            system_dark: start_watcher(),
            next_check: 0.0,
        }
    }

    pub fn reload(&mut self) {
        self.themes = aster_theme::all_themes(&self.data_dir);
    }

    pub fn active_id(&self, system_dark: bool) -> String {
        let wanted = match self.mode.as_str() {
            "light" => self.light.as_str(),
            "dark" => self.dark.as_str(),
            _ if system_dark => self.dark.as_str(),
            _ => self.light.as_str(),
        };
        if self.themes.iter().any(|theme| theme.id == wanted) {
            wanted.to_string()
        } else if system_dark {
            "aster-dark".into()
        } else {
            "aster-light".into()
        }
    }

    pub fn active(&self, system_dark: bool) -> ThemeEntry {
        let id = self.active_id(system_dark);
        self.themes
            .iter()
            .find(|theme| theme.id == id)
            .cloned()
            .unwrap_or_else(|| {
                if system_dark {
                    aster_theme::builtin_entries()
                        .into_iter()
                        .find(|theme| theme.id == "aster-dark")
                } else {
                    aster_theme::builtin_entries()
                        .into_iter()
                        .find(|theme| theme.id == "aster-light")
                }
                .expect("builtin theme")
            })
    }

    pub fn kit(&self) -> Kit {
        kit_from_theme(&self.active(self.system_dark))
    }

    pub fn refresh_system_dark(&mut self) -> bool {
        let dark = SYSTEM_DARK.load(Ordering::Relaxed);
        if dark != self.system_dark {
            self.system_dark = dark;
            true
        } else {
            false
        }
    }

    pub fn token_color(&self, key: &str, fallback: Color) -> Color {
        let theme = self.active(self.system_dark);
        color(&theme, key, fallback)
    }

    pub fn persist(&self) {
        let patch = serde_json::json!({
            "appearance": {
                "mode": self.mode,
                "light": self.light,
                "dark": self.dark,
            }
        });
        let _ = settings::apply_patch(&self.data_dir, &patch);
    }
}

pub fn kit_from_theme(theme: &ThemeEntry) -> Kit {
    let bg = color(theme, "bg", Color::BLACK);
    let fg = color(theme, "text", Color::WHITE);
    let accent = color(theme, "accent", Color::WHITE);
    let mut k = Kit::from_palette(PaletteSeed { bg, fg, accent });
    k.dark = !theme.light;

    let surface = color(theme, "surface", bg);
    let card = color(theme, "card", surface);
    let strong = color(theme, "surfaceStrong", card);
    let menu = color(theme, "menu", card);
    let muted = color(theme, "muted", fg.alpha(0.7));
    let faint = color(theme, "quotedText", muted);
    let border = color(theme, "border", fg.alpha(0.12));
    let border_strong = color(theme, "borderStrong", fg.alpha(0.2));
    let hover = color(theme, "hover", fg.alpha(0.05));
    let selected = color(theme, "selected", fg.alpha(0.12));
    let danger = color(theme, "danger", Color::WHITE);
    let warning = color(theme, "warning", Color::WHITE);
    let success = color(theme, "success", Color::WHITE);
    let accent_text = color(theme, "accentText", accent);
    let accent_button = color(theme, "accentButton", accent);
    let accent_ink = color(theme, "accentInk", bg);

    k.c.page = bg;
    k.c.bg_base = surface;
    k.c.bg_deep = bg;
    k.c.layer_01 = surface;
    k.c.layer_02 = card;
    k.c.layer_03 = strong;
    k.c.layer_04 = accent.alpha(0.15);
    k.c.bg_inverse = fg;
    k.c.bg_contrast = accent_button;
    k.c.bg_button_neutral = card;
    k.c.bg_icon_button_contrast = accent_button;
    k.c.bg_accent = accent_button;
    k.c.submit_fg = accent_ink;
    k.c.submit_icon = accent_ink;
    k.c.text_base = fg;
    k.c.text_muted = muted;
    k.c.text_faint = faint;
    k.c.text_inverse = bg;
    k.c.text_contrast = accent_ink;
    k.c.text_accent = accent_text;
    k.c.icon_base = muted;
    k.c.icon_muted = faint;
    k.c.icon_faint = faint.alpha(0.8);
    k.c.icon_inverse = bg;
    k.c.icon_contrast = accent_ink;
    k.c.icon_accent = accent_text;
    k.c.border_base = border;
    k.c.border_muted = border;
    k.c.border_strong = border_strong;
    k.c.border_focus = color(theme, "focus", accent);
    k.c.hover = hover;
    k.c.pressed = selected;
    k.c.surface_base = color(theme, "input", strong);
    k.c.surface_weak = color(theme, "card", surface);
    k.c.surface_hover = hover;
    k.c.surface_raised = card;
    k.c.surface_raised_base = card;
    k.c.surface_raised_hover = strong;
    k.c.surface_float = menu;
    k.c.surface_disabled = strong;
    k.c.border_weak = border;
    k.c.border_hover = border_strong;
    k.c.border_active = border_strong;
    k.c.border_disabled = border;
    k.c.text_strong = fg;
    k.c.text_weak = muted;
    k.c.text_invert_strong = bg;
    k.c.icon_weaker = faint;
    k.c.std_border = border;
    k.c.std_icon = muted;
    k.c.std_icon_invert = bg;
    k.c.std_icon_strong = fg;
    k.c.std_text = fg;
    k.c.danger = role(danger);
    k.c.warning = role(warning);
    k.c.success = role(success);
    k.c.info = role(accent);

    k.radius.xs = number(theme, "radiusSmall", 1.0);
    k.radius.sm = number(theme, "radiusControl", 2.0);
    k.radius.md = number(theme, "radiusMenu", 4.0);
    k.radius.lg = number(theme, "radiusCard", 6.0);
    k.radius.xl = number(theme, "radiusPanel", 8.0);

    let body = number(theme, "fontSizeBody", 14.0);
    let line = number(theme, "lineHeightNormal", 1.35);
    let medium = number(theme, "fontWeightMedium", 500.0);
    let semibold = number(theme, "fontWeightSemibold", 600.0);
    k.ty.ui = TextStyle::new(body, 400.0).lh(body * line);
    k.ty.ui_medium = TextStyle::new(body, medium).lh(body * line);
    k.ty.ui_compact = TextStyle::new(number(theme, "fontSizeSmall", 12.0), 400.0).lh(16.0);
    k.ty.small = TextStyle::new(number(theme, "fontSizeSmall", 12.0), 400.0);
    k.ty.small_medium = TextStyle::new(number(theme, "fontSizeSmall", 12.0), medium);
    k.ty.body = TextStyle::new(number(theme, "fontSizeBody", 14.0), 400.0);
    k.ty.body_medium = TextStyle::new(number(theme, "fontSizeBody", 14.0), medium);
    k.ty.text_small = TextStyle::new(number(theme, "fontSizeSmall", 12.0), 400.0);
    k.ty.text_small_medium = TextStyle::new(number(theme, "fontSizeSmall", 12.0), medium);
    k.ty.text_xsmall = TextStyle::new(number(theme, "fontSizeCaption", 11.0), 400.0);
    k.ty.text_xsmall_medium = TextStyle::new(number(theme, "fontSizeCaption", 11.0), medium);
    k.ty.mono = TextStyle::new(body, 400.0).mono();
    let _ = semibold;

    k
}

fn role(color: Color) -> StateColors {
    StateColors {
        bg: color.alpha(0.12),
        fg: color,
        border: color.alpha(0.3),
        icon: color,
    }
}

fn token<'a>(theme: &'a ThemeEntry, key: &str) -> Option<&'a serde_json::Value> {
    theme.tokens.get(key)
}

fn color(theme: &ThemeEntry, key: &str, fallback: Color) -> Color {
    token(theme, key)
        .and_then(|value| value.as_str())
        .and_then(hex)
        .unwrap_or(fallback)
}

fn number(theme: &ThemeEntry, key: &str, fallback: f32) -> f32 {
    token(theme, key)
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)
        .unwrap_or(fallback)
}

static SYSTEM_DARK: AtomicBool = AtomicBool::new(false);
static WATCHER: Once = Once::new();

/// Starts a background watcher (once per process) and returns the current
/// preference. Keeps `gsettings` off the UI thread.
pub fn start_watcher() -> bool {
    WATCHER.call_once(|| {
        let initial = detect_system_dark();
        SYSTEM_DARK.store(initial, Ordering::Relaxed);
        std::thread::spawn(|| loop {
            std::thread::sleep(std::time::Duration::from_secs(10));
            let dark = detect_system_dark();
            SYSTEM_DARK.store(dark, Ordering::Relaxed);
        });
    });
    SYSTEM_DARK.load(Ordering::Relaxed)
}

/// Matches the Qt app's `Application.styleHints.colorScheme` (GTK/GNOME/KDE),
/// not the XDG portal preference, so both UIs pick the same slot.
pub fn detect_system_dark() -> bool {
    if let Ok(value) = std::env::var("GTK_THEME") {
        let value = value.to_ascii_lowercase();
        if value.contains("dark") {
            return true;
        }
        if value.contains("light") {
            return false;
        }
    }
    for args in [
        &["get", "org.gnome.desktop.interface", "gtk-theme"][..],
        &["get", "org.gnome.desktop.interface", "color-scheme"][..],
    ] {
        if let Ok(output) = std::process::Command::new("gsettings").args(args).output() {
            let value = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
            if value.contains("prefer-dark") || value.contains("dark") {
                return true;
            }
            if value.contains("prefer-light") || value.contains("light") {
                return false;
            }
        }
    }
    for command in ["kreadconfig6", "kreadconfig5"] {
        if let Ok(output) = std::process::Command::new(command)
            .args([
                "--file",
                "kdeglobals",
                "--group",
                "General",
                "--key",
                "ColorScheme",
            ])
            .output()
        {
            let value = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
            if value.contains("dark") {
                return true;
            }
            if value.contains("light") {
                return false;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_themes_produce_kits() {
        for entry in aster_theme::builtin_entries() {
            let kit = kit_from_theme(&entry);
            assert_eq!(kit.dark, !entry.light);
            assert!(kit.ty.ui.size >= 11.0);
        }
    }

    #[test]
    fn active_id_follows_mode_and_system() {
        let dir = std::env::temp_dir().join(format!("aster-theme-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut state = ThemeState::load(&dir);
        state.mode = "system".into();
        assert_eq!(state.active_id(true), "aster-dark");
        assert_eq!(state.active_id(false), "aster-light");
        state.mode = "light".into();
        assert_eq!(state.active_id(true), "aster-light");
        let _ = std::fs::remove_dir_all(dir);
    }
}
