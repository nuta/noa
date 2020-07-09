use std::fmt;
use std::cmp::min;
use std::cmp::Ordering;
use std::collections::HashMap;
use crate::rope::*;

fn remove_range(buf: &mut Rope, range: &Range, next_cursor: Option<&Cursor>, new_cursors: &mut Vec<Cursor>) {
            // Remove the text in the range.
            buf.remove(&range);

            // Move cursors after the current cursor.
            let front = range.front();
            let end = range.end();
            let num_newlines_deleted = end.y - front.y;
            for c2 in new_cursors.iter_mut() {
                match c2 {
                    Cursor::Normal(pos) if pos.y == end.y => {
                        pos.x = front.x + (pos.x - end.x);
                        pos.y = front.y;
                    }
                    Cursor::Normal(pos) => {
                        pos.y -= num_newlines_deleted;
                    }
                    Cursor::Selection(_) => {
                        continue;
                    }
                }
            }

            // Preserve the current cursor if it's unique (no other cursors at
            // the same position).
            match next_cursor {
                Some(Cursor::Normal(pos)) if pos == front => {}
                _ => {
                    new_cursors.push(Cursor::Normal(*front));
                }
            }
}

pub struct Buffer {
    buf: Rope,
    cursors: CursorSet,
    undo_stack: Vec<Rope>,
    redo_stack: Vec<Rope>,
}

impl Buffer {
    pub fn new() -> Buffer {
        let mut buffer = Buffer {
            buf: Rope::new(),
            cursors: CursorSet::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };

        buffer.mark_undo_point();
        buffer
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn text(&self) -> String {
        self.buf.to_string()
    }

    pub fn cursors(&self) -> &[Cursor] {
        self.cursors.cursors()
    }

    pub fn set_cursors(&mut self, cursors: Vec<Cursor>) {
        self.cursors.set_cursors(cursors);
    }

    pub fn move_cursors(
        &mut self,
        up: usize,
        down: usize,
        left: usize,
        right: usize
    ) {
        self.cursors.move_by_offsets(&self.buf, up, down, left, right);
    }

    pub fn select(
        &mut self,
        up: usize,
        down: usize,
        left: usize,
        right: usize
    ) {
        self.cursors.select_by_offsets(&self.buf, up, down, left, right);
    }

    pub fn insert_char(&mut self, ch: char) {
        self.insert(&ch.to_string())
    }

    pub fn insert(&mut self, string: &str) {
        for cursor in self.cursors.move_by_insertion(&string) {
            match cursor {
                Cursor::Normal(pos) => {
                    self.buf.insert(&pos, string);
                }
                Cursor::Selection(range) => {
                    self.buf.remove(&range);
                    self.buf.insert(range.front(), string);
                }
            };
        }
    }

    pub fn backspace(&mut self) {
        let mut new_cursors = Vec::new();
        let mut iter = self.cursors.cursors().iter().rev().peekable();
        while let Some(c) = iter.next() {
            // Determine the range to be deleted.
            let range = match c {
                Cursor::Normal(pos) => {
                    let start = if pos.y == 0 && pos.x == 0 {
                        continue;
                    } else if pos.x == 0 {
                        Point::new(pos.y - 1, self.buf.line_len(pos.y - 1))
                    } else {
                        Point::new(pos.y, pos.x - 1)
                    };

                    Range::from_points(start, *pos)
                }
                Cursor::Selection(range) => {
                    range.clone()
                }
            };

            remove_range(&mut self.buf, &range, iter.peek().map(|r| *r), &mut new_cursors);
        }

        self.cursors.set_cursors(new_cursors);
    }

    pub fn delete(&mut self) {
        let mut new_cursors = Vec::new();
        let mut iter = self.cursors.cursors().iter().rev().peekable();
        while let Some(c) = iter.next() {
            // Determine the range to be deleted.
            let range = match c {
                Cursor::Normal(pos) => {
                    let max_y = self.buf.num_lines();
                    let max_x = self.buf.line_len(pos.y);
                    let end = if pos.y == max_y && pos.x == max_x {
                        continue;
                    } else if pos.x == max_x {
                        Point::new(pos.y + 1, 0)
                    } else {
                        Point::new(pos.y, pos.x + 1)
                    };

                    Range::from_points(*pos, end)
                }
                Cursor::Selection(range) => {
                    range.clone()
                }
            };

            remove_range(&mut self.buf, &range, iter.peek().map(|r| *r), &mut new_cursors);
        }

        self.cursors.set_cursors(new_cursors);
    }

    pub fn mark_undo_point(&mut self) {
        self.undo_stack.push(self.buf.clone());
    }

    pub fn undo(&mut self) {
        if self.undo_stack.len() == 1 && self.buf.is_empty() {
            return;
        }

        if let Some(top) = self.undo_stack.last() {
            if *top == self.buf {
                self.undo_stack.pop();
            }
        }

        if let Some(buf) = self.undo_stack.pop() {
            self.redo_stack.push(self.buf.clone());
            self.buf = buf;
        }
    }
    pub fn redo(&mut self) {
        if let Some(buf) = self.redo_stack.pop() {
            self.undo_stack.push(self.buf.clone());
            self.buf = buf;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn insertion_and_deletion() {
        let mut b = Buffer::new();
        b.insert("Hello");
        b.insert(" World?");
        assert_eq!(b.text(), "Hello World?");
        b.backspace();
        assert_eq!(b.text(), "Hello World");
        b.insert_char('!');
        assert_eq!(b.text(), "Hello World!");
        b.move_cursors(0, 0, 1, 0); // Move left
        b.delete();
        assert_eq!(b.text(), "Hello World");
        b.delete();
        assert_eq!(b.text(), "Hello World");
    }

    #[test]
    fn single_cursor() {
        let mut b = Buffer::new();
        b.move_cursors(1, 0, 0, 0); // Do nothing
        b.insert("A\nDEF\n12345");
        assert_eq!(b.cursors(), &[Cursor::Normal(Point::new(2, 5))]);
        b.move_cursors(0, 0, 1, 0); // Move right
        assert_eq!(b.cursors(), &[Cursor::Normal(Point::new(2, 4))]);
        b.move_cursors(1, 0, 0, 0); // Move up
        assert_eq!(b.cursors(), &[Cursor::Normal(Point::new(1, 3))]);
        b.move_cursors(0, 3, 0, 0); // Move down
        assert_eq!(b.cursors(), &[Cursor::Normal(Point::new(3, 0))]);
        b.move_cursors(0, 0, 1, 0); // Move left
        assert_eq!(b.cursors(), &[Cursor::Normal(Point::new(2, 5))]);
        b.move_cursors(0, 0, 0, 1); // Move right
        assert_eq!(b.cursors(), &[Cursor::Normal(Point::new(3, 0))]);
    }

    #[test]
    fn insert_on_multi_cursors() {
        let mut b = Buffer::new();
        // abc|
        // d|e
        // |xyz
        b.insert("abc\nde\nxyz");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 3)),
            Cursor::Normal(Point::new(1, 1)),
            Cursor::Normal(Point::new(2, 0)),
        ]);

        // abc123|
        // d123|e
        // 123|xyz
        b.insert("123");
        assert_eq!(b.text(), "abc123\nd123e\n123xyz");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 6)),
            Cursor::Normal(Point::new(1, 4)),
            Cursor::Normal(Point::new(2, 3)),
        ]);

        // abc123[
        // ]|
        // d123[
        // ]|e
        // 123[
        // ]|xyz
        b.insert("[\n]");
        assert_eq!(b.text(), "abc123[\n]\nd123[\n]e\n123[\n]xyz");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(1, 1)),
            Cursor::Normal(Point::new(3, 1)),
            Cursor::Normal(Point::new(5, 1)),
        ]);
    }

    #[test]
    fn backspace_on_multi_cursors() {
        // abc|      ab|
        // def|  =>  de|
        // xyz|      xy|
        let mut b = Buffer::new();
        b.insert("abc\ndef\nxyz");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 3)),
            Cursor::Normal(Point::new(1, 3)),
            Cursor::Normal(Point::new(2, 3)),
        ]);
        b.backspace();
        assert_eq!(b.text(), "ab\nde\nxy");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 2)),
            Cursor::Normal(Point::new(1, 2)),
            Cursor::Normal(Point::new(2, 2)),
        ]);

        // abc|      ab|
        // 1|    =>  |
        // xy|z      x|z
        let mut b = Buffer::new();
        b.insert("abc\n1\nxyz");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 3)),
            Cursor::Normal(Point::new(1, 1)),
            Cursor::Normal(Point::new(2, 2)),
        ]);
        b.backspace();
        assert_eq!(b.text(), "ab\n\nxz");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 2)),
            Cursor::Normal(Point::new(1, 0)),
            Cursor::Normal(Point::new(2, 1)),
        ]);

        // 1230|a|b|c|d|e|f => 123|f
        let mut b = Buffer::new();
        b.insert("1230abcdef");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 4)),
            Cursor::Normal(Point::new(0, 5)),
            Cursor::Normal(Point::new(0, 6)),
            Cursor::Normal(Point::new(0, 7)),
            Cursor::Normal(Point::new(0, 8)),
            Cursor::Normal(Point::new(0, 9)),
        ]);
        b.backspace();
        assert_eq!(b.text(), "123f");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 3)),
        ]);

        // a|bc      |bc|12
        // |12   =>  xy|
        // xyz|
        let mut b = Buffer::new();
        b.insert("abc\n12\nxyz");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 1)),
            Cursor::Normal(Point::new(1, 0)),
            Cursor::Normal(Point::new(2, 3)),
        ]);
        b.backspace();
        assert_eq!(b.text(), "bc12\nxy");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 0)),
            Cursor::Normal(Point::new(0, 2)),
            Cursor::Normal(Point::new(1, 2)),
        ]);

        // 0
        // |abc      0|abc|12|xyz
        // |12   =>
        // |xyz
        let mut b = Buffer::new();
        b.insert("0\nabc\n12\nxyz");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(1, 0)),
            Cursor::Normal(Point::new(2, 0)),
            Cursor::Normal(Point::new(3, 0)),
        ]);
        b.backspace();
        assert_eq!(b.text(), "0abc12xyz");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 1)),
            Cursor::Normal(Point::new(0, 4)),
            Cursor::Normal(Point::new(0, 6)),
        ]);

        // ab|     =>  a|def|g
        // |c|def
        // |g
        let mut b = Buffer::new();
        b.insert("ab\ncdef\ng");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 2)),
            Cursor::Normal(Point::new(1, 0)),
            Cursor::Normal(Point::new(1, 1)),
            Cursor::Normal(Point::new(2, 0)),
        ]);
        b.backspace();
        assert_eq!(b.text(), "adefg");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 1)),
            Cursor::Normal(Point::new(0, 4)),
        ]);

        // ab|   =>  a|def|g
        // |c|def
        // |g
        let mut b = Buffer::new();
        b.insert("ab\ncdef\ng");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 2)),
            Cursor::Normal(Point::new(1, 0)),
            Cursor::Normal(Point::new(1, 1)),
            Cursor::Normal(Point::new(2, 0)),
        ]);
        b.backspace();
        assert_eq!(b.text(), "adefg");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 1)),
            Cursor::Normal(Point::new(0, 4)),
        ]);
    }

    #[test]
    fn delete_on_multi_cursors() {
        // a|Xbc|Yd
        let mut b = Buffer::new();
        b.insert("aXbcYd");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 1)),
            Cursor::Normal(Point::new(0, 4)),
        ]);
        b.delete();
        assert_eq!(b.text(), "abcd");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 1)),
            Cursor::Normal(Point::new(0, 3)),
        ]);

        // a|b|
        let mut b = Buffer::new();
        b.insert("ab");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 1)),
            Cursor::Normal(Point::new(0, 2)),
        ]);
        b.delete();
        assert_eq!(b.text(), "a");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 1)),
        ]);

        // a|bc
        // d|ef
        // g|hi
        let mut b = Buffer::new();
        b.insert("abc\ndef\nghi");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 1)),
            Cursor::Normal(Point::new(1, 1)),
            Cursor::Normal(Point::new(2, 1)),
        ]);
        b.delete();
        assert_eq!(b.text(), "ac\ndf\ngi");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 1)),
            Cursor::Normal(Point::new(1, 1)),
            Cursor::Normal(Point::new(2, 1)),
        ]);

        // a|
        // b|X
        // c|Y
        // d|
        let mut b = Buffer::new();
        b.insert("a\nbX\ncY\nd");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 1)),
            Cursor::Normal(Point::new(1, 1)),
            Cursor::Normal(Point::new(2, 1)),
            Cursor::Normal(Point::new(3, 1)),
        ]);
        b.delete();
        assert_eq!(b.text(), "ab\nc\nd");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 1)),
            Cursor::Normal(Point::new(0, 2)),
            Cursor::Normal(Point::new(1, 1)),
            Cursor::Normal(Point::new(2, 1)),
        ]);

        // ab|
        // cde|
        let mut b = Buffer::new();
        b.insert("ab\ncde");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 2)),
            Cursor::Normal(Point::new(1, 3)),
        ]);
        b.delete();
        assert_eq!(b.text(), "abcde");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 2)),
            Cursor::Normal(Point::new(0, 5)),
        ]);

        // abc|
        // |d|ef
        // ghi|
        let mut b = Buffer::new();
        b.insert("abc\ndef\nghi");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 3)),
            Cursor::Normal(Point::new(1, 0)),
            Cursor::Normal(Point::new(1, 1)),
            Cursor::Normal(Point::new(2, 3)),
        ]);
        b.delete();
        assert_eq!(b.text(), "abcf\nghi");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 3)),
            Cursor::Normal(Point::new(1, 3)),
        ]);

        // abc|     => abc|d|e|f
        // d|Xe|Yf
        let mut b = Buffer::new();
        b.insert("abc\ndXeYf");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 3)),
            Cursor::Normal(Point::new(1, 1)),
            Cursor::Normal(Point::new(1, 3)),
        ]);
        b.delete();
        assert_eq!(b.text(), "abcdef");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 3)),
            Cursor::Normal(Point::new(0, 4)),
            Cursor::Normal(Point::new(0, 5)),
        ]);
    }

    #[test]
    fn multibyte_characters() {
        let mut b = Buffer::new();
        b.insert("Hello 世界!");
        b.set_cursors(vec![Cursor::Normal(Point::new(0, 7))]);
        assert_eq!(b.len(), 9);

        // Hello 世|界! => Hello |界!
        b.backspace();
        assert_eq!(b.text(), "Hello 界!");
        // Hello 世|界! => Hell|界!
        b.backspace();
        b.backspace();
        assert_eq!(b.text(), "Hell界!");
        // Hello 世|界! => Hell|界!
        b.insert("o こんにちは 世");
        assert_eq!(b.text(), "Hello こんにちは 世界!");
    }

    #[test]
    fn single_selection() {
        let mut b = Buffer::new();
        b.insert("abXYZcd");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 2))
        ]);

        // ab|XYZ|cd
        b.select(0, 0, 0, 3);
        assert_eq!(b.cursors(), &[
            Cursor::Selection(Range::new(0, 2, 0, 5)),
        ]);

        // a|b|XYZcd  =>  a|XYZcd
        b.select(0, 0, 4, 0);
        b.backspace();
        assert_eq!(b.text(), "aXYZcd");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 1)),
        ]);

        // a|XYZ|cd  =>  a|cd
        b.select(0, 0, 0, 3);
        b.backspace();
        assert_eq!(b.text(), "acd");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 1)),
        ]);
    }

    #[test]
    fn single_selection_including_newlines() {
        // xy|A     xy|z
        // BCD  =>
        // E|z
        let mut b = Buffer::new();
        b.insert("xyA\nBCD\nEz");
        b.set_cursors(vec![
            Cursor::Selection(Range::new(0, 2, 2, 1))
        ]);
        b.backspace();
        assert_eq!(b.text(), "xyz");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 2)),
        ]);
    }

    #[test]
    fn multi_selections() {
        // ab|XYZ  =>  ab|
        // cd|XYZ  =>  cd|
        // ef|XYZ  =>  ef|
        let mut b = Buffer::new();
        b.insert("abXYZ\ncdXYZ\nefXYZ");
        b.set_cursors(vec![
            Cursor::Selection(Range::new(0, 2, 0, 5)),
            Cursor::Selection(Range::new(1, 2, 1, 5)),
            Cursor::Selection(Range::new(2, 2, 2, 5)),
        ]);
        b.delete();
        assert_eq!(b.text(), "ab\ncd\nef");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 2)),
            Cursor::Normal(Point::new(1, 2)),
            Cursor::Normal(Point::new(2, 2)),
        ]);

        // ab|XY        ab|cd|ef|g
        // Z|cd|XY  =>
        // Z|ef|XY
        // Z|g
        let mut b = Buffer::new();
        b.insert("abXY\nZcdXY\nZefXY\nZg");
        b.set_cursors(vec![
            Cursor::Selection(Range::new(0, 2, 1, 1)),
            Cursor::Selection(Range::new(1, 3, 2, 1)),
            Cursor::Selection(Range::new(2, 3, 3, 1)),
        ]);
        b.backspace();
        assert_eq!(b.text(), "abcdefg");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 2)),
            Cursor::Normal(Point::new(0, 4)),
            Cursor::Normal(Point::new(0, 6)),
        ]);
    }

    #[test]
    fn undo() {
        let mut b = Buffer::new();
        b.redo();
        b.undo();
        assert_eq!(b.text(), "");
        b.insert("abc");
        b.mark_undo_point();
        assert_eq!(b.text(), "abc");
        b.redo(); // Do nothing.
        assert_eq!(b.text(), "abc");
        b.undo();
        assert_eq!(b.text(), "");
        b.redo();
        assert_eq!(b.text(), "abc");
        b.undo();
        assert_eq!(b.text(), "");
        b.undo();
        assert_eq!(b.text(), "");
        b.redo();
        assert_eq!(b.text(), "abc");
        b.redo();
        assert_eq!(b.text(), "abc");

        let mut b = Buffer::new();
        b.insert("abc");
        b.mark_undo_point();
        b.insert("123");
        b.mark_undo_point();
        b.insert("xyz");
        b.mark_undo_point();
        assert_eq!(b.text(), "abc123xyz");
        b.undo();
        assert_eq!(b.text(), "abc123");
        b.undo();
        assert_eq!(b.text(), "abc");
        b.redo();
        assert_eq!(b.text(), "abc123");
        b.redo();
        assert_eq!(b.text(), "abc123xyz");
        b.undo();
        assert_eq!(b.text(), "abc123");
        b.undo();
        assert_eq!(b.text(), "abc");
        b.undo();
        assert_eq!(b.text(), "");
        b.undo();
        assert_eq!(b.text(), "");
    }
}
