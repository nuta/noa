use crate::{buffer::Buffer, cursor::Cursor, word_iter::Word};

impl Buffer {
    pub fn delete_current_word(&mut self) {
        self.select_current_word();
        self.delete_if_not_empty();
    }

    pub fn select_current_word(&mut self) {
        self.update_cursors_with(|c, buffer| {
            if let Some(selection) = buffer.current_word(c.moving_position()) {
                c.select_pos(selection);
            }
        });
    }

    pub fn select_next_word(&mut self) {
        self.update_cursors_with_next_word(|c, word| {
            c.move_moving_position_to(word.range().back())
        });
    }

    pub fn select_prev_word(&mut self) {
        self.update_cursors_with_prev_word(|c, word| {
            c.move_moving_position_to(word.range().front())
        });
    }

    pub fn move_to_next_word(&mut self) {
        self.update_cursors_with_next_word(|c, word| c.move_to_pos(word.range().back()));
    }

    pub fn move_to_prev_word(&mut self) {
        self.update_cursors_with_prev_word(|c, word| c.move_to_pos(word.range().front()));
    }

    fn update_cursors_with_next_word<F>(&mut self, callback: F)
    where
        F: Fn(&mut Cursor, Word),
    {
        self.update_cursors_with(|c, buffer| {
            let pos = c.moving_position();
            let mut word_iter = buffer.word_iter_from_beginning_of_word(pos);
            if let Some(word) = word_iter.next() {
                if pos == word.range().back() {
                    // Move to the next word.
                    if let Some(next_word) = word_iter.next() {
                        callback(c, next_word);
                    }
                } else {
                    // Move to the end of the current word.
                    callback(c, word);
                }
            }
        });
    }

    fn update_cursors_with_prev_word<F>(&mut self, callback: F)
    where
        F: Fn(&mut Cursor, Word),
    {
        self.update_cursors_with(|c, buffer| {
            let pos = c.moving_position();
            let mut word_iter = buffer.word_iter_from_end_of_word(pos);
            if let Some(word) = word_iter.prev() {
                if pos == word.range().front() {
                    // Move to the next word.
                    if let Some(prev_word) = word_iter.prev() {
                        callback(c, prev_word);
                    }
                } else {
                    // Move to the beginning of the current word.
                    callback(c, word);
                }
            }
        });
    }
}
