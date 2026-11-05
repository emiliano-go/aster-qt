// SPDX-License-Identifier: AGPL-3.0-or-later
//! Best-effort native dark mode for HTML email bodies.

use std::collections::HashMap;
use std::path::Path;

use base64::Engine as _;
use sha2::{Digest, Sha256};

const DARK_SURFACE: &str = "#121212";
const DARK_INK: &str = "#e5e5e5";
const DARK_LINK: &str = "#7cc4ff";
const DARK_BORDER: &str = "#374151";

pub fn darken_html(html: &str, dim_images: bool, cache_dir: Option<&Path>) -> String {
    let mut image_cache: HashMap<String, String> = HashMap::new();
    let transformed = rewrite_html(html, dim_images, cache_dir, &mut image_cache);
    format!(
        "<div style=\"background-color:{DARK_SURFACE};color:{DARK_INK};font-family:Inter,system-ui,sans-serif\">{transformed}</div>"
    )
}

fn rewrite_html(
    html: &str,
    dim_images: bool,
    cache_dir: Option<&Path>,
    cache: &mut HashMap<String, String>,
) -> String {
    use lol_html::{element, rewrite_str, RewriteStrSettings};
    let mut out = rewrite_str(
        html,
        RewriteStrSettings {
            element_content_handlers: vec![
                element!("body", |el| {
                    el.set_attribute("style", &format!("background-color:{DARK_SURFACE};color:{DARK_INK};margin:0"))
                        .ok();
                    Ok(())
                }),
                element!("[style]", |el| {
                    if let Some(style) = el.get_attribute("style") {
                        el.set_attribute("style", &rewrite_style(&style))
                            .ok();
                    }
                    Ok(())
                }),
                element!("[bgcolor]", |el| {
                    el.remove_attribute("bgcolor");
                    Ok(())
                }),
                element!("font[color]", |el| {
                    el.remove_attribute("color");
                    Ok(())
                }),
                element!("a", |el| {
                    el.set_attribute("style", &format!("color:{DARK_LINK};text-decoration:underline"))
                        .ok();
                    Ok(())
                }),
                element!("hr", |el| {
                    el.set_attribute("style", &format!("border:0;border-top:1px solid {DARK_BORDER}"))
                        .ok();
                    Ok(())
                }),
                element!("img", |el| {
                    if !dim_images {
                        return Ok(());
                    }
                    let Some(src) = el.get_attribute("src") else {
                        return Ok(());
                    };
                    if let Some(dimmed) = cache.get(&src) {
                        el.set_attribute("src", dimmed).ok();
                        return Ok(());
                    }
                    if let Some(dimmed) = dim_image_src(&src, cache_dir) {
                        cache.insert(src, dimmed.clone());
                        el.set_attribute("src", &dimmed).ok();
                    }
                    Ok(())
                }),
            ],
            ..RewriteStrSettings::new()
        },
    )
    .unwrap_or_else(|_| html.to_string());
    if !out.contains("background-color") {
        out = format!("<div style=\"background-color:{DARK_SURFACE};color:{DARK_INK}\">{out}</div>");
    }
    out
}

fn rewrite_style(style: &str) -> String {
    let mut kept = Vec::new();
    for decl in style.split(';') {
        let Some((name, value)) = decl.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if name == "background" || name == "background-color" || name == "background-image" {
            continue;
        }
        if name == "color" {
            kept.push(format!("color:{}", dark_ink(value)));
            continue;
        }
        if name == "border-color" || name == "border-left-color" || name == "border-right-color"
            || name == "border-top-color" || name == "border-bottom-color"
        {
            kept.push(format!("{name}:{DARK_BORDER}"));
            continue;
        }
        if name == "background" && value.contains("url(") {
            continue;
        }
        kept.push(format!("{name}:{value}"));
    }
    kept.join(";")
}

fn dark_ink(value: &str) -> String {
    let lower = value.trim().to_ascii_lowercase();
    if lower == "transparent" || lower == "inherit" || lower == "initial" || lower == "currentcolor" {
        return DARK_INK.to_string();
    }
    let Some((r, g, b)) = parse_hex(&lower) else {
        return DARK_INK.to_string();
    };
    let luma = (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0;
    if luma < 0.55 {
        DARK_INK.to_string()
    } else {
        lower
    }
}

fn parse_hex(value: &str) -> Option<(u8, u8, u8)> {
    let hex = value.strip_prefix('#')?;
    match hex.len() {
        3 => Some((
            u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?,
            u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?,
            u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?,
        )),
        6 => Some((
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        )),
        _ => None,
    }
}

fn dim_image_src(src: &str, cache_dir: Option<&Path>) -> Option<String> {
    let Some((mime, data)) = decode_image_data(src) else {
        return None;
    };
    let image = match image::load_from_memory(&data) {
        Ok(image) => image,
        Err(_) => {
            if let Some(dir) = cache_dir {
                let _ = std::fs::create_dir_all(dir);
                let digest = Sha256::digest(&data);
                let ext = if mime == "image/jpeg" { "jpg" } else { "png" };
                let path = dir.join(format!("{}.{}", hex(&digest), ext));
                if std::fs::write(&path, &data).is_ok() {
                    return Some(format!("file://{}", path.display()));
                }
            }
            let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
            return Some(format!("data:{mime};base64,{encoded}"));
        }
    };
    let mut image = image.to_rgba8();
    for pixel in image.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        let luma = (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0;
        if luma > 0.55 {
            pixel.0 = [
                (r as f32 * 0.72) as u8,
                (g as f32 * 0.72) as u8,
                (b as f32 * 0.72) as u8,
                a,
            ];
        }
    }
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    if let Some(dir) = cache_dir {
        let _ = std::fs::create_dir_all(dir);
        let digest = Sha256::digest(data);
        let name = format!("{}.png", hex(&digest));
        let path = dir.join(&name);
        if std::fs::write(&path, &out).is_ok() {
            return Some(format!("file://{}", path.display()));
        }
    }
    let encoded = base64::engine::general_purpose::STANDARD.encode(&out);
    Some(format!("data:{mime};base64,{encoded}"))
}

fn decode_image_data(src: &str) -> Option<(&'static str, Vec<u8>)> {
    if let Some(path) = src.strip_prefix("file://") {
        let bytes = std::fs::read(path).ok()?;
        let lower = path.to_ascii_lowercase();
        let mime = if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
            "image/jpeg"
        } else if lower.ends_with(".gif") {
            "image/gif"
        } else if lower.ends_with(".webp") {
            "image/webp"
        } else {
            "image/png"
        };
        return Some((mime, bytes));
    }
    let Some(rest) = src.strip_prefix("data:") else {
        return None;
    };
    let (meta, encoded) = rest.split_once(',')?;
    if !meta.contains(";base64") {
        return None;
    }
    let mime = match meta.split(';').next().unwrap_or_default() {
        "image/png" => "image/png",
        "image/jpeg" | "image/jpg" => "image/jpeg",
        "image/gif" => "image/gif",
        "image/webp" => "image/webp",
        _ => return None,
    };
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).ok()?;
    Some((mime, bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_bright_backgrounds_and_rewrites_black_text() {
        let out = darken_html(
            "<div style=\"background-color:#ffffff;color:#000000\">Hi</div>",
            false,
            None,
        );
        assert!(out.contains(DARK_INK));
        assert!(!out.contains("#ffffff"));
    }

    #[test]
    fn keeps_structural_style_and_links() {
        let out = darken_html(
            "<p style=\"font-size:14px\"><a href=\"https://example.com\">Link</a></p>",
            false,
            None,
        );
        assert!(out.contains("font-size:14px"));
        assert!(out.contains(DARK_LINK));
    }
}
