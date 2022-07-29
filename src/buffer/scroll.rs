use crate::{
    paragraph_iter::{Paragraph, ParagraphIndex},
    raw_buffer::RawBuffer,
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

            if let Some(_) = current_paragraph_reflow.next() {
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

    pub fn scroll_up(&mut self, _buffer: &RawBuffer, n: usize) {
        for _ in 0..n {
            //
        }
    }
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
    }

    #[test]
    fn scroll_down_soft_wrapped() {
        // abcde
        // xyz
        let buf = RawBuffer::from_text("abcdexyz");
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
    }
}
