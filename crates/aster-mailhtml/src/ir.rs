// SPDX-License-Identifier: AGPL-3.0-or-later
//! The intermediate representation the parser builds and the renderer paints.

use rune::Color;

pub use rune::Align;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub code: bool,
    pub sub: bool,
    pub sup: bool,
    pub color: Option<Color>,
    pub background: Option<Color>,
    pub size: Option<f32>,
    pub mono: bool,
}

#[derive(Clone, Debug)]
pub enum Inline {
    Text {
        text: String,
        style: InlineStyle,
        href: Option<String>,
    },
    Image(ImageBlock),
    Break,
}

#[derive(Clone, Debug)]
pub struct ImageBlock {
    pub src: String,
    pub alt: String,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

#[derive(Clone, Debug)]
pub enum Block {
    Paragraph {
        runs: Vec<Inline>,
        align: Align,
    },
    Heading {
        level: u8,
        runs: Vec<Inline>,
        align: Align,
    },
    List {
        ordered: bool,
        start: u64,
        items: Vec<Vec<Block>>,
    },
    Quote(Vec<Block>),
    Pre(String),
    Table(Table),
    Rule,
    Image(ImageBlock),
}

#[derive(Clone, Debug, Default)]
pub struct Table {
    pub rows: Vec<Vec<Cell>>,
    pub header_rows: usize,
}

#[derive(Clone, Debug)]
pub struct Cell {
    pub blocks: Vec<Block>,
    pub colspan: usize,
    pub rowspan: usize,
    pub header: bool,
    pub align: Align,
}
