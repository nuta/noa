use anyhow::Result;
use arrayvec::ArrayString;

pub trait Surface {
    fn is_invalidated(&self, ctx: super::Context) -> bool;
    fn render(&mut self, ctx: super::Context, canvas: &mut Canvas) -> Result<()>;
    fn handle_terminal_event(&mut self, ctx: super::Context) -> Result<()>;
}

/// A character in the terminal screen.
#[derive(Clone, Copy, Debug)]
pub struct Grapheme {
    /// The character. It can be larger than 1 if it consists of multiple unicode
    /// characters like A with the acute accent.
    grapheme: ArrayString<4>,
    fg: crossterm::style::Color,
    bg: crossterm::style::Color,
    attrs: crossterm::style::Attributes,
}

impl Grapheme {
    pub fn blank() -> Grapheme {
        use crossterm::style::Color;

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

    pub fn copy_from_other(&mut self, y: usize, x: usize, other: &Canvas) {
        debug_assert!(y < self.height);
        debug_assert!(x < self.width);
        debug_assert!(y + other.height < self.height);
        debug_assert!(x + other.width < self.width);
        let start = y * self.width + x;
        let end = (y + other.height) * self.width + (x + other.width);
        (&mut self.graphs[start..end]).copy_from_slice(&other.graphs[..]);
    }
}
