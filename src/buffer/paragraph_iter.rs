use crate::{
    cursor::{Position, Range},
    raw_buffer::RawBuffer,
    reflow_iter::ReflowIter,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParagraphIndex {
    pub buffer_y: usize,
}

impl ParagraphIndex {
    pub fn new(_buffer: &RawBuffer, pos: Position) -> Self {
        ParagraphIndex { buffer_y: pos.y }
    }

    pub fn zeroed() -> ParagraphIndex {
        ParagraphIndex { buffer_y: 0 }
    }
}

pub struct Paragraph<'a> {
    pub index: ParagraphIndex,
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
        let index = ParagraphIndex::new(buffer, pos);
        Self::new_at_index(buffer, index, screen_width, tab_width)
    }

    pub fn new_at_index(
        buffer: &'a RawBuffer,
        index: ParagraphIndex,
        screen_width: usize,
        tab_width: usize,
    ) -> ParagraphIter<'a> {
        let pos = Position::new(index.buffer_y, 0);
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
        Some(Paragraph {
            index: ParagraphIndex {
                buffer_y: pos_start.y,
            },
            reflow_iter,
        })
    }
}

impl<'a> Iterator for ParagraphIter<'a> {
    type Item = Paragraph<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let pos = self.pos;
        // if pos.y > self.buffer.num_lines() {
        if !self.buffer.is_valid_position(pos) {
            return None;
        }

        // TODO: Support for too long lines: split a line into multiple paragraphs.
        let pos_start = Position::new(pos.y, 0);
        let pos_end = Position::new(pos.y + 1, 0);
        self.pos = Position::new(pos_start.y + 1, 0);

        // FIXME: GraphemeIter::new() is slow
        let reflow_iter = ReflowIter::new(
            self.buffer,
            Range::from_positions(pos_start, pos_end),
            self.screen_width,
            self.tab_width,
        );
        Some(Paragraph {
            index: ParagraphIndex {
                buffer_y: pos_start.y,
            },
            reflow_iter,
        })
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn paragraph_iter() {}
}
