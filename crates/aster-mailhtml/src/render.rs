// SPDX-License-Identifier: AGPL-3.0-or-later
//! Paints the parsed mail subset with Rune elements.

use rune::{col, el, rich, row, text, Color, El, ImageCache, Link, Span, TextStyle, Tx};

use crate::ir::{Align, Block, Cell, ImageBlock, Inline, InlineStyle, Table};
use crate::parse;

#[derive(Clone, Copy, Debug)]
pub struct MailStyle {
    pub text: Color,
    pub muted: Color,
    pub link: Color,
    pub code_bg: Color,
    pub quote_border: Color,
    pub rule: Color,
    pub surface: Color,
    pub body_size: f32,
}

pub fn render<S: 'static>(
    html: &str,
    style: &MailStyle,
    images: Option<&ImageCache>,
    scale: f32,
    open: fn(&mut S, &str),
) -> El {
    let blocks = parse::parse(html);
    blocks_el(&blocks, style, images, scale, open, 400.0)
}

fn blocks_el<S: 'static>(
    blocks: &[Block],
    style: &MailStyle,
    images: Option<&ImageCache>,
    scale: f32,
    open: fn(&mut S, &str),
    weight: f32,
) -> El {
    col().w_full().min_w(0.0).gap(10.0).children(
        blocks
            .iter()
            .map(|block| block_el(block, style, images, scale, open, weight)),
    )
}

fn block_el<S: 'static>(
    block: &Block,
    style: &MailStyle,
    images: Option<&ImageCache>,
    scale: f32,
    open: fn(&mut S, &str),
    weight: f32,
) -> El {
    match block {
        Block::Paragraph { runs, align } => {
            paragraph(runs, *align, style, images, scale, open, weight)
        }
        Block::Heading { level, runs, align } => {
            let size = match level {
                1 => 26.0,
                2 => 22.0,
                3 => 19.0,
                4 => 17.0,
                5 => 16.0,
                _ => 15.0,
            };
            text_runs(runs, style, size, 600.0, *align, open)
        }
        Block::List {
            ordered,
            start,
            items,
        } => {
            let mut rows = Vec::with_capacity(items.len());
            for (index, item) in items.iter().enumerate() {
                let marker = if *ordered {
                    format!("{}.", start + index as u64)
                } else {
                    "•".to_string()
                };
                rows.push(
                    row()
                        .w_full()
                        .gap(8.0)
                        .child(
                            text(marker)
                                .font(TextStyle::new(style.body_size, 400.0))
                                .color(style.muted)
                                .w(22.0),
                        )
                        .child(blocks_el(item, style, images, scale, open, weight).grow(1.0)),
                );
            }
            col().w_full().gap(4.0).children(rows)
        }
        Block::Quote(inner) => row()
            .w_full()
            .gap(10.0)
            .child(
                row()
                    .w(3.0)
                    .fill(style.quote_border)
                    .radius(2.0)
                    .self_stretch(),
            )
            .child(blocks_el(inner, style, images, scale, open, weight).grow(1.0)),
        Block::Pre(code) => col()
            .w_full()
            .padding(10.0)
            .radius(4.0)
            .fill(style.code_bg)
            .child(
                text(code.as_str())
                    .font(TextStyle::new(style.body_size - 1.0, 400.0).mono())
                    .color(style.text)
                    .wrap_anywhere(),
            ),
        Block::Table(table) => table_el(table, style, images, scale, open),
        Block::Rule => el().w_full().h(1.0).fill(style.rule),
        Block::Image(image) => image_el(image, style, images, scale),
    }
}

fn paragraph<S: 'static>(
    runs: &[Inline],
    align: Align,
    style: &MailStyle,
    images: Option<&ImageCache>,
    scale: f32,
    open: fn(&mut S, &str),
    weight: f32,
) -> El {
    if !runs.iter().any(|run| matches!(run, Inline::Image(_))) {
        return text_runs(runs, style, style.body_size, weight, align, open);
    }
    let mut out = col().w_full().gap(8.0);
    let mut chunk = Vec::new();
    for run in runs {
        match run {
            Inline::Image(image) => {
                if !chunk.is_empty() {
                    out = out.child(text_runs(
                        &chunk,
                        style,
                        style.body_size,
                        400.0,
                        align,
                        open,
                    ));
                    chunk.clear();
                }
                out = out.child(image_el(image, style, images, scale));
            }
            other => chunk.push(other.clone()),
        }
    }
    if !chunk.is_empty() {
        out = out.child(text_runs(
            &chunk,
            style,
            style.body_size,
            400.0,
            align,
            open,
        ));
    }
    out
}

fn text_runs<S: 'static>(
    runs: &[Inline],
    style: &MailStyle,
    size: f32,
    weight: f32,
    align: Align,
    open: fn(&mut S, &str),
) -> El {
    let mut content = String::new();
    let mut spans = Vec::new();
    let mut links = Vec::new();
    for run in runs {
        match run {
            Inline::Text {
                text: value,
                style: run_style,
                href,
            } => {
                let start = content.len();
                content.push_str(value);
                let end = content.len();
                if end > start {
                    if let Some(span) = span_for(start, end, run_style, style, href.is_some()) {
                        spans.push(span);
                    }
                    if let Some(href) = href {
                        links.push((start..end, href.clone()));
                    }
                }
            }
            Inline::Break => content.push('\n'),
            Inline::Image(image) => {
                content.push_str(if image.alt.is_empty() {
                    "[image]"
                } else {
                    image.alt.as_str()
                });
            }
        }
    }
    let mut e = rich(content, spans)
        .font(TextStyle::new(size, weight))
        .color(style.text)
        .align(align)
        .wrap()
        .selectable(true);
    for (range, href) in links {
        e = e.link(Link::new(range, href));
    }
    e.on_link(move |s: &mut S, id: &str, _: &mut Tx| open(s, id))
}

fn span_for(
    start: usize,
    end: usize,
    run: &InlineStyle,
    style: &MailStyle,
    link: bool,
) -> Option<Span> {
    let mut span = Span::new(start, end);
    let mut any = false;
    if run.bold {
        span = span.bold();
        any = true;
    }
    if run.italic {
        span = span.italic();
        any = true;
    }
    if run.mono || run.code {
        span = span.mono();
        any = true;
    }
    if let Some(size) = run.size {
        span = span.size(size);
        any = true;
    }
    if run.code {
        span = span.bg(style.code_bg, 3.0);
        any = true;
    }
    let ink = if link {
        style.link
    } else {
        run.color.unwrap_or(style.text)
    };
    if ink != style.text {
        span = span.color(ink);
        any = true;
    }
    if run.underline {
        span = span.underline(ink);
        any = true;
    }
    any.then_some(span)
}

fn table_el<S: 'static>(
    table: &Table,
    style: &MailStyle,
    images: Option<&ImageCache>,
    scale: f32,
    open: fn(&mut S, &str),
) -> El {
    let mut rows = Vec::with_capacity(table.rows.len());
    for (index, cells) in table.rows.iter().enumerate() {
        let header = index < table.header_rows;
        rows.push(
            row().w_full().min_w(0.0).children(
                cells
                    .iter()
                    .map(|cell| cell_el(cell, header, style, images, scale, open)),
            ),
        );
    }
    col().w_full().min_w(0.0).children(rows)
}

fn cell_el<S: 'static>(
    cell: &Cell,
    header: bool,
    style: &MailStyle,
    images: Option<&ImageCache>,
    scale: f32,
    open: fn(&mut S, &str),
) -> El {
    let weight = cell.colspan.max(1) as f32;
    let content = blocks_el(
        &cell.blocks,
        style,
        images,
        scale,
        open,
        if header { 600.0 } else { 400.0 },
    );
    col()
        .grow(weight)
        .min_w(0.0)
        .padding(6.0)
        .gap(4.0)
        .border(1.0, style.rule)
        .fill(if header {
            style.surface
        } else {
            Color::TRANSPARENT
        })
        .child(content)
}

fn image_el(image: &ImageBlock, style: &MailStyle, images: Option<&ImageCache>, scale: f32) -> El {
    let label = if image.alt.is_empty() {
        "Image".to_string()
    } else {
        image.alt.clone()
    };
    let Some(cache) = images else {
        return image_placeholder(&label, style);
    };
    let picture = if let Some(url) = image.src.strip_prefix("file://") {
        cache.file(url, 640.0, scale)
    } else if let Some(url) = image.src.strip_prefix("file:") {
        cache.file(url, 640.0, scale)
    } else if let Some(rest) = image.src.strip_prefix("data:") {
        let Some((meta, data)) = rest.split_once(',') else {
            return image_placeholder(&label, style);
        };
        let mime = meta.split(';').next().unwrap_or("application/octet-stream");
        let bytes = if meta.contains(";base64") {
            use base64::Engine as _;
            match base64::engine::general_purpose::STANDARD.decode(data) {
                Ok(bytes) => bytes,
                Err(_) => return image_placeholder(&label, style),
            }
        } else {
            data.as_bytes().to_vec()
        };
        cache.load_mime(bytes, mime, 640.0, scale)
    } else {
        return image_placeholder(&label, style);
    };
    match picture.wait() {
        Ok([width, _]) if width > 0 => {
            let shown = image.width.unwrap_or(width as f32).clamp(16.0, 880.0);
            el().role("image")
                .label(label)
                .w(shown)
                .pixels(picture.pixels())
                .radius(4.0)
        }
        _ => image_placeholder(&label, style),
    }
}

fn image_placeholder(label: &str, style: &MailStyle) -> El {
    row()
        .role("image")
        .label(label)
        .w_full()
        .padding(8.0)
        .radius(4.0)
        .fill(style.surface)
        .child(
            text(label)
                .font(TextStyle::new(style.body_size - 1.0, 400.0))
                .color(style.muted)
                .wrap(),
        )
}
