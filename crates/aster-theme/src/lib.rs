// SPDX-License-Identifier: AGPL-3.0-or-later
//! CSS custom-property theme engine.
//!
//! QML has no CSS selector engine. A theme is a `:root` token sheet: every
//! visual value in the application is a registered custom property. Unknown
//! tokens are rejected so typos cannot silently fall back.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Color,
    Length,
    Number,
    Integer,
    Duration,
    Text,
}

pub struct TokenSpec {
    pub key: &'static str,
    pub css: &'static str,
    pub kind: TokenKind,
    pub default: Value,
    pub comment: &'static str,
}

macro_rules! spec {
    ($key:literal, $css:literal, $kind:ident, $default:expr, $comment:literal) => {
        TokenSpec {
            key: $key,
            css: $css,
            kind: TokenKind::$kind,
            default: json!($default),
            comment: $comment,
        }
    };
}

static SPECS: LazyLock<Vec<TokenSpec>> = LazyLock::new(|| {
    vec![
        // Palette
        spec!("bg", "--bg", Color, "#121417", "Window background"),
        spec!("sidebar", "--sidebar", Color, "#1a1d22", "Sidebar background"),
        spec!("surface", "--surface", Color, "#1b1e23", "Panels and dialogs"),
        spec!("card", "--card", Color, "#23262c", "Card surface"),
        spec!("surfaceStrong", "--surface-strong", Color, "#2e323a", "Raised controls and selected rows"),
        spec!("menu", "--menu", Color, "#23262c", "Menus and popovers"),
        spec!("input", "--input", Color, "#221b1e23", "Text field background"),
        spec!("hover", "--hover", Color, "#2e2e323a", "Hover background"),
        spec!("selected", "--selected", Color, "#3d2e323a", "Selected row background"),
        spec!("border", "--border", Color, "#171b1e23", "Default border"),
        spec!("borderStrong", "--border-strong", Color, "#292e323a", "Hover and focus border"),
        spec!("text", "--text", Color, "#ece7de", "Primary text"),
        spec!("text2", "--text-2", Color, "#dbebe7de", "Secondary text"),
        spec!("muted", "--muted", Color, "#a8ece7de", "Muted text and icons"),
        spec!("accent", "--accent", Color, "#d98e5c", "Accent color"),
        spec!("accentButton", "--accent-button", Color, "#d98e5c", "Solid button background"),
        spec!("accentText", "--accent-text", Color, "#e7a97f", "Accent-colored text"),
        spec!("accentInk", "--accent-ink", Color, "#171717", "Text on accent backgrounds"),
        spec!("danger", "--danger", Color, "#d97a72", "Errors and destructive actions"),
        spec!("warning", "--warning", Color, "#d9b36a", "Warnings"),
        spec!("success", "--success", Color, "#7fae83", "Success state"),
        spec!("selection", "--selection", Color, "#59d98e5c", "Text selection"),
        spec!("focus", "--focus", Color, "#d98e5c", "Focus ring"),
        spec!("overlay", "--overlay", Color, "#8c000000", "Modal scrim"),
        spec!("shadow", "--shadow", Color, "#40000000", "Shadow color"),
        spec!("unreadDot", "--unread-dot", Color, "#d98e5c", "Unread indicator"),
        spec!("trustVerified", "--trust-verified", Color, "#7fae83", "Verified sender"),
        spec!("trustWarning", "--trust-warning", Color, "#d9b36a", "Unverified or changed sender"),
        spec!("trustDanger", "--trust-danger", Color, "#d97a72", "Dangerous or failed trust"),
        spec!("pgpOk", "--pgp-ok", Color, "#7fae83", "PGP key is present and trusted"),
        spec!("pgpWarn", "--pgp-warn", Color, "#d9b36a", "PGP key is missing, expiring, or unverified"),
        spec!("threadLine", "--thread-line", Color, "#2e2e323a", "Conversation thread line"),
        spec!("messageDivider", "--message-divider", Color, "#343a44", "Divider between message rows"),
        spec!("quotedText", "--quoted-text", Color, "#a8ece7de", "Quoted reply text"),
        spec!("htmlMailBg", "--html-mail-bg", Color, "#ffffff", "HTML mail body background"),
        spec!("signature", "--signature", Color, "#a8ece7de", "Signature text"),
        // Typography
        spec!("fontFamily", "--font-family", Text, "Inter", "UI font family"),
        spec!("fontBrand", "--font-brand", Text, "TeX Gyre Adventor", "Brand font family"),
        spec!("fontMono", "--font-mono", Text, "JetBrains Mono", "Monospace font family"),
        spec!("fontMail", "--font-mail", Text, "Inter", "Message body font family"),
        spec!("fontSizeMicro", "--font-size-micro", Length, 9, "Micro text"),
        spec!("fontSizeCaption", "--font-size-caption", Length, 10, "Caption text"),
        spec!("fontSizeSmall", "--font-size-small", Length, 12, "Small text"),
        spec!("fontSizeBody", "--font-size-body", Length, 14, "Body text"),
        spec!("fontSizeSubtitle", "--font-size-subtitle", Length, 15, "Subtitle text"),
        spec!("fontSizeTitle", "--font-size-title", Length, 16, "Title text"),
        spec!("fontSizePage", "--font-size-page", Length, 18, "Page title"),
        spec!("fontSizeLarge", "--font-size-large", Length, 22, "Large display text"),
        spec!("fontSizeDisplay", "--font-size-display", Length, 32, "Hero display text"),
        spec!("fontSizeHtmlMail", "--font-size-html-mail", Length, 15, "HTML mail body size"),
        spec!("fontWeightRegular", "--font-weight-regular", Integer, 400, "Regular weight"),
        spec!("fontWeightMedium", "--font-weight-medium", Integer, 500, "Medium weight"),
        spec!("fontWeightSemibold", "--font-weight-semibold", Integer, 600, "Semibold weight"),
        spec!("fontWeightUnread", "--font-weight-unread", Integer, 600, "Unread row weight"),
        spec!("lineHeightTight", "--line-height-tight", Number, 1.2, "Tight line height"),
        spec!("lineHeightNormal", "--line-height-normal", Number, 1.35, "Normal line height"),
        spec!("lineHeightRelaxed", "--line-height-relaxed", Number, 1.55, "Relaxed line height"),
        spec!("lineHeightHtmlMail", "--line-height-html-mail", Number, 1.5, "HTML mail line height"),
        // Spacing
        spec!("space0", "--space-0", Length, 0, "No space"),
        spec!("space1", "--space-1", Length, 4, "Extra small gap"),
        spec!("space2", "--space-2", Length, 8, "Small gap"),
        spec!("spaceSnug", "--space-snug", Length, 10, "Snug gap"),
        spec!("space3", "--space-3", Length, 12, "Medium gap"),
        spec!("space4", "--space-4", Length, 16, "Default gap"),
        spec!("space5", "--space-5", Length, 24, "Large gap"),
        spec!("space6", "--space-6", Length, 32, "Section gap"),
        spec!("space7", "--space-7", Length, 40, "Page gap"),
        spec!("space8", "--space-8", Length, 48, "Hero gap"),
        spec!("spaceHalf", "--space-half", Length, 2, "Hairline gap"),
        spec!("spaceTight", "--space-tight", Length, 5, "Tight gap"),
        spec!("spaceCompact", "--space-compact", Length, 6, "Compact gap"),
        // Radii
        spec!("radiusLine", "--radius-line", Length, 1, "Hairline radius"),
        spec!("radiusHairline", "--radius-hairline", Length, 2, "Hairline corner"),
        spec!("radiusSmall", "--radius-small", Length, 3, "Small corner"),
        spec!("radiusMicro", "--radius-micro", Length, 4, "Micro corner"),
        spec!("radiusDot", "--radius-dot", Length, 5, "Dot corner"),
        spec!("radiusLoose", "--radius-loose", Length, 7, "Loose corner"),
        spec!("radiusBadge", "--radius-badge", Length, 9, "Badge corner"),
        spec!("radiusWide", "--radius-wide", Length, 11, "Wide corner"),
        spec!("radiusControl", "--radius-control", Length, 8, "Buttons and inputs"),
        spec!("radiusCard", "--radius-card", Length, 14, "Cards"),
        spec!("radiusPanel", "--radius-panel", Length, 16, "Panels and dialogs"),
        spec!("radiusMenu", "--radius-menu", Length, 10, "Menus and popovers"),
        spec!("radiusPill", "--radius-pill", Length, 999, "Pills and toggles"),
        spec!("radiusKeycap", "--radius-keycap", Length, 6, "Keyboard keycaps"),
        // Borders and icons
        spec!("borderWidth", "--border-width", Number, 1, "Default border width"),
        spec!("borderWidthStrong", "--border-width-strong", Number, 1.5, "Emphasized border width"),
        spec!("borderWidthFocus", "--border-width-focus", Number, 1.5, "Focus border width"),
        spec!("iconSmall", "--icon-small", Length, 14, "Small icon"),
        spec!("iconBody", "--icon-body", Length, 16, "Default icon"),
        spec!("iconLarge", "--icon-large", Length, 20, "Large icon"),
        spec!("iconPage", "--icon-page", Length, 32, "Page mark"),
        // Layout and controls
        spec!("controlHeight", "--control-height", Length, 36, "Default control height"),
        spec!("controlHeightSmall", "--control-height-small", Length, 30, "Compact control height"),
        spec!("topbarHeight", "--topbar-height", Length, 56, "Top bar height"),
        spec!("statusbarHeight", "--statusbar-height", Length, 30, "Status bar height"),
        spec!("sidebarWidth", "--sidebar-width", Length, 256, "Expanded sidebar width"),
        spec!("listWidth", "--list-width", Length, 380, "Message list width"),
        spec!("readerWidth", "--reader-width", Length, 720, "Reader content width"),
        spec!("settingsNavWidth", "--settings-nav-width", Length, 220, "Settings navigation width"),
        spec!("dialogWidth", "--dialog-width", Length, 480, "Default dialog width"),
        spec!("dialogWidthWide", "--dialog-width-wide", Length, 720, "Wide dialog width"),
        spec!("composerHeight", "--composer-height", Length, 560, "Composer window height"),
        spec!("messageRowHeight", "--message-row-height", Length, 82, "Message row height"),
        spec!("messageRowPadding", "--message-row-padding", Length, 10, "Message row padding"),
        spec!("threadIndent", "--thread-indent", Length, 18, "Thread indentation"),
        spec!("htmlMailMaxWidth", "--html-mail-max-width", Length, 720, "HTML mail max width"),
        spec!("trustBannerHeight", "--trust-banner-height", Length, 34, "Trust banner height"),
        spec!("unreadBarWidth", "--unread-bar-width", Length, 3, "Unread bar width"),
        spec!("attachmentChipHeight", "--attachment-chip-height", Length, 30, "Attachment chip height"),
        // Motion
        spec!("motionFast", "--motion-fast", Duration, 110, "Fast transition (ms)"),
        spec!("motionMedium", "--motion-medium", Duration, 180, "Medium transition (ms)"),
        spec!("motionSlow", "--motion-slow", Duration, 280, "Slow transition (ms)"),
        spec!("motionToast", "--motion-toast", Duration, 2400, "Toast lifetime (ms)"),
        spec!("easingStandard", "--easing-standard", Text, "OutCubic", "Standard easing curve"),
        spec!("easingEmphasized", "--easing-emphasized", Text, "OutQuint", "Emphasized easing curve"),
        // Opacity and shadows
        spec!("opacityNone", "--opacity-none", Number, 0.0, "Fully transparent"),
        spec!("opacityFaint", "--opacity-faint", Number, 0.35, "Faint decoration"),
        spec!("opacityMedium", "--opacity-medium", Number, 0.6, "Medium decoration"),
        spec!("opacityEmphasis", "--opacity-emphasis", Number, 0.65, "Emphasized decoration"),
        spec!("opacityDisabled", "--opacity-disabled", Number, 0.45, "Disabled control"),
        spec!("shadowBlur", "--shadow-blur", Number, 24, "Shadow blur radius"),
        spec!("shadowOffsetY", "--shadow-offset-y", Number, 8, "Shadow vertical offset"),
        spec!("shadowOpacity", "--shadow-opacity", Number, 0.25, "Shadow opacity"),
    ]
});

pub fn specs() -> &'static [TokenSpec] {
    &SPECS
}

pub fn spec_map() -> BTreeMap<&'static str, &'static TokenSpec> {
    specs().iter().map(|s| (s.css, s)).collect()
}

pub fn default_tokens() -> Map<String, Value> {
    specs()
        .iter()
        .map(|s| (s.key.to_string(), s.default.clone()))
        .collect()
}

fn parse_hex(value: &str) -> Option<(u8, u8, u8, u8)> {
    let hex = value.trim().trim_start_matches('#');
    let expand = |c: char| -> Option<u8> { c.to_digit(16).map(|v| (v * 17) as u8) };
    match hex.len() {
        3 => {
            let mut it = hex.chars();
            Some((
                expand(it.next()?)?,
                expand(it.next()?)?,
                expand(it.next()?)?,
                255,
            ))
        }
        6 => Some((
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            255,
        )),
        8 => Some((
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            u8::from_str_radix(&hex[6..8], 16).ok()?,
            u8::from_str_radix(&hex[0..2], 16).ok()?,
        )),
        _ => None,
    }
}

fn qml_hex(r: u8, g: u8, b: u8, a: u8) -> String {
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{a:02x}{r:02x}{g:02x}{b:02x}")
    }
}

fn css_hex_from_qml(value: &str) -> String {
    let Some((r, g, b, a)) = parse_hex(value) else {
        return value.to_string();
    };
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

fn parse_css_color(value: &str) -> Result<String> {
    let raw = value.trim();
    if raw.eq_ignore_ascii_case("transparent") {
        return Ok("#00000000".into());
    }
    if raw.eq_ignore_ascii_case("white") {
        return Ok("#ffffff".into());
    }
    if raw.eq_ignore_ascii_case("black") {
        return Ok("#000000".into());
    }
    let css_hex = raw.trim_start_matches('#');
    if css_hex.len() == 8 {
        let p = |r: std::ops::Range<usize>| u8::from_str_radix(&css_hex[r], 16).ok();
        if let (Some(r), Some(g), Some(b), Some(a)) = (p(0..2), p(2..4), p(4..6), p(6..8)) {
            return Ok(qml_hex(r, g, b, a));
        }
    }
    if css_hex.len() == 4 {
        let mut it = css_hex.chars();
        let expand = |c: char| c.to_digit(16).map(|v| (v * 17) as u8);
        if let (Some(r), Some(g), Some(b), Some(a)) = (
            it.next().and_then(expand),
            it.next().and_then(expand),
            it.next().and_then(expand),
            it.next().and_then(expand),
        ) {
            return Ok(qml_hex(r, g, b, a));
        }
    }
    if let Some((r, g, b, a)) = parse_hex(raw) {
        return Ok(qml_hex(r, g, b, a));
    }
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("rgb(") || lower.starts_with("rgba(") {
        let body = lower
            .split_once('(')
            .and_then(|(_, rest)| rest.split_once(')'))
            .map(|(body, _)| body)
            .unwrap_or("");
        let parts: Vec<&str> = body.split(',').map(str::trim).collect();
        if parts.len() == 3 || parts.len() == 4 {
            let channel = |s: &str| -> Result<u8> {
                let n: f64 = s
                    .trim_end_matches('%')
                    .parse()
                    .with_context(|| format!("invalid rgb channel `{s}`"))?;
                let n = if s.contains('%') { n * 2.55 } else { n };
                Ok(n.round().clamp(0.0, 255.0) as u8)
            };
            let alpha = if parts.len() == 4 {
                let n: f64 = parts[3]
                    .trim_end_matches('%')
                    .parse()
                    .context("invalid alpha")?;
                let n = if parts[3].contains('%') { n / 100.0 } else { n };
                (n.clamp(0.0, 1.0) * 255.0).round() as u8
            } else {
                255
            };
            return Ok(qml_hex(channel(parts[0])?, channel(parts[1])?, channel(parts[2])?, alpha));
        }
    }
    bail!("invalid color `{raw}`")
}

fn parse_number(value: &str, kind: TokenKind) -> Result<Value> {
    let raw = value.trim();
    let cleaned = raw
        .trim_end_matches("px")
        .trim_end_matches("ms")
        .trim_end_matches('%')
        .trim();
    let n: f64 = cleaned
        .parse()
        .with_context(|| format!("invalid number `{raw}`"))?;
    if !n.is_finite() {
        bail!("number must be finite");
    }
    match kind {
        TokenKind::Integer => Ok(json!(n.round() as i64)),
        TokenKind::Length | TokenKind::Number | TokenKind::Duration => Ok(json!(n)),
        _ => bail!("not a number token"),
    }
}

fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

#[derive(Clone, Debug)]
pub struct ThemeEntry {
    pub id: String,
    pub name: String,
    pub light: bool,
    pub builtin: bool,
    pub source: String,
    pub tokens: Map<String, Value>,
}

impl ThemeEntry {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "light": self.light,
            "builtin": self.builtin,
            "source": self.source,
            "bg": self.tokens.get("bg").cloned().unwrap_or(json!("#121417")),
            "surface": self.tokens.get("surface").cloned().unwrap_or(json!("#1b1e23")),
            "accent": self.tokens.get("accent").cloned().unwrap_or(json!("#d98e5c")),
            "text": self.tokens.get("text").cloned().unwrap_or(json!("#ece7de")),
            "tokens": self.tokens,
        })
    }
}

/// Tokens keyed by both the Rust name and the CSS property name.
/// QML uses the CSS names, the engine uses the Rust names.
pub fn qml_tokens(entry: &ThemeEntry) -> Map<String, Value> {
    let mut out = Map::new();
    for spec in specs() {
        if let Some(value) = entry.tokens.get(spec.key) {
            out.insert(spec.key.to_string(), value.clone());
            out.insert(
                spec.css.trim_start_matches("--").to_string(),
                value.clone(),
            );
        }
    }
    out
}

pub fn parse_css_theme(text: &str, fallback_id: &str) -> Result<ThemeEntry> {
    let clean = strip_comments(text);
    let mut declarations: BTreeMap<String, String> = BTreeMap::new();
    let mut found_root = false;
    let mut rest = clean.as_str();
    while let Some(pos) = rest.find(":root") {
        found_root = true;
        rest = &rest[pos + 5..];
        let Some(open) = rest.find('{') else { break };
        let Some(close) = rest[open + 1..].find('}') else { break };
        let body = &rest[open + 1..open + 1 + close];
        for decl in body.split(';') {
            let Some((key, value)) = decl.split_once(':') else { continue };
            let key = key.trim();
            if key.starts_with("--") {
                declarations.insert(key.to_string(), value.trim().to_string());
            }
        }
        rest = &rest[open + 1 + close + 1..];
    }
    if !found_root {
        bail!("theme CSS needs a :root rule");
    }

    let meta = |name: &str| declarations.get(name).cloned().unwrap_or_default();
    let id = {
        let v = meta("--theme-id");
        if v.is_empty() {
            fallback_id.to_string()
        } else {
            v.trim_matches('"').to_string()
        }
    };
    if id.is_empty() || !valid_id(&id) {
        bail!("theme id must contain only letters, numbers, '-' or '_'");
    }
    let name = {
        let v = meta("--theme-name");
        let v = v.trim_matches('"');
        if v.is_empty() { id.clone() } else { v.to_string() }
    };
    let light = match meta("--theme-light").to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" => true,
        "false" | "no" | "0" | "" => false,
        other => bail!("--theme-light must be true or false, got `{other}`"),
    };

    let mut tokens = default_tokens();
    let known = spec_map();
    for (css, value) in &declarations {
        if matches!(css.as_str(), "--theme-id" | "--theme-name" | "--theme-light") {
            continue;
        }
        let Some(spec) = known.get(css.as_str()) else {
            bail!("unknown theme token `{css}`");
        };
        let parsed = match spec.kind {
            TokenKind::Color => json!(parse_css_color(value)?),
            TokenKind::Length | TokenKind::Number | TokenKind::Integer | TokenKind::Duration => {
                parse_number(value, spec.kind)?
            }
            TokenKind::Text => json!(value.trim_matches('"').trim_matches('\'')),
        };
        tokens.insert(spec.key.to_string(), parsed);
    }
    Ok(ThemeEntry {
        id,
        name,
        light,
        builtin: false,
        source: "css".into(),
        tokens,
    })
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn themes_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("themes")
}

pub fn load_css_themes(data_dir: &Path) -> Vec<ThemeEntry> {
    let dir = themes_dir(data_dir);
    let Ok(read) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("css") {
            continue;
        }
        let fallback = path.file_stem().and_then(|s| s.to_str()).unwrap_or("theme");
        match std::fs::read_to_string(&path)
            .map_err(|e| anyhow!(e))
            .and_then(|text| parse_css_theme(&text, fallback))
        {
            Ok(mut theme) => {
                theme.source = path.display().to_string();
                out.push(theme);
            }
            Err(err) => tracing::warn!("theme {}: {err}", path.display()),
        }
    }
    out.sort_by_key(|t| t.name.to_lowercase());
    out
}

pub fn builtin_entries() -> Vec<ThemeEntry> {
    let dark = ThemeEntry {
        id: "aster-dark".into(),
        name: "Aster Dark".into(),
        light: false,
        builtin: true,
        source: "builtin".into(),
        tokens: default_tokens(),
    };
    let mut light_tokens = default_tokens();
    let overrides = [
        ("bg", "#f6f3ee"),
        ("sidebar", "#efeae2"),
        ("surface", "#fbf9f5"),
        ("card", "#ffffff"),
        ("surfaceStrong", "#e7e1d8"),
        ("menu", "#ffffff"),
        ("input", "#14000000"),
        ("hover", "#14000000"),
        ("selected", "#22000000"),
        ("border", "#22000000"),
        ("borderStrong", "#33000000"),
        ("text", "#1c1b19"),
        ("text2", "#3a3835"),
        ("muted", "#6b6761"),
        ("selection", "#59d98e5c"),
        ("threadLine", "#22000000"),
        ("quotedText", "#6b6761"),
        ("signature", "#6b6761"),
        ("htmlMailBg", "#ffffff"),
    ];
    for (key, value) in overrides {
        light_tokens.insert(key.into(), json!(value));
    }
    let light = ThemeEntry {
        id: "aster-light".into(),
        name: "Aster Light".into(),
        light: true,
        builtin: true,
        source: "builtin".into(),
        tokens: light_tokens,
    };
    vec![light, dark]
}

pub fn all_themes(data_dir: &Path) -> Vec<ThemeEntry> {
    let mut out = builtin_entries();
    out.extend(load_css_themes(data_dir));
    out
}

pub fn themes_json(data_dir: &Path) -> Value {
    Value::Array(all_themes(data_dir).iter().map(ThemeEntry::to_json).collect())
}

pub fn theme_exists(data_dir: &Path, id: &str) -> bool {
    all_themes(data_dir).iter().any(|t| t.id == id)
}

pub fn write_css_theme(data_dir: &Path, entry: &ThemeEntry) -> Result<PathBuf> {
    std::fs::create_dir_all(themes_dir(data_dir))?;
    let path = themes_dir(data_dir).join(format!("{}.css", entry.id));
    std::fs::write(&path, css_from_entry(entry))?;
    Ok(path)
}

pub fn css_from_entry(entry: &ThemeEntry) -> String {
    let mut out = String::new();
    out.push_str("/* Aster Qt CSS theme. */\n:root {\n");
    out.push_str(&format!("  --theme-id: {};\n", entry.id));
    out.push_str(&format!("  --theme-name: {};\n", entry.name));
    out.push_str(&format!("  --theme-light: {};\n", entry.light));
    for spec in specs() {
        let value = entry
            .tokens
            .get(spec.key)
            .cloned()
            .unwrap_or_else(|| spec.default.clone());
        let text = match spec.kind {
            TokenKind::Color => css_hex_from_qml(value.as_str().unwrap_or("#000000")),
            TokenKind::Length => format!("{}px", value.as_f64().unwrap_or(0.0)),
            TokenKind::Duration => {
                format!("{}ms", value.as_f64().map(|v| v.round() as i64).unwrap_or(0))
            }
            TokenKind::Number | TokenKind::Integer => value.to_string(),
            TokenKind::Text => value.as_str().unwrap_or("").to_string(),
        };
        out.push_str(&format!("  {}: {};\n", spec.css, text));
    }
    out.push_str("}\n");
    out
}

pub const TEMPLATE: &str = include_str!("../../../themes/template.css");

pub fn template_css() -> String {
    let mut out = String::new();
    out.push_str("/* Aster Qt theme template.\n");
    out.push_str(" * Copy to <data>/themes/<id>.css, edit any token, select in Settings > Appearance.\n");
    out.push_str(" * This is a token sheet, not a selector stylesheet. Every token below is honored.\n");
    out.push_str(" */\n\n:root {\n");
    out.push_str("  /* Identity */\n");
    out.push_str("  --theme-id: my-theme;\n");
    out.push_str("  --theme-name: My Theme;\n");
    out.push_str("  --theme-light: false;\n\n");
    for spec in specs() {
        out.push_str(&format!("  /* {} */\n", spec.comment));
        let value = match spec.kind {
            TokenKind::Color => css_hex_from_qml(spec.default.as_str().unwrap_or("#000000")),
            TokenKind::Length => format!("{}px", spec.default.as_f64().unwrap_or(0.0)),
            TokenKind::Duration => {
                format!("{}ms", spec.default.as_i64().unwrap_or(0))
            }
            TokenKind::Number | TokenKind::Integer => spec.default.to_string(),
            TokenKind::Text => spec.default.as_str().unwrap_or("").to_string(),
        };
        out.push_str(&format!("  {}: {};\n", spec.css, value));
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_contains_every_token() {
        for spec in specs() {
            assert!(TEMPLATE.contains(spec.css), "missing {} in template", spec.css);
        }
    }

    #[test]
    fn checked_in_template_matches_generator() {
        assert_eq!(TEMPLATE, template_css());
    }

    #[test]
    fn parser_accepts_colors_and_units() {
        let css = ":root{--theme-id:t;--theme-name:T;--theme-light:true;--bg:#fff;--accent:rgb(217,142,92);--space-3:12px;--motion-fast:110ms;--opacity-medium:0.6;--font-family:Inter;}";
        let theme = parse_css_theme(css, "fallback").unwrap();
        assert_eq!(theme.id, "t");
        assert!(theme.light);
        assert_eq!(theme.tokens["bg"], "#ffffff");
        assert_eq!(theme.tokens["space3"], 12.0);
        assert_eq!(theme.tokens["motionFast"], 110.0);
    }

    #[test]
    fn qml_tokens_include_css_aliases() {
        let theme = builtin_entries().into_iter().next().unwrap();
        let tokens = qml_tokens(&theme);
        assert_eq!(tokens.get("text2"), tokens.get("text-2"));
        assert_eq!(tokens.get("fontSizeBody"), tokens.get("font-size-body"));
    }

    #[test]
    fn parser_rejects_unknown_tokens() {
        assert!(parse_css_theme(":root{--not-a-token:1;}", "bad").is_err());
    }

    #[test]
    fn parser_requires_root() {
        assert!(parse_css_theme("body{--bg:#000;}", "bad").is_err());
    }

    #[test]
    fn builtin_themes_have_all_tokens() {
        for theme in builtin_entries() {
            for spec in specs() {
                assert!(theme.tokens.contains_key(spec.key), "{} missing {}", theme.id, spec.key);
            }
        }
    }

    #[test]
    fn round_trip_through_css() {
        let theme = parse_css_theme(&template_css(), "my-theme").unwrap();
        let css = css_from_entry(&theme);
        let again = parse_css_theme(&css, "my-theme").unwrap();
        assert_eq!(again.tokens, theme.tokens);
    }

    #[test]
    fn theme_id_cannot_escape_directory() {
        assert!(!valid_id("../evil"));
        assert!(valid_id("dusk"));
    }
}
