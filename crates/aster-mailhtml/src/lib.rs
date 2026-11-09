// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sanitised HTML mail → Rune elements.
//!
//! Input is the output of `aster-client-core`'s `safe_body_html`: the ammonia allowlist
//! (semantic tags, inline CSS inlined by `css-inline`) with `file:`/`data:` images.
//! The renderer covers that subset; unknown tags and CSS are dropped, not guessed.

mod ir;
mod parse;
mod render;

pub use ir::{Align, Block, Cell, Inline, InlineStyle, Table};
pub use render::{render, MailStyle};

#[cfg(test)]
mod render_tests;
