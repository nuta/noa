use crate::{
    cursor::Position, display_width::DisplayWidth, grapheme_iter::GraphemeIter,
    raw_buffer::RawBuffer,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenPosition {
    pub y: usize,
    pub x: usize,
}

impl ScreenPosition {
    pub fn new(y: usize, x: usize) -> ScreenPosition {
        ScreenPosition { y, x }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PrintableGrapheme<'a> {
    Eof,
    Grapheme(&'a str),
    Whitespaces,
    ZeroWidth,
    /// Contains newline character(s).
    Newline(Option<&'a str>),
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReflowItem<'a> {
    pub grapheme: PrintableGrapheme<'a>,
    pub grapheme_width: usize,
    pub pos_in_buffer: Position,
    pub pos_in_screen: ScreenPosition,
}

#[derive(Clone)]
pub struct ReflowIter<'a> {
    buffer: &'a RawBuffer,
    iter: GraphemeIter<'a>,
    /// The number of columns in the screen.
    screen_width: usize,
    screen_pos: ScreenPosition,
    tab_width: usize,
    pos_end: Option<Position>,
    return_eof: bool,
}

impl<'a> ReflowIter<'a> {
    pub fn new(
        buffer: &'a RawBuffer,
        pos_start: Position,
        pos_end: Option<Position>,
        screen_width: usize,
        tab_width: usize,
    ) -> ReflowIter<'a> {
        ReflowIter {
            buffer,
            iter: buffer.grapheme_iter(pos_start),
            screen_width,
            screen_pos: ScreenPosition { y: 0, x: 0 },
            tab_width,
            pos_end,
            return_eof: false,
        }
    }

    pub fn enable_eof(&mut self, enable: bool) {
        self.return_eof = enable;
    }
}

impl<'a> Iterator for ReflowIter<'a> {
    type Item = ReflowItem<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (pos_in_buffer, grapheme) = match self.iter.next() {
                Some((pos_in_buffer, grapheme)) => (pos_in_buffer, grapheme),
                None if self.return_eof => {
                    self.return_eof = false;
                    return Some(ReflowItem {
                        grapheme: PrintableGrapheme::Eof,
                        grapheme_width: 0,
                        // What if it's not eof?
                        pos_in_buffer: Position::new(
                            self.buffer.num_lines() - 1,
                            self.buffer.line_len(self.buffer.num_lines() - 1),
                        ),
                        pos_in_screen: self.screen_pos,
                    });
                }
                None => {
                    return None;
                }
            };

            if matches!(self.pos_end, Some(pos_end) if pos_in_buffer >= pos_end) {
                return None;
            }

            let (printable, grapheme_width) = match grapheme {
                "\n" => (PrintableGrapheme::Newline(Some(grapheme)), 1),
                "\t" => {
                    let n = width_to_next_tab_stop(self.screen_pos.x, self.tab_width);
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

            if self.screen_pos.x + grapheme_width > self.screen_width {
                self.screen_pos.y += 1;
                self.screen_pos.x = 0;
            }

            let pos_in_screen = self.screen_pos;

            if matches!(printable, PrintableGrapheme::Newline(_)) {
                self.screen_pos.y += 1;
                self.screen_pos.x = 0;
            } else {
                self.screen_pos.x += grapheme_width;
            }

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
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn reflow_iter() {
        // abc
        // d
        let buf = RawBuffer::from_text("abc\nd");
        let mut iter = ReflowIter::new(&buf, Position::new(0, 0), None, 4, 4);
        assert_eq!(
            iter.next(),
            Some(ReflowItem {
                grapheme: PrintableGrapheme::Grapheme("a"),
                grapheme_width: 1,
                pos_in_buffer: Position::new(0, 0),
                pos_in_screen: ScreenPosition { y: 0, x: 0 },
            })
        );
        assert_eq!(
            iter.next(),
            Some(ReflowItem {
                grapheme: PrintableGrapheme::Grapheme("b"),
                grapheme_width: 1,
                pos_in_buffer: Position::new(0, 1),
                pos_in_screen: ScreenPosition { y: 0, x: 1 },
            })
        );
        assert_eq!(
            iter.next(),
            Some(ReflowItem {
                grapheme: PrintableGrapheme::Grapheme("c"),
                grapheme_width: 1,
                pos_in_buffer: Position::new(0, 2),
                pos_in_screen: ScreenPosition { y: 0, x: 2 },
            })
        );
        assert_eq!(
            iter.next(),
            Some(ReflowItem {
                grapheme: PrintableGrapheme::Newline(Some("\n")),
                grapheme_width: 1,
                pos_in_buffer: Position::new(0, 3),
                pos_in_screen: ScreenPosition { y: 0, x: 3 },
            })
        );
        assert_eq!(
            iter.next(),
            Some(ReflowItem {
                grapheme: PrintableGrapheme::Grapheme("d"),
                grapheme_width: 1,
                pos_in_buffer: Position::new(1, 0),
                pos_in_screen: ScreenPosition { y: 1, x: 0 },
            })
        );
    }

    #[test]
    fn reflow_iter_wrapped() {
        // ab
        // c
        let buf = RawBuffer::from_text("abc");
        let mut iter = ReflowIter::new(&buf, Position::new(0, 0), None, 2, 4);
        assert_eq!(
            iter.next(),
            Some(ReflowItem {
                grapheme: PrintableGrapheme::Grapheme("a"),
                grapheme_width: 1,
                pos_in_buffer: Position::new(0, 0),
                pos_in_screen: ScreenPosition { y: 0, x: 0 },
            })
        );
        assert_eq!(
            iter.next(),
            Some(ReflowItem {
                grapheme: PrintableGrapheme::Grapheme("b"),
                grapheme_width: 1,
                pos_in_buffer: Position::new(0, 1),
                pos_in_screen: ScreenPosition { y: 0, x: 1 },
            })
        );
        assert_eq!(
            iter.next(),
            Some(ReflowItem {
                grapheme: PrintableGrapheme::Grapheme("c"),
                grapheme_width: 1,
                pos_in_buffer: Position::new(0, 2),
                pos_in_screen: ScreenPosition { y: 1, x: 0 },
            })
        );
    }

    #[test]
    fn test_width_to_next_tab_stop() {
        assert_eq!(width_to_next_tab_stop(0, 4), 4);
        assert_eq!(width_to_next_tab_stop(1, 4), 3);
        assert_eq!(width_to_next_tab_stop(2, 4), 2);
        assert_eq!(width_to_next_tab_stop(3, 4), 1);
        assert_eq!(width_to_next_tab_stop(4, 4), 4);
        assert_eq!(width_to_next_tab_stop(5, 4), 3);
        assert_eq!(width_to_next_tab_stop(6, 4), 2);
        assert_eq!(width_to_next_tab_stop(7, 4), 1);
        assert_eq!(width_to_next_tab_stop(8, 4), 4);
    }
}
