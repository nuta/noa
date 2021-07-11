use std::fs::OpenOptions;
use std::path::Path;

use regex::Regex;
use ropey::iter::Chunks;

use crate::cursor::{Point, Range};

#[derive(Clone, Debug, PartialEq)]
pub struct Rope {
    inner: ropey::Rope,
    modified_line: Option<usize>,
    cached_num_lines: usize,
    version: usize,
}

impl Rope {
    pub fn new() -> Rope {
        Rope {
            inner: ropey::Rope::new(),
            modified_line: None,
            cached_num_lines: 1,
            version: 1,
        }
    }

    pub fn from_reader<T: std::io::Read>(reader: T) -> std::io::Result<Rope> {
        let inner = ropey::Rope::from_reader(reader)?;
        let cached_num_lines = inner.len_lines();
        Ok(Rope {
            inner,
            modified_line: None,
            cached_num_lines,
            version: 1,
        })
    }

    #[cfg(test)]
    pub fn from_str(str: &str) -> Rope {
        let mut rope = Rope::new();
        rope.insert(Point::new(0, 0), str);
        rope
    }

    pub fn is_empty(&self) -> bool {
        self.inner.len_chars() == 0
    }

    /// Returns the number of characters in the buffer.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner.len_chars()
    }

    /// Returns the number of bytes in the buffer.
    pub fn len_bytes(&self) -> usize {
        self.inner.len_bytes()
    }

    /// Returns the number of characters in the buffer.
    pub fn num_lines(&self) -> usize {
        self.cached_num_lines
    }

    /// Returns the number of characters in a line except new line characters.
    pub fn line_len(&self, line: usize) -> usize {
        if line == self.num_lines() {
            0
        } else {
            self.line(line).len_chars()
        }
    }

    pub fn chunks(&self) -> Chunks {
        self.inner.chunks()
    }

    pub fn version(&self) -> usize {
        self.version
    }

    pub fn modified_line(&self) -> &Option<usize> {
        &self.modified_line
    }

    pub fn reset_modified_line(&mut self) {
        self.modified_line = None;
    }

    pub fn text(&self) -> String {
        self.inner.to_string()
    }

    pub fn save_into_file(&self, path: &Path) -> std::io::Result<()> {
        let f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        self.inner.write_to(f)
    }

    pub fn insert(&mut self, pos: Point, string: &str) {
        self.inner.insert(self.index_in_rope(pos), string);
        self.on_modified(pos.y);
    }

    pub fn insert_char(&mut self, pos: Point, ch: char) {
        self.inner.insert_char(self.index_in_rope(pos), ch);
        self.on_modified(pos.y);
    }

    pub fn clear(&mut self) {
        self.inner.remove(0..self.inner.len_chars());
    }

    pub fn remove(&mut self, range: &Range) {
        let start = self.index_in_rope(range.front());
        let end = self.index_in_rope(range.back());
        self.inner.remove(start..end);
        self.on_modified(range.front().y);
    }

    fn on_modified(&mut self, start_y: usize) {
        self.cached_num_lines = self.inner.len_lines();
        self.modified_line = Some(start_y);
        self.version += 1;
    }

    /// Returns a line except new line characters.
    pub fn line(&self, line: usize) -> ropey::RopeSlice {
        let slice = self.inner.line(line);

        // The slice contains newline characters. Trim them.
        let mut len = slice.len_chars();
        while len > 0 {
            if slice.char(len - 1) != '\n' {
                break;
            }

            len -= 1;
        }

        slice.slice(..len)
    }

    pub fn chars(&self) -> ropey::iter::Chars<'_> {
        self.inner.chars()
    }

    pub fn sub_str(&self, range: &Range) -> ropey::RopeSlice {
        let start = self.index_in_rope(range.front());
        let end = self.index_in_rope(range.back());
        self.inner.slice(start..end)
    }

    pub fn word_at(&self, pos: &Point) -> Option<(Range, String)> {
        if pos.y >= self.num_lines() {
            return None;
        }

        let mut word = String::new();
        let mut start = 0;
        let mut end = 0;
        for (i, ch) in self.line(pos.y).chars().enumerate() {
            if is_word_char(ch) {
                word.push(ch);
                end = i + 1;
            } else if i >= pos.x {
                break;
            } else {
                word.clear();
                start = i + 1;
            }
        }

        if word.is_empty() {
            None
        } else {
            Some((Range::new(pos.y, start, pos.y, end), word))
        }
    }

    pub fn prev_word_at(&self, pos: &Point) -> Option<Range> {
        if pos.x == 0 {
            if pos.y > 0 {
                return Some(Range::new(pos.y - 1, self.line_len(pos.y - 1), pos.y, 0));
            } else {
                return None;
            }
        }

        let mut state = None;
        let mut start = 0;
        for (i, ch) in self.line(pos.y).chars().take(pos.x).enumerate() {
            let current_state = is_word_char(ch);
            match state {
                Some(prev_state) if prev_state == current_state => {}
                _ => {
                    start = i;
                }
            }

            state = Some(current_state);
        }

        Some(Range::new(pos.y, start, pos.y, pos.x))
    }

    pub fn prev_word_end(&self, pos: &Point) -> Point {
        assert!(pos.y < self.num_lines());

        let line = self.line(pos.y).to_string();
        let line_len = line.chars().count();
        let end = line
            .chars()
            .rev()
            .skip(line_len - pos.x)
            .enumerate()
            .skip_while(|(_, ch)| !is_word_char(*ch))
            .skip_while(|(_, ch)| is_word_char(*ch))
            .next()
            .map(|(i, _)| pos.x - i)
            .unwrap_or(0);

        Point::new(pos.y, end)
    }

    pub fn next_word_end(&self, pos: &Point) -> Point {
        assert!(pos.y < self.num_lines());

        let line = self.line(pos.y);
        let end = line
            .chars()
            .skip(pos.x)
            .enumerate()
            .skip_while(|(_, ch)| !is_word_char(*ch))
            .skip_while(|(_, ch)| is_word_char(*ch))
            .next()
            .map(|(i, _)| pos.x + i)
            .unwrap_or_else(|| line.len_chars());

        Point::new(pos.y, end)
    }

    pub fn find_prev(&self, needle: &str, before: Option<Point>) -> Option<Range> {
        if needle.is_empty() {
            return None;
        }

        let heystack = self.text();
        let rfind_from = before
            .map(|pos| self.point_to_byte_off(pos))
            .unwrap_or_else(|| heystack.len());

        heystack[..rfind_from].rfind(needle).map(|byte_offset| {
            let start = self.byte_off_to_point(byte_offset);
            // TODO: pattern might include '\n'
            let end = Point::new(start.y, start.x + needle.chars().count());
            Range::from_points(start, end)
        })
    }

    pub fn find_next(&self, needle: &str, after: Option<Point>) -> Option<Range> {
        self.find_all(needle, after).next()
    }

    pub fn find_next_by_regex(
        &self,
        pattern: &str,
        after: Option<Point>,
    ) -> Result<Option<Range>, regex::Error> {
        if pattern.is_empty() {
            return Ok(None);
        }

        self.find_all_by_regex(pattern, after)
            .map(|mut iter| iter.next())
    }

    pub fn find_prev_by_regex(
        &self,
        pattern: &str,
        before: Option<Point>,
    ) -> Result<Option<Range>, regex::Error> {
        let iter = self.find_all_by_regex(pattern, None)?;
        let mut prev = None;
        for range in iter {
            match before {
                Some(before) if range.front() < before => {
                    prev = Some(range);
                }
                Some(_) => {
                    return Ok(prev);
                }
                None => {
                    prev = Some(range);
                }
            }
        }

        Ok(prev)
    }

    pub fn find_all<'a>(&'a self, needle: &str, after: Option<Point>) -> SearchIter<'a> {
        if needle.is_empty() {
            return SearchIter::new_empty(self);
        }

        let needle = needle.to_owned();
        SearchIter::new(
            self,
            move |heystack| {
                heystack.find(&needle).map(|byte_offset| SearchMatch {
                    byte_offset,
                    matched_len: needle.len(),
                })
            },
            after,
        )
    }

    pub fn find_all_by_regex<'a>(
        &'a self,
        pattern: &str,
        after: Option<Point>,
    ) -> Result<SearchIter<'a>, regex::Error> {
        if pattern.is_empty() {
            return Ok(SearchIter::new_empty(self));
        }

        let re = Regex::new(pattern)?;
        Ok(SearchIter::new(
            self,
            move |heystack| {
                re.find(heystack).map(|m| SearchMatch {
                    byte_offset: m.start(),
                    matched_len: heystack[m.start()..m.end()].chars().count(),
                })
            },
            after,
        ))
    }

    fn index_in_rope(&self, pos: Point) -> usize {
        let x = if pos.x == std::usize::MAX {
            self.line_len(pos.y)
        } else {
            pos.x
        };

        self.inner.line_to_char(pos.y) + x
    }

    fn point_to_byte_off(&self, pos: Point) -> usize {
        let line_off = self.inner.line_to_byte(pos.y);
        let line = self.inner.line(pos.y);
        let column_off = line.char_to_byte(pos.x);
        line_off + column_off
    }

    fn byte_off_to_point(&self, offset: usize) -> Point {
        let lineno = self.inner.byte_to_line(offset);
        let line_off = self.inner.line_to_byte(lineno);
        let line = self.inner.line(lineno);
        let colno = line.byte_to_char(offset - line_off);
        Point::new(lineno, colno)
    }
}

struct SearchMatch {
    byte_offset: usize,
    matched_len: usize,
}

pub struct SearchIter<'a> {
    rope: &'a Rope,
    text: String,
    current: Option<Point>,
    searcher: Box<dyn Fn(&str) -> Option<SearchMatch>>,
    reached_eof: bool,
}

impl<'a> SearchIter<'a> {
    fn new(
        rope: &'a Rope,
        searcher: impl Fn(&str) -> Option<SearchMatch> + 'static,
        after: Option<Point>,
    ) -> SearchIter<'a> {
        SearchIter {
            rope,
            text: rope.text(), // FIXME: Avoid using time-consuming .text()
            current: after,
            searcher: Box::new(searcher),
            reached_eof: false,
        }
    }

    fn new_empty(rope: &'a Rope) -> SearchIter<'a> {
        SearchIter {
            rope,
            text: "".to_owned(),
            current: None,
            searcher: Box::new(|_| None),
            reached_eof: true,
        }
    }
}

impl<'a> Iterator for SearchIter<'a> {
    type Item = Range;
    fn next(&mut self) -> Option<Self::Item> {
        if self.reached_eof {
            return None;
        }

        let start = match self.current {
            Some(Point { y, x }) => {
                if y == self.rope.num_lines() - 1 && x == self.rope.line_len(y) {
                    // EOF.
                    self.reached_eof = true;
                    return None;
                }

                self.rope.point_to_byte_off(Point::new(y, x + 1))
            }
            None => 0,
        };

        let m = (*self.searcher)(&self.text[start..]);
        let (pos, range) = match m {
            Some(SearchMatch {
                byte_offset,
                matched_len,
            }) => {
                let pos = self.rope.byte_off_to_point(start + byte_offset);
                // TODO: pattern might include '\n'
                let end = Point::new(pos.y, pos.x + matched_len);
                (Some(pos), Some(Range::from_points(pos, end)))
            }
            None => (None, None),
        };

        self.current = pos;
        range
    }
}

fn is_word_char(ch: char) -> bool {
    char::is_ascii_alphanumeric(&ch) || ch == '_'
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn point_to_byte_off() {
        // ABC
        // あaア
        // xy
        //
        // (Both あ and ア occupy 3 bytes.)
        let rope = Rope::from_str("ABC\nあaア\nxy");
        assert_eq!(rope.point_to_byte_off(Point::new(0, 2)), 2);
        assert_eq!(rope.point_to_byte_off(Point::new(1, 0)), 4);
        assert_eq!(rope.point_to_byte_off(Point::new(1, 1)), 7);
        assert_eq!(rope.point_to_byte_off(Point::new(2, 2)), 14);
    }

    #[test]
    fn byte_off_to_point() {
        // ABC
        // あaア
        // xy
        //
        // (Both あ and ア occupy 3 bytes.)
        let rope = Rope::from_str("ABC\nあaア\nxy");
        assert_eq!(rope.byte_off_to_point(4), Point::new(1, 0));
        assert_eq!(rope.byte_off_to_point(2), Point::new(0, 2));
        assert_eq!(rope.byte_off_to_point(7), Point::new(1, 1));
        assert_eq!(rope.byte_off_to_point(14), Point::new(2, 2));
    }

    #[test]
    fn find_next() {
        // It's ABC
        // これはABCです
        // ABC
        //
        // (あ and its friends occupy 3 bytes.)
        let rope = Rope::from_str("It's ABC\nこれはABCです\nABC");
        assert_eq!(rope.find_next("", None), None);
        assert_eq!(rope.find_next("It's", None), Some(Range::new(0, 0, 0, 4)));
        assert_eq!(rope.find_next("ABC", None), Some(Range::new(0, 5, 0, 8)));
        assert_eq!(
            rope.find_next("ABC", Some(Point::new(0, 4))),
            Some(Range::new(0, 5, 0, 8))
        );
        assert_eq!(
            rope.find_next("ABC", Some(Point::new(0, 5))),
            Some(Range::new(1, 3, 1, 6))
        );
        assert_eq!(
            rope.find_next("ABC", Some(Point::new(1, 3))),
            Some(Range::new(2, 0, 2, 3))
        );
        assert_eq!(rope.find_next("ABC", Some(Point::new(2, 0))), None);
        assert_eq!(rope.find_next("ABC", Some(Point::new(2, 3))), None);
    }

    #[test]
    fn find_prev() {
        // It's ABC
        // これはABCです
        // ABC
        //
        // (あ and its friends occupy 3 bytes.)
        let rope = Rope::from_str("It's ABC\nこれはABCです\nABC");
        assert_eq!(rope.find_prev("", None), None);
        assert_eq!(rope.find_prev("ABC", None), Some(Range::new(2, 0, 2, 3)));
        assert_eq!(rope.find_prev("ABC", Some(Point::new(0, 5))), None);
        assert_eq!(
            rope.find_prev("ABC", Some(Point::new(0, 8))),
            Some(Range::new(0, 5, 0, 8))
        );
        assert_eq!(
            rope.find_prev("ABC", Some(Point::new(2, 0))),
            Some(Range::new(1, 3, 1, 6))
        );
    }

    #[test]
    fn find_by_regex() {
        // It's ABC
        // これはABCです
        // ABC
        //
        // (あ and its friends occupy 3 bytes.)
        let rope = Rope::from_str("It's ABC\nこれはABCです\nABC");
        assert_eq!(rope.find_next_by_regex("", None), Ok(None));
        assert_eq!(
            rope.find_next_by_regex("...", None),
            Ok(Some(Range::new(0, 0, 0, 3)))
        );
        assert_eq!(
            rope.find_next_by_regex("A.C", None),
            Ok(Some(Range::new(0, 5, 0, 8)))
        );
        assert_eq!(
            rope.find_next_by_regex("A..", Some(Point::new(0, 4))),
            Ok(Some(Range::new(0, 5, 0, 8)))
        );
        assert_eq!(
            rope.find_next_by_regex("こ", Some(Point::new(0, 0))),
            Ok(Some(Range::new(1, 0, 1, 1)))
        );
        assert_eq!(
            rope.find_next_by_regex("A.+", Some(Point::new(0, 5))),
            Ok(Some(Range::new(1, 3, 1, 8)))
        );
        assert_eq!(
            rope.find_next_by_regex(".B.", Some(Point::new(1, 3))),
            Ok(Some(Range::new(2, 0, 2, 3)))
        );
        assert_eq!(
            rope.find_next_by_regex("A[BC]{2}", Some(Point::new(2, 0))),
            Ok(None)
        );
        assert_eq!(
            rope.find_next_by_regex("A[BC]{2}", Some(Point::new(2, 3))),
            Ok(None)
        );
    }

    #[test]
    fn find_prev_by_regex() {
        // It's ABC
        // これはABCです
        // ABC
        //
        // (あ and its friends occupy 3 bytes.)
        let rope = Rope::from_str("It's ABC\nこれはABCです\nABC");
        assert_eq!(rope.find_prev_by_regex("", None), Ok(None));
        assert_eq!(
            rope.find_prev_by_regex("ABC", None),
            Ok(Some(Range::new(2, 0, 2, 3)))
        );
        assert_eq!(
            rope.find_prev_by_regex("ABC", Some(Point::new(0, 5))),
            Ok(None)
        );
        assert_eq!(
            rope.find_prev_by_regex("ABC", Some(Point::new(0, 8))),
            Ok(Some(Range::new(0, 5, 0, 8)))
        );
        assert_eq!(
            rope.find_prev_by_regex("ABC", Some(Point::new(2, 0))),
            Ok(Some(Range::new(1, 3, 1, 6)))
        );
    }

    #[test]
    fn find_all() {
        // It's ABC
        // これはABCです
        // ABC
        //
        // (あ and its friends occupy 3 bytes.)
        let rope = Rope::from_str("It's ABC\nこれはABCです\nABC");
        assert_eq!(rope.find_all("", None).collect::<Vec<Range>>(), vec![]);
        assert_eq!(
            rope.find_all("It's", None).collect::<Vec<Range>>(),
            vec![Range::new(0, 0, 0, 4)]
        );
        assert_eq!(
            rope.find_all("ABC", None).collect::<Vec<Range>>(),
            vec![
                Range::new(0, 5, 0, 8),
                Range::new(1, 3, 1, 6),
                Range::new(2, 0, 2, 3),
            ]
        );
        assert_eq!(
            rope.find_all("ABC", Some(Point::new(1, 2)))
                .collect::<Vec<Range>>(),
            vec![Range::new(1, 3, 1, 6), Range::new(2, 0, 2, 3),]
        );
    }

    #[test]
    fn find_all_by_regex() {
        // It's ABC
        // これはABCです
        // ABC
        //
        // (あ and its friends occupy 3 bytes.)
        let rope = Rope::from_str("It's ABC\nこれはABCです\nABC");
        assert_eq!(
            rope.find_all_by_regex("", None)
                .map(|iter| iter.collect::<Vec<Range>>()),
            Ok(vec![])
        );
        assert_eq!(
            rope.find_all_by_regex("It's", None)
                .map(|iter| iter.collect::<Vec<Range>>()),
            Ok(vec![Range::new(0, 0, 0, 4)])
        );
        assert_eq!(
            rope.find_all_by_regex("A[BC]{2}", None)
                .map(|iter| iter.collect::<Vec<Range>>()),
            Ok(vec![
                Range::new(0, 5, 0, 8),
                Range::new(1, 3, 1, 6),
                Range::new(2, 0, 2, 3),
            ])
        );
        assert_eq!(
            rope.find_all_by_regex("A[Bx].", Some(Point::new(1, 2)))
                .map(|iter| iter.collect::<Vec<Range>>()),
            Ok(vec![Range::new(1, 3, 1, 6), Range::new(2, 0, 2, 3),])
        );
    }
}
