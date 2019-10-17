use std::cmp::min;
use std::ops::Range;
use std::fs;
use std::path::{Path, PathBuf};
use crate::buffer::{Buffer, Line};
use crate::highlight::Highlight;
use crate::screen::Position;
use crate::utils::report_exec_time;
use syntect::highlighting::Style;
use unicode_width::UnicodeWidthStr;

pub struct File {
    /// The file path. It's `None` if the buffer is pseudo one (e.g.
    /// scratch).
    path: Option<PathBuf>,
    /// The file contents.
    buffer: Buffer,
    /// The highlighter state for the buffer. It's `None` if the highlighting
    /// for the file is disabled.
    highlight: Option<Highlight>,
    /// The line from which the frontend needs to redraw.
    line_modified: usize,
}

impl File {
    pub fn pseudo_file(name: &str) -> File {
        File {
            path: None,
            highlight: None,
            buffer: Buffer::new(name),
            line_modified: 0,
        }
    }

    pub fn open_file(name: &str, path: &Path) -> std::io::Result<File> {
        let handle = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path);

        let buffer = match handle {
            Ok(handle) => Buffer::from_file(name, &handle)?,
            Err(err) => {
                match err.kind() {
                    // TODO: Check the permission.
                    std::io::ErrorKind::NotFound => {
                        Buffer::new(name)
                    }
                    _ => {
                        return Err(err);
                    }
                }
            },

        };

        Ok(File {
            path: Some(path.to_owned()),
            buffer,
            highlight: None,
            line_modified: 0,
        })
    }

    pub fn set_highlight(&mut self, highlight: Option<Highlight>) {
        self.highlight = highlight;
        self.update_highlight(0);
    }

    pub fn update_highlight(&mut self, line_from: usize) {
        report_exec_time("highlight", || {
            let buffer = &self.buffer;
            if let Some(ref mut highlight) = self.highlight {
                highlight.parse(line_from, buffer);
            }
        });
    }

    pub fn highlight<'a>(
        &'a self,
        lineno: usize,
        column: usize,
        display_width: usize
    ) -> HighlightedSpans<'a> {
        HighlightedSpans {
            line: self.buffer.line_at(lineno),
            spans: self.highlight.as_ref().map(|h| h.lines()[lineno].spans().iter()),
            column,
            remaining_width: display_width,
            char_index: 0,
        }
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        let path = match &self.path {
            Some(path) => path,
            None => return Ok(()),
        };

        trace!("saving the buffer to a file: {}", path.display());
        let mut handle = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        self.buffer.write_to_file(&mut handle)
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn insert(&mut self, pos: &Position, ch: char) {
        self.buffer.insert(pos, ch);
        self.line_modified = min(self.line_modified, pos.line);
    }

    pub fn backspace(&mut self, pos: &Position) -> Option<usize> {
        if pos.column == 0 && pos.line > 0 {
            self.line_modified = min(self.line_modified, pos.line - 1);
        } else {
            self.line_modified = min(self.line_modified, pos.line);
        }

        self.buffer.backspace(pos)
    }

    pub fn delete(&mut self, pos: &Position) {
        self.buffer.delete(pos);
        self.line_modified = min(self.line_modified, pos.line);
    }

    pub fn line_modified(&self) -> usize {
        self.line_modified
    }

    pub fn reset_line_modified(&mut self) {
        self.line_modified = self.buffer().num_lines().saturating_sub(1);
    }
}

pub struct HighlightedSpans<'a> {
    line: &'a Line,
    spans: Option<std::slice::Iter<'a, (Style, Range<usize>)>>,
    column: usize,
    remaining_width: usize,
    char_index: usize,
}

impl<'a> Iterator for HighlightedSpans<'a> {
    type Item = (Option<Style>, &'a str, usize);
    // FIXME: Support long line.
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut iter) = self.spans {
            if self.remaining_width == 0 {
                // Already reached to the end of column in the screen.
                return None;
            }

            while let Some(span) = iter.next() {
                let style = Some(span.0);

                // Exclude newline characters.
                if span.1.start >= self.line.len() {
                    return None;
                }

                let span_start = span.1.start;
                let span_end = min(span.1.end, self.line.len());
                if span_start == span_end {
                    // This should would not happen...
                    continue;
                }

                let text = &self.line[span_start..span_end];
                let num_chars = text.chars().count();
                let width = UnicodeWidthStr::width_cjk(text);

                if self.char_index >= self.column {
                    if self.remaining_width < width {
                        // Reached to the end of screen (x-axis).
                        let mut truncated_span_end = span_end - 1;
                        let mut truncated_width = width;
                        let mut truncated_text =
                            &self.line[span_start..truncated_span_end];
                        while truncated_width > self.remaining_width {
                            truncated_span_end -= 1;
                            truncated_width =
                                UnicodeWidthStr::width_cjk(truncated_text);
                            truncated_text =
                                &self.line[span_start..truncated_span_end];
                        }

                        self.remaining_width = 0;
                        return Some((style, truncated_text, truncated_width));
                    } else {
                        // The span is entirely displayed in the screen.
                        self.remaining_width -= width;
                        return Some((style, text, width));
                    }
                } else if self.column - self.char_index < width {
                    // The span is partially displayed in the screen.
                    self.char_index = self.column;
                    let start =
                        min(span_start, self.column);
                    let end =
                        min(span_end, start + (width - (self.column + self.char_index)));
                    let text = &self.line[start..end];
                    let width = UnicodeWidthStr::width_cjk(text);
                    self.remaining_width -= width;
                    return Some((style, text, width));
                } else {
                    // The span is out of the screen. SKip it.
                    self.char_index += num_chars;
                }
            }

            // An empty line.
            None
        } else {
            // Highlighting is disabled.
            if self.remaining_width == 0 {
                None
            } else {
                let width = UnicodeWidthStr::width_cjk(self.line.as_str());
                self.remaining_width = 0;
                Some((None, self.line.as_str(), width))
            }
        }
    }
}
