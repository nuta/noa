use crate::{
    buffer::Buffer,
    cursor::{Cursor, Range},
    word_iter::Word,
};

impl Buffer {
    pub fn current_word_str(&self) -> Option<String> {
        let c = self.main_cursor();
        if c.is_selection() {
            return None;
        }

        self.current_word(c.moving_position())
            .map(|range| self.substr(range))
    }

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

    pub fn backspace_word(&mut self) {
        self.cursors.foreach(|c, past_cursors| {
            if c.selection().is_empty() {
                // Select the previous word.
                let back_pos = c.moving_position();
                let mut char_iter = self.buf.char_iter(back_pos);

                fn is_whitespace_pred(c: char) -> bool {
                    matches!(c, ' ' | '\t' | '\n')
                }

                fn is_symbols_pred(c: char) -> bool {
                    "!@#$%^&*()-=+[]{}\\|;:'\",.<>/?".contains(c)
                }

                fn others_pred(c: char) -> bool {
                    !is_whitespace_pred(c) && !is_symbols_pred(c)
                }

                let pred = match char_iter.prev() {
                    Some(c) if is_whitespace_pred(c) => is_whitespace_pred,
                    Some(c) if is_symbols_pred(c) => is_symbols_pred,
                    _ => others_pred,
                };

                let front_pos = loop {
                    let pos = char_iter.last_position();
                    match char_iter.prev() {
                        Some(c) if pred(c) => continue,
                        _ => break pos,
                    }
                };

                c.select_range(Range::from_positions(front_pos, back_pos));
            }

            self.buf.edit_at_cursor(c, past_cursors, "");
        });
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
