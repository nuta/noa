use crate::buffer::Buffer;

impl Buffer {
    pub fn delete_current_word(&mut self) {
        self.select_current_word();
        self.delete();
    }

    pub fn select_current_word(&mut self) {
        self.update_cursors_with(|c, buffer| {
            let mut word_iter = buffer.word_iter(c.moving_position());
            if let Some(selection) = word_iter.next() {
                c.select_pos(selection.range());
            }
        });
    }

    pub fn select_next_word(&mut self) {
        self.update_cursors_with(|c, buffer| {
            let mut word_iter = buffer.word_iter(c.moving_position());

            // Skip current word.
            if let Some(word) = word_iter.next() {
                c.move_moving_position_to(word.range().back());
            }
        });
    }

    pub fn select_prev_word(&mut self) {
        self.update_cursors_with(|c, buffer| {
            let mut word_iter = buffer.word_iter(c.moving_position());

            // Skip current word.
            word_iter.prev();
            // Move to the previous word.
            word_iter.prev();

            c.move_moving_position_to(word_iter.position());
        });
    }

    pub fn move_to_next_word(&mut self) {
        self.update_cursors_with(|c, buffer| {
            let pos = c.moving_position();
            let mut word_iter = buffer.word_iter_from_beginning_of_word(pos);
            if let Some(word) = word_iter.next() {
                if pos == word.range().back() {
                    // Move to the next word.
                    if let Some(next_word) = word_iter.next() {
                        c.move_to_pos(next_word.range().back());
                    }
                } else {
                    // Move to the end of the current word.
                    c.move_to_pos(word.range().back());
                }
            }
        });
    }

    pub fn move_to_prev_word(&mut self) {
        self.update_cursors_with(|c, buffer| {
            let pos = c.moving_position();
            let mut word_iter = buffer.word_iter_from_end_of_word(pos);
            if let Some(word) = word_iter.prev() {
                if pos == word.range().front() {
                    // Move to the previous word.
                    if let Some(next_word) = word_iter.prev() {
                        c.move_to_pos(next_word.range().front());
                    }
                } else {
                    // Move to the beginning of the current word.
                    c.move_to_pos(word.range().front());
                }
            }
        });
    }
}
