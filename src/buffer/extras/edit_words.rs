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
            let prev_pos = c.moving_position();
            let mut word_iter = buffer.word_iter(prev_pos);
            if let Some(word) = word_iter.next() {
                if prev_pos == word.range().back() {
                    // Move to the next word.
                    if let Some(next_word) = word_iter.next() {
                        c.move_to_pos(next_word.range().back());
                    }
                } else {
                    // The end of the current word.
                    c.move_to_pos(word.range().back());
                }
            }
        });
    }

    pub fn move_to_prev_word(&mut self) {
        self.update_cursors_with(|c, buffer| {
            let mut word_iter = buffer.word_iter(c.moving_position());

            // Skip current word.
            word_iter.prev();
            // Move to the previous word.
            word_iter.prev();

            c.move_to_pos(word_iter.position());
        });
    }
}
