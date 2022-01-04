use arrayvec::ArrayString;
use crossterm::style::Color;
use noa_common::logger::backtrace;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DrawOp<'a> {
    MoveTo { y: usize, x: usize },
    Grapheme(&'a str),
    FgColor(Color),
    BgColor(Color),
    Bold,
    NoBold,
}

#[derive(Clone, Debug)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub deco: Decoration,
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
    grapheme: ArrayString<4>,
    fg: Color,
    bg: Color,
    deco: Decoration,
}

impl Grapheme {
    pub fn new(grapheme: &str) -> Grapheme {
        Grapheme {
            grapheme: ArrayString::from(grapheme).unwrap(),
            fg: Color::Reset,
            bg: Color::Reset,
            deco: Default::default(),
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

                if new.fg != fg {
                    ops.push(DrawOp::FgColor(new.fg));
                    fg = new.fg;
                }

                if new.bg != bg {
                    ops.push(DrawOp::BgColor(new.bg));
                    bg = new.bg;
                }

                if new.deco != deco {
                    if new.deco.bold != deco.bold {
                        ops.push(if new.deco.bold {
                            DrawOp::Bold
                        } else {
                            DrawOp::NoBold
                        });
                    }
                    deco = new.deco;
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

    pub fn set_grapheme(&mut self, y: usize, x: usize, graph: Grapheme) {
        let in_bounds = y < self.height && x < self.width;
        if !in_bounds {
            warn!(
                "out of bounds draw: (y, x) = ({}, {}), (height, width) = ({}, {})",
                y, x, self.height, self.width,
            );
            backtrace();
            return;
        }

        self.graphs[(self.y + y) * self.canvas_width + self.x + x] = graph;
    }

    pub fn set_char_with_attrs(
        &mut self,
        y: usize,
        x: usize,
        ch: char,
        fg: Color,
        bg: Color,
        deco: Decoration,
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
                deco,
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
        deco: Decoration,
    ) {
        for (i, ch) in string.chars().enumerate() {
            self.set_char_with_attrs(y, x + i, ch, fg, bg, deco);
        }
    }

    pub fn draw_char(&mut self, y: usize, x: usize, ch: char) {
        self.set_char_with_attrs(y, x, ch, Color::Reset, Color::Reset, Default::default());
    }

    pub fn draw_str(&mut self, y: usize, x: usize, string: &str) {
        self.set_str_with_attrs(y, x, string, Color::Reset, Color::Reset, Default::default());
    }

    pub fn set_fg(&mut self, y: usize, x: usize, x_end: usize, fg: Color) {
        self.update_range(y, x, y + 1, x_end, |graph| graph.fg = fg);
    }

    pub fn set_bg(&mut self, y: usize, x: usize, x_end: usize, bg: Color) {
        self.update_range(y, x, y + 1, x_end, |graph| graph.bg = bg);
    }

    pub fn set_decoration(&mut self, y: usize, x: usize, x_end: usize, deco: Decoration) {
        self.update_range(y, x, y + 1, x_end, |graph| graph.deco = deco);
    }

    pub fn set_style(&mut self, y: usize, x: usize, x_end: usize, style: &Style) {
        self.update_range(y, x, y + 1, x_end, |graph| {
            graph.fg = style.fg;
            graph.bg = style.bg;
            graph.deco = style.deco;
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

    pub fn draw_borders(
        &mut self,
        y_top: usize,
        x_left: usize,
        y_bottom: usize,
        x_right: usize,
    ) -> CanvasViewMut<'_> {
        debug_assert!(y_top < y_bottom);
        debug_assert!(x_left < y_bottom);
        debug_assert!(y_bottom <= self.height);
        debug_assert!(x_right <= self.width);

        if x_right - x_left <= 2 || y_bottom - y_top <= 2 {
            warn!("too small canvas to draw borders");
            return CanvasViewMut {
                graphs: self.graphs,
                canvas_width: self.canvas_width,
                x: self.x,
                y: self.y,
                width: self.width,
                height: self.height,
            };
        }

        let vertical_bar = Grapheme::new("\u{2502}" /* vertical bar */);
        for y in (y_top + 1)..(y_bottom - 1) {
            self.set_grapheme(y, x_left, vertical_bar);
            self.set_grapheme(y, x_right - 1, vertical_bar);
        }

        let horizontal_bar = Grapheme::new("\u{2500}" /* horizontal bar */);
        for x in (x_left + 1)..(x_right - 1) {
            self.set_grapheme(y_top, x, horizontal_bar);
            self.set_grapheme(y_bottom - 1, x, horizontal_bar);
        }

        self.set_grapheme(y_top, x_left, Grapheme::new("\u{250d}" /* scroll */));
        self.set_grapheme(
            y_top,
            x_right - 1,
            Grapheme::new("\u{2511}" /* top_right */),
        );
        self.set_grapheme(
            y_bottom - 1,
            x_left,
            Grapheme::new("\u{2515}" /* bottom_left */),
        );
        self.set_grapheme(
            y_bottom - 1,
            x_right - 1,
            Grapheme::new("\u{2519}" /* bottom_right */),
        );

        CanvasViewMut {
            graphs: self.graphs,
            canvas_width: self.canvas_width,
            x: self.x + 1,
            y: self.y + 1,
            width: self.width - 2,
            height: self.height - 2,
        }
    }
}
