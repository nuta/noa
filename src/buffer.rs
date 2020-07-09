use std::fmt;
use std::cmp::min;
use std::cmp::Ordering;
use std::collections::HashMap;
use crate::rope::*;

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
                    self.buf.insert(&range.start, string);
                }
            };
        }
    }

    pub fn backspace(&mut self) {
        for c in self.cursors.move_by_backspace(&self.buf) {
            match c {
                Cursor::Normal(pos) => {
                    let start = if pos.y == 0 && pos.x == 0 {
                        continue;
                    } else if pos.x == 0 {
                        Point::new(pos.y - 1, self.buf.line_len(pos.y - 1))
                    } else {
                        Point::new(pos.y, pos.x - 1)
                    };

                    self.buf.remove(&Range::from_points(start, pos));
                }
                Cursor::Selection(range) => {
                    self.buf.remove(&range);
                }
            };
        }
    }

    pub fn delete(&mut self) {
        for c in self.cursors.move_by_delete(&self.buf) {
            match c {
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

                    self.buf.remove(&Range::from_points(pos, end));
                }
                Cursor::Selection(range) => {
                    self.buf.remove(&range);
                }
            };
        }
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
