use arrayvec::ArrayString;
use crossterm::style::{Attributes, Color};

use super::DrawOp;

/// A character in the terminal screen.
#[derive(Clone, Copy, PartialEq, Debug)]
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
            width,
            height,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn clear(&mut self) {
        for graph in &mut self.graphs {
            *graph = Grapheme::blank();
        }
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
        for ch in string.chars() {
            self.set_char_with_attrs(y, x, ch, fg, bg, attrs);
        }
    }

    pub fn set_char(&mut self, y: usize, x: usize, ch: char) {
        self.set_char_with_attrs(y, x, ch, Color::Reset, Color::Reset, Default::default());
    }

    pub fn set_str(&mut self, y: usize, x: usize, string: &str) {
        self.set_str_with_attrs(y, x, string, Color::Reset, Color::Reset, Default::default());
    }

    pub fn set_fg(&mut self, y: usize, x: usize, y_end: usize, x_end: usize, fg: Color) {
        self.update_range(y, x, y_end, x_end, |graph| graph.fg = fg);
    }

    pub fn set_bg(&mut self, y: usize, x: usize, y_end: usize, x_end: usize, bg: Color) {
        self.update_range(y, x, y_end, x_end, |graph| graph.bg = bg);
    }

    pub fn add_attrs(&mut self, y: usize, x: usize, y_end: usize, x_end: usize, attrs: Attributes) {
        self.update_range(y, x, y_end, x_end, |graph| graph.attrs.extend(attrs));
    }

    pub fn update_range<F>(&mut self, y: usize, x: usize, y_end: usize, x_end: usize, f: F)
    where
        F: Fn(&mut Grapheme),
    {
        debug_assert!(y <= y_end);
        debug_assert!(x <= x_end);
        debug_assert!(y_end < self.height);
        debug_assert!(x_end < self.width);

        for y in y..y_end {
            for x in x..x_end {
                f(&mut self.graphs[y * self.width + x]);
            }
        }
    }

    pub fn copy_from_other(&mut self, y: usize, x: usize, other: &Canvas) {
        debug_assert!(y < self.height);
        debug_assert!(x < self.width);
        debug_assert!(y + other.height <= self.height);
        debug_assert!(x + other.width <= self.width);

        for y_off in 0..other.height() {
            let dst_start = (y + y_off) * self.width + x;
            let dst_end = dst_start + other.width;
            let src_start = y_off * other.width;
            let src_end = src_start + other.width;
            (&mut self.graphs[dst_start..dst_end])
                .copy_from_slice(&other.graphs[src_start..src_end]);
        }
    }

    pub fn compute_draw_updates<'a, 'b>(&'a self, other: &'b Canvas) -> Vec<DrawOp<'a>> {
        debug_assert_eq!(self.width(), other.width());
        debug_assert_eq!(self.height(), other.height());

        let mut y = 0;
        let mut x = 0;
        let mut fg = Color::Reset;
        let mut bg = Color::Reset;
        let mut attrs = Attributes::default();
        let mut needs_move = false;
        let mut ops = Vec::with_capacity(self.width() * self.height());
        for (new, old) in self.graphs.iter().zip(&other.graphs) {
            if old == new {
                needs_move = true;
            } else {
                if needs_move {
                    ops.push(DrawOp::MoveTo { y, x });
                    needs_move = false;
                }

                if new.fg != fg {
                    ops.push(DrawOp::FgColor(new.fg));
                    fg = new.fg;
                }

                if new.bg != bg {
                    ops.push(DrawOp::BgColor(new.bg));
                    bg = new.bg;
                }

                if new.attrs != attrs {
                    ops.push(DrawOp::Attributes(new.attrs));
                    attrs = new.attrs;
                }

                ops.push(DrawOp::Grapheme(&new.grapheme));
            }

            x += 1;
            if x >= self.width {
                y += 1;
                x = 0;
            }
        }

        ops
    }
}
