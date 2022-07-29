use crate::{
    paragraph_iter::{Paragraph, ParagraphIndex},
    raw_buffer::RawBuffer,
};

#[derive(Debug)]
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
