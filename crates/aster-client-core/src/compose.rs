// SPDX-License-Identifier: AGPL-3.0-or-later
//! Markdown to email compatible HTML and plain text.

use pulldown_cmark::{html, Event, Options, Parser, Tag, TagEnd};

pub fn markdown_to_email(markdown: &str) -> (String, String) {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut html_out = String::new();
    html::push_html(&mut html_out, Parser::new_ext(markdown, options));
    let html_body = format!(
        "<div style=\"font-family:Inter,system-ui,sans-serif;font-size:15px;line-height:1.5;color:#1c1b19\">{}</div>",
        html_out
    );
    let plain = events_to_text(Parser::new_ext(markdown, options));
    (plain, html_body)
}

fn events_to_text(parser: Parser<'_>) -> String {
    let mut out = String::new();
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut link_target: Option<String> = None;
    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {}
                Tag::Heading { .. } => {
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                Tag::List(start) => list_stack.push(start),
                Tag::Item => {
                    out.push('\n');
                    let depth = list_stack.len().saturating_sub(1);
                    out.push_str(&"  ".repeat(depth));
                    if let Some(Some(n)) = list_stack.last_mut() {
                        out.push_str(&format!("{n}. "));
                        *n += 1;
                    } else {
                        out.push_str("- ");
                    }
                }
                Tag::BlockQuote(_) => {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str("> ");
                }
                Tag::CodeBlock(_) => {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                Tag::Link { dest_url, .. } => link_target = Some(dest_url.to_string()),
                Tag::Table(_) | Tag::TableHead | Tag::TableRow => {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                Tag::TableCell => out.push_str(" | "),
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph | TagEnd::Heading(_) => out.push_str("\n\n"),
                TagEnd::List(_) => {
                    list_stack.pop();
                    out.push('\n');
                }
                TagEnd::Item => {}
                TagEnd::CodeBlock => out.push_str("\n"),
                TagEnd::Link => {
                    if let Some(target) = link_target.take() {
                        out.push_str(&format!(" ({target})"));
                    }
                }
                TagEnd::Table | TagEnd::TableHead | TagEnd::TableRow => out.push('\n'),
                _ => {}
            },
            Event::Text(text) => out.push_str(&text),
            Event::Code(code) => {
                out.push('`');
                out.push_str(&code);
                out.push('`');
            }
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push('\n'),
            Event::Rule => {
                out.push_str("\n---\n");
            }
            _ => {}
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_round_trip() {
        let (plain, html) = markdown_to_email("# Hi\n\nThis is **bold** and a [link](https://example.com).\n\n- one\n- two\n\n```\ncode\n```");
        assert!(plain.contains("Hi"));
        assert!(plain.contains("bold"));
        assert!(plain.contains("one"));
        assert!(html.contains("<h1>Hi</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("href=\"https://example.com\""));
        assert!(html.contains("<li>one</li>"));
    }
}
