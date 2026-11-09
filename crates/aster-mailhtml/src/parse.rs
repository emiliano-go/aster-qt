// SPDX-License-Identifier: AGPL-3.0-or-later
//! A tolerant parser for the sanitised mail subset.

use html5ever::tendril::TendrilSink;
use html5ever::{parse_document, Attribute};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use rune::color::hex;
use rune::Color;

use crate::ir::{Align, Block, Cell, ImageBlock, Inline, InlineStyle, Table};

#[derive(Clone, Default)]
struct Ctx {
    style: InlineStyle,
    align: Align,
    href: Option<String>,
}

pub fn parse(html: &str) -> Vec<Block> {
    let dom = parse_document(RcDom::default(), Default::default()).one(html);
    let mut blocks = blocks_of(&dom.document, &Ctx::default());
    if blocks.is_empty() && !html.trim().is_empty() {
        // A document that parsed to nothing but text: keep it as content.
        let text = decode_text(html);
        if !text.trim().is_empty() {
            blocks.push(Block::Paragraph {
                runs: vec![Inline::Text {
                    text: text.trim().to_string(),
                    style: InlineStyle::default(),
                    href: None,
                }],
                align: Align::Start,
            });
        }
    }
    blocks
}

fn blocks_of(node: &Handle, ctx: &Ctx) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut para = Vec::new();
    for child in node.children.borrow().iter() {
        walk(child, ctx, &mut para, &mut blocks);
    }
    flush(&mut para, &mut blocks, ctx.align);
    blocks
}

fn walk(node: &Handle, ctx: &Ctx, para: &mut Vec<Inline>, blocks: &mut Vec<Block>) {
    match &node.data {
        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            push_text(para, &text, &ctx.style, ctx.href.as_deref());
        }
        NodeData::Element { name, attrs, .. } => {
            let tag: &str = name.local.as_ref();
            if tag == "script" || tag == "style" || tag == "head" || tag == "title" {
                return;
            }
            let mut next = ctx.clone();
            let raw = Attr::new(attrs);
            if raw
                .get("style")
                .is_some_and(|style| css(&style).display_none)
            {
                return;
            }
            apply_tag(tag, &raw, &mut next);
            if tag == "br" {
                para.push(Inline::Break);
                return;
            }
            if tag == "img" {
                if let Some(image) = image_of(&raw) {
                    para.push(Inline::Image(image));
                }
                return;
            }
            if tag == "hr" {
                flush(para, blocks, ctx.align);
                blocks.push(Block::Rule);
                return;
            }
            if tag == "table" {
                flush(para, blocks, ctx.align);
                if let Some(table) = table_of(node, &next) {
                    blocks.push(Block::Table(table));
                }
                return;
            }
            if tag == "pre" {
                flush(para, blocks, ctx.align);
                let text = collect_text(node);
                if !text.trim().is_empty() {
                    blocks.push(Block::Pre(text.trim_end().to_string()));
                }
                return;
            }
            if tag == "ul" || tag == "ol" {
                flush(para, blocks, ctx.align);
                blocks.push(list_of(node, &next, tag == "ol", &raw));
                return;
            }
            if is_block(tag) {
                flush(para, blocks, ctx.align);
                match tag {
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let level = tag[1..].parse::<u8>().unwrap_or(3);
                        let runs = inlines(node, &next);
                        if !runs.is_empty() {
                            blocks.push(Block::Heading {
                                level,
                                runs,
                                align: next.align,
                            });
                        }
                    }
                    "blockquote" => {
                        let inner = blocks_of(node, &next);
                        if !inner.is_empty() {
                            blocks.push(Block::Quote(inner));
                        }
                    }
                    _ => blocks.extend(blocks_of(node, &next)),
                }
                return;
            }
            for child in node.children.borrow().iter() {
                walk(child, &next, para, blocks);
            }
        }
        _ => {}
    }
}

fn inlines(node: &Handle, ctx: &Ctx) -> Vec<Inline> {
    let mut out = Vec::new();
    collect_inlines(node, ctx, &mut out);
    trim(&mut out);
    merge(&mut out);
    out
}

fn collect_inlines(node: &Handle, ctx: &Ctx, out: &mut Vec<Inline>) {
    for child in node.children.borrow().iter() {
        match &child.data {
            NodeData::Text { contents } => {
                let text = contents.borrow().to_string();
                push_text(out, &text, &ctx.style, ctx.href.as_deref());
            }
            NodeData::Element { name, attrs, .. } => {
                let tag: &str = name.local.as_ref();
                if tag == "br" {
                    out.push(Inline::Break);
                    continue;
                }
                if tag == "img" {
                    if let Some(image) = image_of(&Attr::new(attrs)) {
                        out.push(Inline::Image(image));
                    }
                    continue;
                }
                let raw = Attr::new(attrs);
                if raw
                    .get("style")
                    .is_some_and(|style| css(&style).display_none)
                {
                    continue;
                }
                let mut next = ctx.clone();
                apply_tag(tag, &raw, &mut next);
                collect_inlines(child, &next, out);
            }
            _ => {}
        }
    }
}

fn list_of(node: &Handle, ctx: &Ctx, ordered: bool, attrs: &Attr) -> Block {
    let mut items = Vec::new();
    for child in node.children.borrow().iter() {
        if let NodeData::Element { name, .. } = &child.data {
            let local: &str = name.local.as_ref();
            if local == "li" {
                let inner = blocks_of(child, ctx);
                if !inner.is_empty() {
                    items.push(inner);
                }
            }
        }
    }
    let start = attrs
        .get("start")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1);
    Block::List {
        ordered,
        start,
        items,
    }
}

fn table_of(node: &Handle, ctx: &Ctx) -> Option<Table> {
    let mut table = Table::default();
    collect_rows(node, ctx, &mut table);
    if table.rows.is_empty() {
        return None;
    }
    Some(table)
}

fn collect_rows(node: &Handle, ctx: &Ctx, table: &mut Table) {
    for child in node.children.borrow().iter() {
        let NodeData::Element { name, .. } = &child.data else {
            continue;
        };
        let local: &str = name.local.as_ref();
        match local {
            "thead" | "tbody" | "tfoot" => collect_rows(child, ctx, table),
            "tr" => {
                let mut cells = Vec::new();
                for cell in child.children.borrow().iter() {
                    let NodeData::Element { name, attrs, .. } = &cell.data else {
                        continue;
                    };
                    let tag: &str = name.local.as_ref();
                    if tag != "td" && tag != "th" {
                        continue;
                    }
                    let raw = Attr::new(attrs);
                    if raw
                        .get("style")
                        .is_some_and(|style| css(&style).display_none)
                    {
                        continue;
                    }
                    let mut cell_ctx = ctx.clone();
                    if let Some(align) = raw.get("align").and_then(|value| parse_align(&value)) {
                        cell_ctx.align = align;
                    }
                    if let Some(style) = raw.get("style") {
                        let css = css(&style);
                        if let Some(align) = css.text_align {
                            cell_ctx.align = align;
                        }
                    }
                    cells.push(Cell {
                        blocks: blocks_of(cell, &cell_ctx),
                        colspan: raw
                            .get("colspan")
                            .and_then(|value| value.parse::<usize>().ok())
                            .unwrap_or(1)
                            .max(1),
                        rowspan: raw
                            .get("rowspan")
                            .and_then(|value| value.parse::<usize>().ok())
                            .unwrap_or(1)
                            .max(1),
                        header: tag == "th",
                        align: cell_ctx.align,
                    });
                }
                if !cells.is_empty() {
                    if cells.iter().all(|cell| cell.header) {
                        table.header_rows += 1;
                    }
                    table.rows.push(cells);
                }
            }
            _ => {}
        }
    }
}

fn collect_text(node: &Handle) -> String {
    match &node.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        NodeData::Element { name, .. } => {
            let tag: &str = name.local.as_ref();
            if tag == "br" {
                return "\n".into();
            }
            let mut out = String::new();
            for child in node.children.borrow().iter() {
                out.push_str(&collect_text(child));
            }
            out
        }
        _ => String::new(),
    }
}

fn is_block(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "div"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "main"
            | "aside"
            | "center"
            | "figure"
            | "figcaption"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "blockquote"
            | "ul"
            | "ol"
            | "li"
            | "dl"
            | "dt"
            | "dd"
            | "table"
            | "pre"
            | "hr"
            | "form"
            | "fieldset"
    )
}

struct Attr(Vec<Attribute>);

impl Attr {
    fn new(attrs: &std::cell::RefCell<Vec<Attribute>>) -> Self {
        Attr(attrs.borrow().clone())
    }
    fn get(&self, name: &str) -> Option<String> {
        self.0
            .iter()
            .find(|attr| {
                let local: &str = attr.name.local.as_ref();
                local.eq_ignore_ascii_case(name)
            })
            .map(|attr| attr.value.to_string())
    }
}

fn apply_tag(tag: &str, attrs: &Attr, ctx: &mut Ctx) {
    match tag {
        "b" | "strong" => ctx.style.bold = true,
        "i" | "em" | "cite" => ctx.style.italic = true,
        "u" | "ins" => ctx.style.underline = true,
        "s" | "strike" | "del" => ctx.style.strike = true,
        "code" | "kbd" | "samp" | "tt" => {
            ctx.style.code = true;
            ctx.style.mono = true;
        }
        "small" => scale_size(&mut ctx.style, 0.85),
        "sub" => {
            ctx.style.sub = true;
            scale_size(&mut ctx.style, 0.75);
        }
        "sup" => {
            ctx.style.sup = true;
            scale_size(&mut ctx.style, 0.75);
        }
        "a" => {
            if let Some(href) = attrs.get("href").filter(|href| safe_url(href)) {
                ctx.href = Some(href);
            }
        }
        "font" => {
            if let Some(color) = attrs.get("color").and_then(|value| parse_color(&value)) {
                ctx.style.color = Some(color);
            }
            if let Some(size) = attrs.get("size").and_then(|value| legacy_font_size(&value)) {
                ctx.style.size = Some(size);
            }
            if let Some(face) = attrs.get("face") {
                if face.to_ascii_lowercase().contains("mono") {
                    ctx.style.mono = true;
                }
            }
        }
        "center" => ctx.align = Align::Center,
        _ => {}
    }
    if let Some(align) = attrs.get("align").and_then(|value| parse_align(&value)) {
        ctx.align = align;
    }
    if let Some(style) = attrs.get("style") {
        let css = css(&style);
        if let Some(color) = css.color {
            ctx.style.color = Some(color);
        }
        if let Some(color) = css.background {
            ctx.style.background = Some(color);
        }
        if let Some(size) = css.font_size {
            ctx.style.size = Some(size);
        }
        if let Some(weight) = css.font_weight {
            ctx.style.bold = weight >= 600.0;
        }
        if css.italic {
            ctx.style.italic = true;
        }
        if css.underline {
            ctx.style.underline = true;
        }
        if css.mono {
            ctx.style.mono = true;
        }
        if let Some(align) = css.text_align {
            ctx.align = align;
        }
    }
}

fn image_of(attrs: &Attr) -> Option<ImageBlock> {
    let src = attrs.get("src")?;
    if !safe_url(&src) {
        return None;
    }
    let width = attrs
        .get("width")
        .and_then(|value| parse_length(&value))
        .or_else(|| attrs.get("style").and_then(|style| css(&style).width));
    let height = attrs
        .get("height")
        .and_then(|value| parse_length(&value))
        .or_else(|| attrs.get("style").and_then(|style| css(&style).height));
    Some(ImageBlock {
        src,
        alt: attrs.get("alt").unwrap_or_default(),
        width,
        height,
    })
}

fn push_text(out: &mut Vec<Inline>, text: &str, style: &InlineStyle, href: Option<&str>) {
    let text = decode_text(text);
    if text.is_empty() {
        return;
    }
    if let Some(Inline::Text {
        text: last,
        style: last_style,
        href: last_href,
    }) = out.last_mut()
    {
        if last_style == style && last_href.as_deref() == href {
            last.push_str(&text);
            return;
        }
    }
    out.push(Inline::Text {
        text,
        style: style.clone(),
        href: href.map(str::to_string),
    });
}

fn flush(para: &mut Vec<Inline>, blocks: &mut Vec<Block>, align: Align) {
    let mut runs = std::mem::take(para);
    trim(&mut runs);
    merge(&mut runs);
    if !runs.is_empty() {
        blocks.push(Block::Paragraph { runs, align });
    }
}

fn trim(runs: &mut Vec<Inline>) {
    while let Some(Inline::Text { text, .. }) = runs.first_mut() {
        let trimmed = text.trim_start();
        if trimmed.is_empty() {
            runs.remove(0);
        } else {
            *text = trimmed.to_string();
            break;
        }
    }
    while let Some(Inline::Text { text, .. }) = runs.last_mut() {
        let trimmed = text.trim_end();
        if trimmed.is_empty() {
            runs.pop();
        } else {
            *text = trimmed.to_string();
            break;
        }
    }
}

fn merge(runs: &mut Vec<Inline>) {
    let mut out: Vec<Inline> = Vec::with_capacity(runs.len());
    for run in runs.drain(..) {
        match (&run, out.last_mut()) {
            (
                Inline::Text { text, style, href },
                Some(Inline::Text {
                    text: last,
                    style: last_style,
                    href: last_href,
                }),
            ) if style == last_style && href == last_href => last.push_str(text),
            _ => out.push(run),
        }
    }
    *runs = out;
}

fn scale_size(style: &mut InlineStyle, factor: f32) {
    let base = style.size.unwrap_or(15.0);
    style.size = Some(base * factor);
}

fn legacy_font_size(value: &str) -> Option<f32> {
    let size = value.trim().parse::<u8>().ok()?;
    Some(match size {
        1 => 10.0,
        2 => 13.0,
        3 => 16.0,
        4 => 18.0,
        5 => 24.0,
        6 => 32.0,
        _ => 40.0,
    })
}

fn parse_align(value: &str) -> Option<Align> {
    match value.trim().to_ascii_lowercase().as_str() {
        "center" | "middle" => Some(Align::Center),
        "right" | "end" => Some(Align::End),
        "left" | "start" | "justify" => Some(Align::Start),
        _ => None,
    }
}

fn parse_length(value: &str) -> Option<f32> {
    let value = value.trim().to_ascii_lowercase();
    let number = value
        .trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '%')
        .parse::<f32>()
        .ok()?;
    if value.ends_with('%') {
        None
    } else if value.ends_with("pt") {
        Some(number * 1.333)
    } else if value.ends_with("em") || value.ends_with("rem") {
        Some(number * 15.0)
    } else {
        Some(number)
    }
}

fn parse_color(value: &str) -> Option<Color> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(rest) = value.strip_prefix('#') {
        return hex(&format!("#{rest}"));
    }
    if let Some(rest) = value.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
        let parts: Vec<f32> = rest
            .split(',')
            .filter_map(|p| p.trim().parse::<f32>().ok())
            .collect();
        if parts.len() >= 3 {
            return Some(Color {
                r: parts[0] / 255.0,
                g: parts[1] / 255.0,
                b: parts[2] / 255.0,
                a: parts.get(3).copied().unwrap_or(1.0),
            });
        }
    }
    if let Some(rest) = value
        .strip_prefix("rgba(")
        .and_then(|v| v.strip_suffix(')'))
    {
        let parts: Vec<f32> = rest
            .split(',')
            .filter_map(|p| p.trim().parse::<f32>().ok())
            .collect();
        if parts.len() >= 3 {
            return Some(Color {
                r: parts[0] / 255.0,
                g: parts[1] / 255.0,
                b: parts[2] / 255.0,
                a: parts.get(3).copied().unwrap_or(1.0),
            });
        }
    }
    named_color(&value)
}

fn named_color(value: &str) -> Option<Color> {
    let (r, g, b) = match value {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "blue" => (0, 0, 255),
        "yellow" => (255, 255, 0),
        "orange" => (255, 165, 0),
        "purple" => (128, 0, 128),
        "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192),
        "maroon" => (128, 0, 0),
        "navy" => (0, 0, 128),
        "teal" => (0, 128, 128),
        "olive" => (128, 128, 0),
        "lime" => (0, 255, 0),
        "aqua" | "cyan" => (0, 255, 255),
        "fuchsia" | "magenta" => (255, 0, 255),
        _ => return None,
    };
    Some(Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    })
}

fn safe_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    !lower.starts_with("javascript:")
        && !lower.starts_with("vbscript:")
        && !lower.starts_with("data:text/html")
}

#[derive(Default)]
struct Css {
    color: Option<Color>,
    background: Option<Color>,
    font_size: Option<f32>,
    font_weight: Option<f32>,
    italic: bool,
    underline: bool,
    mono: bool,
    text_align: Option<Align>,
    display_none: bool,
    width: Option<f32>,
    height: Option<f32>,
}

fn css(style: &str) -> Css {
    let mut out = Css::default();
    for declaration in style.split(';') {
        let Some((name, value)) = declaration.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim().to_ascii_lowercase();
        match name.as_str() {
            "color" => out.color = parse_color(&value),
            "background" | "background-color" => out.background = parse_color(&value),
            "font-size" => out.font_size = parse_length(&value),
            "font-weight" => {
                out.font_weight = match value.as_str() {
                    "bold" | "bolder" => Some(700.0),
                    "normal" | "lighter" => Some(400.0),
                    other => other.parse::<f32>().ok(),
                }
            }
            "font-style" => out.italic = value == "italic" || value == "oblique",
            "text-decoration" | "text-decoration-line" => {
                out.underline = value.contains("underline");
            }
            "font-family" => out.mono = value.contains("mono") || value.contains("courier"),
            "text-align" => out.text_align = parse_align(&value),
            "display" => out.display_none = value == "none",
            "width" => out.width = parse_length(&value),
            "height" => out.height = parse_length(&value),
            _ => {}
        }
    }
    out
}

fn decode_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find('&') {
        out.push_str(&rest[..index]);
        rest = &rest[index..];
        let Some(end) = rest.find(';').filter(|end| *end <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{00a0}'),
            "copy" => Some('\u{00a9}'),
            "reg" => Some('\u{00ae}'),
            "trade" => Some('\u{2122}'),
            "hellip" => Some('\u{2026}'),
            "mdash" => Some('\u{2014}'),
            "ndash" => Some('\u{2013}'),
            "lsquo" => Some('\u{2018}'),
            "rsquo" => Some('\u{2019}'),
            "ldquo" => Some('\u{201c}'),
            "rdquo" => Some('\u{201d}'),
            "bull" => Some('\u{2022}'),
            "middot" => Some('\u{00b7}'),
            "euro" => Some('\u{20ac}'),
            "pound" => Some('\u{00a3}'),
            "yen" => Some('\u{00a5}'),
            _ => entity
                .strip_prefix('#')
                .and_then(|number| {
                    if let Some(hex) = number
                        .strip_prefix('x')
                        .or_else(|| number.strip_prefix('X'))
                    {
                        u32::from_str_radix(hex, 16).ok()
                    } else {
                        number.parse::<u32>().ok()
                    }
                })
                .and_then(char::from_u32),
        };
        match decoded {
            Some(ch) => {
                out.push(ch);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_paragraphs_and_inline_styles() {
        let blocks = parse("<p>Hello <b>world</b> &amp; <a href=\"https://x.test\">link</a></p>");
        let Block::Paragraph { runs, .. } = &blocks[0] else {
            panic!("expected a paragraph, got {blocks:?}");
        };
        assert_eq!(runs.len(), 4);
        assert!(matches!(&runs[1], Inline::Text { style, .. } if style.bold));
        assert!(
            matches!(&runs[3], Inline::Text { href: Some(href), .. } if href == "https://x.test")
        );
    }

    #[test]
    fn parses_lists_and_tables() {
        let blocks = parse("<ul><li>one</li><li>two</li></ul><table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>");
        assert!(
            matches!(&blocks[0], Block::List { ordered: false, items, .. } if items.len() == 2)
        );
        let Block::Table(table) = &blocks[1] else {
            panic!("expected a table, got {blocks:?}");
        };
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.header_rows, 1);
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(
            decode_text("a &amp; b &#8212; c &nbsp;d"),
            "a & b \u{2014} c \u{00a0}d"
        );
    }
}
