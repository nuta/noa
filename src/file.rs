use std::ops::Range;
use std::fs;
use std::path::{Path, PathBuf};
use crate::buffer::{Buffer, Line};
use crate::highlight::Highlight;
use syntect::highlighting::Style;

pub struct File {
    /// The file path. It's `None` if the buffer is pseudo one (e.g.
    /// scratch).
    path: Option<PathBuf>,
    /// The file contents.
    buffer: Buffer,
    /// The highlighter state for the buffer. It's `None` if the highlighting
    /// for the file is disabled.
    highlight: Option<Highlight>,
}

impl File {
    pub fn pseudo_file(name: &str) -> File {
        File {
            path: None,
            highlight: None,
            buffer: Buffer::new(name),
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
        })
    }

    pub fn set_highlight(&mut self, highlight: Option<Highlight>) {
        self.highlight = highlight;
        self.update_highlight(0);
    }

    pub fn update_highlight(&mut self, line_from: usize) {
        let buffer = &self.buffer;
        if let Some(ref mut highlight) = self.highlight {
            highlight.parse(line_from, buffer);
        }
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

    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
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
    type Item = (Option<Style>, &'a str);
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut iter) = self.spans {
            while let Some(span) = iter.next() {
                let style = Some(span.0);
                use std::cmp::min;
                // Exclude newline characters.
                let span_start = min(span.1.start, self.line.len().saturating_sub(1));
                let span_end = min(span.1.end, self.line.len());
                if span_start == 0 && span_end == 0 {
                    continue;
                }
                
                let text = &self.line[span_start..span_end];
                let num_chars = text.chars().count();
                let width = unicode_width::UnicodeWidthStr::width_cjk(text);

                if self.char_index >= self.column {
                    // The span is entirely displayed in the screen.
                    self.remaining_width -= width;
                    return Some((style, text));
                } else if self.column - self.char_index < width {
                    // The span is partially displayed in the screen.
                    self.char_index = self.column;
                    self.remaining_width -= width;
                    let start = 
                        min(span_start, self.column);
                    let end = 
                        min(span_end, start + (width - (self.column + self.char_index)));
                    let text = &self.line[start..end];
                    return Some((style, text));
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
                self.remaining_width = 0;
                Some((None, self.line.as_str()))
            }
        }
    }
}
