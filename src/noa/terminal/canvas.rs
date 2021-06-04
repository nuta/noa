use anyhow::Result;
use arrayvec::ArrayString;
use crossterm::style::{Attributes, Color};

use super::DrawOp;

/// A character in the terminal screen.
#[derive(Clone, Copy, Debug)]
pub struct Grapheme {
    /// The character. It can be larger than 1 if it consists of multiple unicode
    /// characters like A with the acute accent.
    grapheme: ArrayString<4>,
    fg: Color,
    bg: Color,
    attrs: Attributes,
}

impl Grapheme {
    pub fn blank() -> Grapheme {
        Grapheme {
            grapheme: ArrayString::from(" ").unwrap(),
            fg: Color::Reset,
            bg: Color::Reset,
            attrs: Default::default(),
        }
    }
}

/// A rectangle filled with characters.
pub struct Canvas {
    /// Contains `height * width` items.
    graphs: Vec<Grapheme>,
    /// The number of characters in a screen column.
    width: usize,
    /// The number of lines in the screen.
    height: usize,
}

impl Canvas {
    pub fn new(height: usize, width: usize) -> Canvas {
        let mut graphs = Vec::with_capacity(height * width);
        for _ in 0..(height * width) {
            graphs.push(Grapheme::blank());
        }

        Canvas {
            graphs,
            height,
            width,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn set_grapheme(&mut self, y: usize, x: usize, graph: Grapheme) {
        debug_assert!(y < self.height);
        debug_assert!(x < self.width);

        self.graphs[y * self.width + x] = graph;
    }

    pub fn set_char_with_attrs(
        &mut self,
        y: usize,
        x: usize,
        ch: char,
        fg: Color,
        bg: Color,
        attrs: Attributes,
    ) {
        let mut grapheme = ArrayString::new();
        grapheme.push(ch);

        self.set_grapheme(
            y,
            x,
            Grapheme {
                grapheme,
                fg,
                bg,
                attrs,
            },
        )
    }

    pub fn set_str_with_attrs(
        &mut self,
        y: usize,
        x: usize,
        string: &str,
        fg: Color,
        bg: Color,
        attrs: Attributes,
    ) {
    }

    pub fn set_char(&mut self, y: usize, x: usize, ch: char) {
        self.set_char_with_attrs(y, x, ch, Color::Reset, Color::Reset, Default::default());
    }
    pub fn set_str(&mut self, y: usize, x: usize, string: &str) {
        self.set_str_with_attrs(y, x, string, Color::Reset, Color::Reset, Default::default());
    }

    pub fn copy_from_other(&mut self, y: usize, x: usize, other: &Canvas) {
        debug_assert!(y < self.height);
        debug_assert!(x < self.width);
        debug_assert!(y + other.height < self.height);
        debug_assert!(x + other.width < self.width);
        let start = y * self.width + x;
        let end = (y + other.height) * self.width + (x + other.width);
        (&mut self.graphs[start..end]).copy_from_slice(&other.graphs[..]);
    }

    pub fn compute_draw_updates<'a, 'b>(&'a self, other: &'b Canvas) -> DrawUpdates<'a, 'b> {
        DrawUpdates {
            index: 0,
            prev: self,
            next: other,
        }
    }
}

pub struct DrawUpdates<'a, 'b> {
    index: usize,
    prev: &'a Canvas,
    next: &'b Canvas,
}

impl<'a, 'b> Iterator for DrawUpdates<'a, 'b> {
    type Item = DrawOp<'b>;
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}
