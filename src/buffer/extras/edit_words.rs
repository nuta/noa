use crate::buffer::Buffer;

impl Buffer {
    pub fn delete_current_word(&mut self) {
        self.select_current_word();
        self.delete();
    }

    pub fn select_current_word(&mut self) {
        self.update_cursors_with(|c, buffer| {
            let mut word_iter = buffer.word_iter(c.moving_position());

            // Move to current word.
            word_iter.next();

            if let Some(selection) = word_iter.range() {
                c.select_pos(*selection);
            }
        });
    }

    pub fn select_next_word(&mut self) {
        self.update_cursors_with(|c, buffer| {
            let mut word_iter = buffer.word_iter(c.moving_position());

            // Skip current word.
            word_iter.next();
            // Move to the next word.
            word_iter.next();

            c.move_moving_position_to(word_iter.position());
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
            let mut word_iter = buffer.word_iter(c.moving_position());

            // Skip current word.
            word_iter.next();
            // Move to the next word.
            word_iter.next();

            c.move_to_pos(word_iter.position());
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
