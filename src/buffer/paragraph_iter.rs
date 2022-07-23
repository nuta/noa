use crate::{cursor::Position, raw_buffer::RawBuffer, reflow_iter::ReflowIter};

pub struct Paragraph<'a> {
    pub reflow_iter: ReflowIter<'a>,
}

pub struct ParagraphIter<'a> {
    pos: Position,
    buffer: &'a RawBuffer,
    screen_width: usize,
    tab_width: usize,
}

impl<'a> ParagraphIter<'a> {
    /// Returns `ParagraphIter` from the paragraph containing the given position.
    pub fn new(
        buffer: &'a RawBuffer,
        pos: Position,
        screen_width: usize,
        tab_width: usize,
    ) -> ParagraphIter<'a> {
        ParagraphIter {
            pos,
            buffer,
            screen_width,
            tab_width,
        }
    }
}

impl<'a> Iterator for ParagraphIter<'a> {
    type Item = Paragraph<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let pos = self.pos;
        if !self.buffer.is_valid_position(pos) {
            return None;
        }

        dbg!(pos);

        // TODO: Support for too long lines: split a line into multiple paragraphs.
        let pos = Position::new(pos.y, 0);
        self.pos = Position::new(pos.y + 1, 0);

        let reflow_iter = ReflowIter::new(self.buffer, pos, self.screen_width, self.tab_width);
        Some(Paragraph { reflow_iter })
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn paragraph_iter() {}
}
