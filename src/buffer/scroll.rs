use crate::{
    cursor::Position,
    paragraph_iter::{Paragraph, ParagraphIndex},
    raw_buffer::RawBuffer,
    reflow_iter::ScreenPosition,
};

#[derive(Debug, PartialEq, Eq)]
pub struct Scroll {
    pub paragraph_index: ParagraphIndex,
    pub y_in_paragraph: usize,
    // Non-zero only if soft wrap is disabled.
    pub x_in_paragraph: usize,
}

impl Scroll {
    pub fn zeroed() -> Scroll {
        Scroll {
            paragraph_index: ParagraphIndex::zeroed(),
            y_in_paragraph: 0,
            x_in_paragraph: 0,
        }
    }

    pub fn scroll_down(
        &mut self,
        buffer: &RawBuffer,
        screen_width: usize,
        tab_width: usize,
        n: usize,
    ) {
        for _ in 0..n {
            let mut paragraph_iter =
                buffer.paragraph_iter_at_index(self.paragraph_index, screen_width, tab_width);
            let mut current_paragraph_reflow = paragraph_iter
                .next()
                .unwrap()
                .reflow_iter
                .skip_while(|item| item.pos_in_screen.y <= self.y_in_paragraph);

            if current_paragraph_reflow.next().is_some() {
                // Scroll within the current paragraph.
                self.y_in_paragraph += 1;
                continue;
            }

            match paragraph_iter.next() {
                Some(Paragraph { index, .. }) => {
                    // Scroll to the next paragraph.
                    self.paragraph_index = index;
                    self.y_in_paragraph = 0;
                }
                None => {
                    // No more paragraph: at EOF.
                    return;
                }
            }
        }
    }

    pub fn scroll_up(
        &mut self,
        buffer: &RawBuffer,
        screen_width: usize,
        tab_width: usize,
        n: usize,
    ) {
        for _ in 0..n {
            if self.y_in_paragraph > 0 {
                // Scroll within the current paragraph.
                self.y_in_paragraph -= 1;
            } else {
                // Scroll to the previous paragraph.
                let mut paragraph_iter =
                    buffer.paragraph_iter_at_index(self.paragraph_index, screen_width, tab_width);

                if let Some(prev) = paragraph_iter.prev() {
                    self.paragraph_index = prev.index;
                    self.y_in_paragraph = prev
                        .reflow_iter
                        .map(|item| item.pos_in_screen.y)
                        .max()
                        .unwrap_or(0);
                }
            }
        }
    }

    pub fn adjust_scroll(
        &mut self,
        buffer: &RawBuffer,
        screen_width: usize,
        screen_height: usize,
        tab_width: usize,
        first_visible_pos: Position,
        last_visible_pos: Position,
        pos: Position,
    ) {
        if let Some((paragraph_index, pos_in_screen)) =
            locate_row(buffer, screen_width, tab_width, pos)
        {
            // Scroll vertically.
            if pos < first_visible_pos || pos > last_visible_pos {
                self.paragraph_index = paragraph_index;
                self.y_in_paragraph = pos_in_screen.y;

                if pos > last_visible_pos {
                    self.scroll_up(
                        buffer,
                        screen_width,
                        tab_width,
                        screen_height.saturating_sub(1),
                    );
                }
            }

            // Scroll horizontally (no softwrap).
            info!(
                "scroll h: {} {} {}",
                pos_in_screen.x, self.x_in_paragraph, screen_width
            );
            if pos_in_screen.x >= self.x_in_paragraph + screen_width {
                self.x_in_paragraph = pos_in_screen.x - screen_width + 1;
            } else if pos_in_screen.x < self.x_in_paragraph {
                self.x_in_paragraph = pos_in_screen.x;
            }
        }
    }
}

fn locate_row(
    buffer: &RawBuffer,
    screen_width: usize,
    tab_width: usize,
    pos: Position,
) -> Option<(ParagraphIndex, ScreenPosition)> {
    let paragraph = buffer
        .paragraph_iter(pos, screen_width, tab_width)
        .next()
        .unwrap();

    paragraph
        .reflow_iter
        .skip_while(|item| item.pos_in_buffer < pos)
        .next()
        .map(|item| (paragraph.index, item.pos_in_screen))
}

#[cfg(test)]
mod tests {
    use crate::cursor::Position;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn scroll_down() {
        // abc
        // xyz
        let buf = RawBuffer::from_text("abc\nxyz");
        let mut scroll = Scroll {
            paragraph_index: ParagraphIndex::new(&buf, Position::new(0, 0)),
            x_in_paragraph: 0,
            y_in_paragraph: 0,
        };

        scroll.scroll_down(&buf, 5, 4, 1);
        assert_eq!(
            scroll,
            Scroll {
                paragraph_index: ParagraphIndex { buffer_y: 1 },
                x_in_paragraph: 0,
                y_in_paragraph: 0,
            }
        );

        // Scroll at EOF. No changes.
        scroll.scroll_down(&buf, 5, 4, 1);
        assert_eq!(
            scroll,
            Scroll {
                paragraph_index: ParagraphIndex { buffer_y: 1 },
                x_in_paragraph: 0,
                y_in_paragraph: 0,
            }
        );
    }

    #[test]
    fn scroll_down_soft_wrapped() {
        // abcde
        // xyz
        //
        let buf = RawBuffer::from_text("abcdexyz\n");
        let mut scroll = Scroll {
            paragraph_index: ParagraphIndex::new(&buf, Position::new(0, 0)),
            x_in_paragraph: 0,
            y_in_paragraph: 0,
        };

        scroll.scroll_down(&buf, 5, 4, 1);
        assert_eq!(
            scroll,
            Scroll {
                paragraph_index: ParagraphIndex { buffer_y: 0 },
                x_in_paragraph: 0,
                y_in_paragraph: 1,
            }
        );

        scroll.scroll_down(&buf, 5, 4, 1);
        assert_eq!(
            scroll,
            Scroll {
                paragraph_index: ParagraphIndex { buffer_y: 1 },
                x_in_paragraph: 0,
                y_in_paragraph: 0,
            }
        );
    }

    #[test]
    fn scroll_up() {
        // abc
        // xyz
        let buf = RawBuffer::from_text("abc\nxyz");
        let mut scroll = Scroll {
            paragraph_index: ParagraphIndex::new(&buf, Position::new(1, 0)),
            x_in_paragraph: 0,
            y_in_paragraph: 0,
        };

        scroll.scroll_up(&buf, 5, 4, 1);
        assert_eq!(
            scroll,
            Scroll {
                paragraph_index: ParagraphIndex { buffer_y: 0 },
                x_in_paragraph: 0,
                y_in_paragraph: 0,
            }
        );

        // Scroll at the top. No changes.
        scroll.scroll_up(&buf, 5, 4, 1);
        assert_eq!(
            scroll,
            Scroll {
                paragraph_index: ParagraphIndex { buffer_y: 0 },
                x_in_paragraph: 0,
                y_in_paragraph: 0,
            }
        );
    }

    #[test]
    fn scroll_up_soft_wrapped() {
        // abcde
        // xyz
        let buf = RawBuffer::from_text("abcdexyz");
        let mut scroll = Scroll {
            paragraph_index: ParagraphIndex::new(&buf, Position::new(0, 0)),
            x_in_paragraph: 0,
            y_in_paragraph: 1,
        };

        scroll.scroll_up(&buf, 5, 4, 1);
        assert_eq!(
            scroll,
            Scroll {
                paragraph_index: ParagraphIndex { buffer_y: 0 },
                x_in_paragraph: 0,
                y_in_paragraph: 0,
            }
        );
    }

    #[test]
    fn test_locate_row() {
        // abcde
        // xyz
        // 123
        let buf = RawBuffer::from_text("abcdexyz\n123");

        assert_eq!(
            locate_row(&buf, 5, 4, Position::new(0, 0)),
            Some((ParagraphIndex { buffer_y: 0 }, 0))
        );
        assert_eq!(
            locate_row(&buf, 5, 4, Position::new(0, 3)),
            Some((ParagraphIndex { buffer_y: 0 }, 0))
        );
        assert_eq!(
            locate_row(&buf, 5, 4, Position::new(0, 5)),
            Some((ParagraphIndex { buffer_y: 0 }, 1))
        );
        assert_eq!(
            locate_row(&buf, 5, 4, Position::new(1, 2)),
            Some((ParagraphIndex { buffer_y: 1 }, 0))
        );
    }
}
