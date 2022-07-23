use crate::{
    cursor::{Position, Range},
    raw_buffer::RawBuffer,
    reflow_iter::ReflowIter,
};

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

    pub fn prev(&mut self) -> Option<Paragraph<'_>> {
        if self.pos.y == 0 {
            return None;
        }

        // TODO: Support for too long lines: split a line into multiple paragraphs.
        let pos_start = Position::new(self.pos.y - 1, 0);
        let pos_end = Position::new(self.pos.y, 0);
        self.pos = Position::new(self.pos.y - 1, 0);

        let reflow_iter = ReflowIter::new(
            self.buffer,
            Range::from_positions(pos_start, pos_end),
            self.screen_width,
            self.tab_width,
        );
        Some(Paragraph { reflow_iter })
    }
}

impl<'a> Iterator for ParagraphIter<'a> {
    type Item = Paragraph<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let pos = self.pos;
        if !self.buffer.is_valid_position(pos) {
            return None;
        }

        // TODO: Support for too long lines: split a line into multiple paragraphs.
        let pos_start = Position::new(pos.y, 0);
        let pos_end = Position::new(pos.y + 1, 0);
        self.pos = Position::new(pos_start.y + 1, 0);

        let reflow_iter = ReflowIter::new(
            self.buffer,
            Range::from_positions(pos_start, pos_end),
            self.screen_width,
            self.tab_width,
        );
        Some(Paragraph { reflow_iter })
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn paragraph_iter() {}
}
