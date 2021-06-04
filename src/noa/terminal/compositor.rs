use anyhow::Result;

pub trait Surface {
    fn is_invalidated(&self, ctx: super::Context) -> bool;
    fn render(&mut self, ctx: super::Context, canvas: &mut Canvas) -> Result<()>;
    fn handle_terminal_event(&mut self, ctx: super::Context) -> Result<()>;
}

/// A character in the terminal screen.
#[derive(Clone, Debug)]
pub struct Grapheme {
    /// The character. It can be larger than 1 if it consists of multiple unicode
    /// characters like A with the acute accent.
    grapheme: String,
    fg: crossterm::style::Color,
    bg: crossterm::style::Color,
    attrs: crossterm::style::Attributes,
}

/// A rectangle filled with characters.
pub struct Canvas {
    cells: Vec<Vec<Grapheme>>,
    /// The number of characters in a screen column.
    width: usize,
    /// The number of lines in the screen.
    height: usize,
}

impl Canvas {
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn set_grapheme(&self, y: usize, x: usize, graph: Grapheme) -> usize {
        self.height
    }
}
