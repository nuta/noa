use std::{cmp::max, collections::HashMap};

use noa_buffer::{
    buffer::Buffer,
    cursor::{Cursor, Position},
};

use crate::view::View;

pub struct MovementState {
    visual_xs: HashMap<Cursor, usize>,
}

impl MovementState {
    pub fn new() -> MovementState {
        MovementState {
            visual_xs: HashMap::new(),
        }
    }

    pub fn movement<'a>(&'a mut self, buffer: &'a mut Buffer, view: &'a View) -> Movement<'a> {
        Movement {
            state: self,
            buffer,
            view,
        }
    }
}

pub struct Movement<'a> {
    state: &'a mut MovementState,
    buffer: &'a mut Buffer,
    view: &'a View,
}

impl<'a> Movement<'a> {
    /// Moves the cursor to left by one grapheme.
    pub fn move_cursors_left(&mut self) {
        self.update_cursors_with(|buffer, _, c| c.move_left(buffer));
        self.state.visual_xs.clear();
    }

    /// Moves the cursor to right by one grapheme.
    pub fn move_cursors_right(&mut self) {
        self.update_cursors_with(|buffer, _, c| c.move_right(buffer));
        self.state.visual_xs.clear();
    }

    /// Moves the cursor to up by one display row (respecting soft wrapping).
    pub fn move_cursors_up(&mut self) {
        self.move_cursors_vertically(-1, |c, pos| c.move_to(pos));
    }

    /// Moves the cursor to down by one display row (respecting soft wrapping).
    pub fn move_cursors_down(&mut self) {
        self.move_cursors_vertically(1, |c, pos| c.move_to(pos));
    }

    pub fn add_cursors_up(&mut self) {
        todo!()
    }

    pub fn add_cursors_down(&mut self) {
        todo!()
    }

    pub fn select_up(&mut self) {
        self.move_cursors_vertically(-1, |c, pos| {
            c.move_moving_position_to(pos);
        });
    }
    pub fn select_down(&mut self) {
        self.move_cursors_vertically(1, |c, pos| {
            c.move_moving_position_to(pos);
        });
    }
    pub fn select_left(&mut self) {
        self.move_cursors_horizontally(-1, |c, pos| {
            c.move_moving_position_to(pos);
        });
    }
    pub fn select_right(&mut self) {
        self.move_cursors_horizontally(1, |c, pos| {
            c.move_moving_position_to(pos);
        });
    }

    pub fn select_until_beginning_of_line(&mut self) {
        self.buffer.deselect_cursors();

        self.update_cursors_with(|buffer, _, c| {
            let pos = c.moving_position();
            let left = pos.x;

            let mut new_pos = c.moving_position();
            new_pos.move_by(buffer, 0, 0, left, 0);
            c.move_moving_position_to(new_pos);
        });
    }

    pub fn select_until_end_of_line(&mut self) {
        self.buffer.deselect_cursors();

        self.update_cursors_with(|buffer, _, c| {
            let pos = c.moving_position();
            let right = buffer.line_len(pos.y) - pos.x;

            let mut new_pos = c.moving_position();
            new_pos.move_by(buffer, 0, 0, 0, right);
            c.move_moving_position_to(new_pos);
        });
    }

    fn move_cursors_vertically<F>(&mut self, y_diff: isize, f: F)
    where
        F: Fn(&mut Cursor, Position),
    {
        let mut visual_xs = self.state.visual_xs.clone();
        let mut new_visual_xs = HashMap::new();
        self.update_cursors_with(|buffer, view, c| {
            let (i_y, i_x) = view.locate_row_by_position(c.moving_position());
            let dest_row = view.display_rows_as_slice().get(if y_diff > 0 {
                i_y.saturating_add(y_diff.abs() as usize)
            } else {
                i_y.saturating_sub(y_diff.abs() as usize)
            });

            if let Some(dest_row) = dest_row {
                let visual_x = visual_xs.get(c).copied();
                let new_pos = dest_row
                    .positions
                    .get(max(i_x, visual_x.unwrap_or(i_x)))
                    .copied()
                    .unwrap_or_else(|| dest_row.end_of_row_position());
                f(c, new_pos);
                new_visual_xs.insert(c.clone(), visual_x.unwrap_or(i_x));
            }
        });
        self.state.visual_xs = new_visual_xs;
    }

    fn move_cursors_horizontally<F>(&mut self, x_diff: isize, f: F)
    where
        F: Fn(&mut Cursor, Position),
    {
        let (left, right) = if x_diff > 0 {
            (0, x_diff as usize)
        } else {
            (x_diff as usize, 0)
        };

        self.update_cursors_with(|buffer, _, c| {
            let mut new_pos = c.moving_position();
            new_pos.move_by(buffer, 0, 0, left, right);
            f(c, new_pos);
        });
    }

    // TODO: Use Buffer::updater_cursors_with() once the borrow checker supports
    //       using &mut self.view in its closure.
    fn update_cursors_with<F>(&mut self, mut f: F)
    where
        F: FnMut(&Buffer, &View, &mut Cursor),
    {
        let mut new_cursors = self.buffer.cursors().to_vec();
        for c in &mut new_cursors {
            f(&self.buffer, &self.view, c);
        }
        self.buffer.set_cursors(&new_cursors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_movement() {
        // ABC
        // 12
        // XYZ
        let mut buffer = Buffer::from_text("ABC\n12\nXYZ");
        let mut view = View::new();
        view.layout(&buffer, 5);
        let mut movement_state = MovementState::new();
        let mut movement = movement_state.movement(&mut buffer, &mut view);

        movement.buffer.set_cursors(&[Cursor::new(2, 1)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(1, 1)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(0, 1)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(0, 1)]);

        movement.buffer.set_cursors(&[Cursor::new(0, 1)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(1, 1)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(2, 1)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(2, 1)]);
    }

    #[test]
    fn cursor_movement_at_end_of_line() {
        // ABC
        // ABC
        // ABC
        let mut buffer = Buffer::from_text("ABC\nABC\nABC");
        let mut view = View::new();
        view.layout(&buffer, 5);
        let mut movement_state = MovementState::new();
        let mut movement = movement_state.movement(&mut buffer, &mut view);

        movement.buffer.set_cursors(&[Cursor::new(2, 3)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(1, 3)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(0, 3)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(0, 3)]);

        movement.buffer.set_cursors(&[Cursor::new(0, 3)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(1, 3)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(2, 3)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(2, 3)]);
    }

    #[test]
    fn cursor_movement_through_empty_text() {
        let mut buffer = Buffer::from_text("");
        let mut view = View::new();
        view.layout(&buffer, 5);
        let mut movement_state = MovementState::new();
        let mut movement = movement_state.movement(&mut buffer, &mut view);

        movement.buffer.set_cursors(&[Cursor::new(0, 0)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(0, 0)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(0, 0)]);
    }

    #[test]
    fn cursor_movement_through_empty_lines() {
        // ""
        // ""
        // ""
        let mut buffer = Buffer::from_text("\n\n");
        let mut view = View::new();
        view.layout(&buffer, 5);
        let mut movement_state = MovementState::new();
        let mut movement = movement_state.movement(&mut buffer, &mut view);

        movement.buffer.set_cursors(&[Cursor::new(2, 0)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(1, 0)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(0, 0)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(0, 0)]);

        movement.buffer.set_cursors(&[Cursor::new(0, 0)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(1, 0)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(2, 0)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(2, 0)]);
    }

    #[test]
    fn cursor_movement_preserving_visual_x() {
        // "ABCDEFG"
        // "123"
        // ""
        // "HIJKLMN"
        let mut buffer = Buffer::from_text("ABCDEFG\n123\n\nHIJKLMN");
        let mut view = View::new();
        view.layout(&buffer, 10);
        let mut movement_state = MovementState::new();
        let mut movement = movement_state.movement(&mut buffer, &mut view);

        movement.buffer.set_cursors(&[Cursor::new(3, 5)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(2, 0)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(1, 3)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(0, 5)]);
        movement.move_cursors_up();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(0, 5)]);

        movement.buffer.set_cursors(&[Cursor::new(0, 5)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(1, 3)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(2, 0)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(3, 5)]);
        movement.move_cursors_down();
        assert_eq!(movement.buffer.cursors(), &[Cursor::new(3, 5)]);
    }
}
