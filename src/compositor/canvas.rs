use arrayvec::ArrayString;
use noa_common::logger::{self, backtrace};

pub use crossterm::style::Color;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DrawOp {
    MoveTo { y: usize, x: usize },
    Grapheme(ArrayString<8>),
    FgColor(Color),
    BgColor(Color),
    Bold,
    NoBold,
    Invert,
    NoInvert,
    Underline,
    NoUnderline,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub deco: Decoration,
}

impl Style {
    #[must_use]
    pub fn merge(mut self, other: Style) -> Self {
        if other.fg != Color::Reset {
            self.fg = other.fg;
        }

        if other.bg != Color::Reset {
            self.bg = other.bg;
        }

        self.deco.bold |= other.deco.bold;
        self.deco.underline |= other.deco.underline;
        self.deco.inverted |= other.deco.inverted;

        self
    }
}

impl Default for Style {
    fn default() -> Self {
        Style {
            fg: Color::Reset,
            bg: Color::Reset,
            deco: Decoration::default(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Decoration {
    pub bold: bool,
    pub inverted: bool,
    pub underline: bool,
}

impl Decoration {
    pub const fn empty() -> Decoration {
        Decoration {
            bold: false,
            inverted: false,
            underline: false,
        }
    }

    pub const fn bold() -> Decoration {
        Decoration {
            bold: true,
            inverted: false,
            underline: false,
        }
    }

    pub const fn underline() -> Decoration {
        Decoration {
            bold: false,
            inverted: false,
            underline: true,
        }
    }

    pub const fn inverted() -> Decoration {
        Decoration {
            bold: false,
            inverted: true,
            underline: false,
        }
    }
}

/// A character in the terminal screen.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Grapheme {
    /// The character. It can be larger than 1 if it consists of multiple unicode
    /// characters like A with the acute accent.
    pub chars: ArrayString<8>,
    pub style: Style,
}

impl Grapheme {
    pub fn new(grapheme: &str) -> Grapheme {
        Grapheme {
            chars: ArrayString::from(grapheme).unwrap(),
            style: Default::default(),
        }
    }

    pub fn blank() -> Grapheme {
        Grapheme::new(" ")
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

    pub fn copy_from_other(&mut self, y: usize, x: usize, other: &Canvas) {
        let in_bounds = y < self.height
            && x < self.width
            && y + other.height <= self.height
            && x + other.width <= self.width;

        if !in_bounds {
            warn!(
                "out of bounds copy: dst_size=({}, {}), dst_pos=({}, {}), src_size=({}, {})",
                self.height, self.width, y, x, other.height, other.width,
            );
            backtrace();
            return;
        }

        debug_assert!(y < self.height);
        debug_assert!(x < self.width);
        debug_assert!(y + other.height <= self.height);
        debug_assert!(x + other.width <= self.width);

        for y_off in 0..other.height() {
            let dst_start = (y + y_off) * self.width + x;
            let dst_end = dst_start + other.width;
            let src_start = y_off * other.width;
            let src_end = src_start + other.width;
            self.graphs[dst_start..dst_end].copy_from_slice(&other.graphs[src_start..src_end]);
        }
    }

    pub fn compute_draw_updates(&self, other: &Canvas) -> Vec<DrawOp> {
        debug_assert_eq!(self.width(), other.width());
        debug_assert_eq!(self.height(), other.height());

        let mut y = 0;
        let mut x = 0;
        let mut fg = Color::Reset;
        let mut bg = Color::Reset;
        let mut deco = Decoration::default();
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

                if new.style.fg != fg {
                    ops.push(DrawOp::FgColor(new.style.fg));
                    fg = new.style.fg;
                }

                if new.style.bg != bg {
                    ops.push(DrawOp::BgColor(new.style.bg));
                    bg = new.style.bg;
                }

                if new.style.deco != deco {
                    if new.style.deco.bold != deco.bold {
                        ops.push(if new.style.deco.bold {
                            DrawOp::Bold
                        } else {
                            DrawOp::NoBold
                        });
                    }

                    if new.style.deco.inverted != deco.inverted {
                        ops.push(if new.style.deco.inverted {
                            DrawOp::Invert
                        } else {
                            DrawOp::NoInvert
                        });
                    }

                    if new.style.deco.underline != deco.underline {
                        ops.push(if new.style.deco.underline {
                            DrawOp::Underline
                        } else {
                            DrawOp::NoUnderline
                        });
                    }
                    deco = new.style.deco;
                }

                ops.push(DrawOp::Grapheme(new.chars));
            }

            x += 1;
            if x >= self.width {
                y += 1;
                x = 0;
            }
        }

        ops
    }

    pub fn view_mut(&mut self) -> CanvasViewMut<'_> {
        CanvasViewMut {
            graphs: &mut self.graphs,
            canvas_width: self.width,
            y: 0,
            x: 0,
            width: self.width,
            height: self.height,
        }
    }
}

/// A part of rectangle a canvas.
pub struct CanvasViewMut<'a> {
    graphs: &'a mut [Grapheme],
    canvas_width: usize,
    y: usize,
    x: usize,
    width: usize,
    height: usize,
}

impl<'a> CanvasViewMut<'a> {
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn clear(&mut self) {
        for graph in self.graphs.iter_mut() {
            *graph = Grapheme::blank();
        }
    }

    pub fn write(&mut self, y: usize, x: usize, graph: Grapheme) {
        let in_bounds = y < self.height && x < self.width;
        if !in_bounds {
            warn!(
                "out of bounds draw: (y, x) = ({}, {}), (height, width) = ({}, {})",
                y, x, self.height, self.width,
            );
            backtrace();
            return;
        }

        if graph.chars.contains('\n') {
            warn!("tried to draw '\\n'");
            logger::backtrace();
            return;
        }

        let index = (self.y + y) * self.canvas_width + self.x + x;
        self.graphs[index] = Grapheme {
            chars: graph.chars,
            style: self.graphs[index].style.merge(graph.style),
        };
    }

    pub fn write_char(&mut self, y: usize, x: usize, ch: char) {
        self.write_char_with_style(y, x, ch, Style::default());
    }

    pub fn write_char_with_style(&mut self, y: usize, x: usize, ch: char, style: Style) {
        let mut chars = ArrayString::new();
        chars.push(ch);
        self.write(y, x, Grapheme { chars, style })
    }

    pub fn write_str(&mut self, y: usize, x: usize, string: &str) {
        self.write_str_with_style(y, x, string, Style::default());
    }

    pub fn write_str_with_style(&mut self, y: usize, x: usize, string: &str, style: Style) {
        for (i, ch) in string.chars().enumerate() {
            self.write_char_with_style(y, x + i, ch, style);
        }
    }

    pub fn set_decoration(&mut self, y: usize, x: usize, x_end: usize, deco: Decoration) {
        self.update_range(y, x, y + 1, x_end, |graph| graph.style.deco = deco);
    }

    pub fn apply_style(&mut self, y: usize, x: usize, x_end: usize, style: Style) {
        self.update_range(y, x, y + 1, x_end, |graph| {
            graph.style.fg = style.fg;
            graph.style.bg = style.bg;
            graph.style.deco = style.deco;
        });
    }

    pub fn update_range<F>(&mut self, y: usize, x: usize, y_end: usize, x_end: usize, f: F)
    where
        F: Fn(&mut Grapheme),
    {
        let in_bounds = y <= y_end && x <= x_end && y_end <= self.height && x_end <= self.width;
        if !in_bounds {
            warn!(
                "out of bounds update_range: (y, x) = ({}-{}, {}-{}), (height, width) = ({}, {})",
                y, y_end, x, x_end, self.height, self.width,
            );
            backtrace();
            return;
        }

        for y in y..y_end {
            for x in x..x_end {
                f(&mut self.graphs[(self.y + y) * self.canvas_width + self.x + x]);
            }
        }
    }
}
