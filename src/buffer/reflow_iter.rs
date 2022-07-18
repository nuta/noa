use crate::{
    cursor::Position, display_width::DisplayWidth, grapheme_iter::GraphemeIter,
    raw_buffer::RawBuffer,
};

pub struct ReflowIter<'a> {
    iter: GraphemeIter<'a>,
    /// The number of columns in the screen.
    screen_width: usize,
    screen_y: usize,
    screen_x: usize,
    tab_width: usize,
}

impl<'a> ReflowIter<'a> {
    pub fn new(
        buffer: &'a RawBuffer,
        pos_start: Position,
        screen_width: usize,
        tab_width: usize,
    ) -> ReflowIter<'a> {
        ReflowIter {
            iter: buffer.grapheme_iter(pos_start),
            screen_width,
            screen_y: 0,
            screen_x: 0,
            tab_width,
        }
    }
}

pub enum PrintableGrapheme<'a> {
    Grapheme(&'a str),
    Whitespaces,
    ZeroWidth,
}

pub struct ReflowItem<'a> {
    pub grapheme: PrintableGrapheme<'a>,
    pub grapheme_width: usize,
    pub pos_in_buffer: Position,
    pub pos_in_screen: (usize, usize),
}

impl<'a> Iterator for ReflowIter<'a> {
    type Item = ReflowItem<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (pos_in_buffer, grapheme) = self.iter.next()?;
            let (printable, grapheme_width) = match grapheme {
                "\n" => {
                    self.screen_y += 1;
                    self.screen_x = 0;
                    continue;
                }
                "\t" => {
                    let n = width_to_next_tab_stop(self.screen_x, self.tab_width);
                    (PrintableGrapheme::Whitespaces, n)
                }
                _ => {
                    let w = grapheme.display_width();
                    if w == 0 {
                        // We treat a zero-width character as a single character otherwise it'll be
                        // very confusing.
                        (PrintableGrapheme::ZeroWidth, 1)
                    } else {
                        (PrintableGrapheme::Grapheme(grapheme), w)
                    }
                }
            };

            if self.screen_x + grapheme_width > self.screen_width {
                self.screen_y += 1;
                self.screen_x = 0;
            }

            let pos_in_screen = (self.screen_y, self.screen_x);
            self.screen_x += grapheme_width;

            return Some(ReflowItem {
                grapheme: printable,
                grapheme_width,
                pos_in_buffer,
                pos_in_screen,
            });
        }
    }
}

fn width_to_next_tab_stop(x: usize, tab_width: usize) -> usize {
    let level = x / tab_width + 1;
    tab_width * level - x
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn next() {}
}
