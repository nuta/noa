use std::cmp::{Ordering, max};
use std::fmt;

pub struct GapBuffer {
    buf: Vec<char>,
    gap_start: usize,
    gap_end: usize,
    default_gap_len: usize,
}

impl GapBuffer {
    pub fn new() -> GapBuffer {
        GapBuffer {
            buf: Vec::new(),
            gap_start: 0,
            gap_end: 0,
            default_gap_len: 64,
        }
    }

    #[cfg(test)]
    pub fn set_default_gap_len(&mut self, len: usize) {
        self.default_gap_len = len;
    }

    pub fn len(&self) -> usize {
        self.buf.len() - (self.gap_end - self.gap_start)
    }

    pub fn text(&self) -> String {
        let mut s = String::with_capacity(self.buf.len() - (self.gap_end - self.gap_start));
        s.extend(&self.buf[..self.gap_start]);
        s.extend(&self.buf[self.gap_end..]);
        s
    }

    /// Inserts the text at `offset`.
    pub fn insert(&mut self, string: &str, offset: usize) {
        debug_assert!(offset <= self.len());
        let chars: Vec<char> = string.chars().collect();
        let len = chars.len();
        self.place_gap_at(offset, chars.len());
        (&mut self.buf[offset..offset + len]).copy_from_slice(&chars);
        self.gap_start += len;
    }

    /// Removes the text in the range of [start, end).
    pub fn delete(&mut self, start: usize, end: usize) {
        debug_assert!(start <= self.len());
        debug_assert!(end <= self.len() + 1);

        //  abcXYd___  =>  abcXY___d
        //     ^^             ^^
        self.place_gap_at(end, 0);

        //  abcXY___d  =>  abc_____d
        //     ^^
        self.gap_start = start;
    }

    fn place_gap_at(&mut self, offset: usize, min_len: usize) {
        debug_assert!(self.gap_start <= self.gap_end);

        // Move the gap.
        match offset.cmp(&self.gap_start) {
            // abcdef
            //    ^
            Ordering::Equal => {
                self.gap_start = offset;
            }
            // abc___def  =>  a___bcdef
            //  ^              ^
            Ordering::Less => {
                let range = offset..(self.gap_start);
                let copy_to = self.gap_end - range.len();
                self.buf.copy_within(range, copy_to);
                self.gap_start = offset;
                self.gap_end = copy_to;
            }
            // abc___def  =>  abcd___ef
            //        ^           ^
            Ordering::Greater => {
                let range = self.gap_end..(self.gap_end + offset - self.gap_start);
                let copy_to = self.gap_start;
                self.gap_start += range.len();
                self.gap_end = range.end;
                self.buf.copy_within(range, copy_to);
                let range = self.gap_end..(self.gap_end + offset - self.gap_start);
            }
        }

        if self.gap_end - self.gap_start < min_len {
            // Too small gap for min_len. Expand the buffer and the gap.
            let range = self.gap_end..self.buf.len();
            let gap_len = self.default_gap_len + min_len;
            self.buf.resize(self.buf.len() + gap_len, '.');
            self.buf.copy_within(range, self.gap_end + gap_len);
            self.gap_end += gap_len;
        }
    }
}

impl fmt::Debug for GapBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut text = String::new();
        text.extend(&self.buf[..self.gap_start]);
        text.extend("_".repeat(self.gap_end - self.gap_start).chars());
        text.extend(&self.buf[self.gap_end..]);
        write!(f, "GapBuffer(\"{}\")", text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insertion() {
        let mut gb = GapBuffer::new();
        gb.set_default_gap_len(2);
        gb.insert("abc", 0);
        assert_eq!(gb.text(), "abc");
        gb.insert("def", 3);
        assert_eq!(gb.text(), "abcdef");
        gb.insert("13", 0);
        assert_eq!(gb.text(), "13abcdef");
        gb.insert("2", 1);
        assert_eq!(gb.text(), "123abcdef");
        gb.insert("XYZ", 6);
        assert_eq!(gb.text(), "123abcXYZdef");
    }

    #[test]
    fn deletion() {
        let mut gb = GapBuffer::new();
        gb.set_default_gap_len(2);
        gb.insert("abcde", 0);
        gb.delete(0, 1);
        assert_eq!(gb.text(), "bcde");
        gb.delete(2, 4);
        assert_eq!(gb.text(), "bc");
        gb.insert("XYZ", 1);
        assert_eq!(gb.text(), "bXYZc");
        gb.delete(0, 5);
        assert_eq!(gb.text(), "");
    }
}
