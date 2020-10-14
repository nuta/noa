use crate::command_box::{File, Location};
use crate::rope::Range;
use crate::buffer::Buffer;
use grep::matcher::Matcher;
use grep::regex::{RegexMatcher, RegexMatcherBuilder};
use grep::searcher::{Searcher, SearcherBuilder, Sink, SinkMatch};
use ignore::WalkBuilder;
use std::path::Path;

pub const NUM_MATCHES_MAX: usize = 1000;

struct GrepSink<'a> {
    file: File,
    matcher: &'a RegexMatcher,
    locations: &'a mut Vec<Location>,
}

impl<'a> GrepSink<'a> {
    pub fn new(
        file: File,
        matcher: &'a RegexMatcher,
        locations: &'a mut Vec<Location>,
    ) -> GrepSink<'a> {
        GrepSink {
            file,
            matcher,
            locations,
        }
    }
}

impl<'a> Sink for GrepSink<'a> {
    type Error = std::io::Error;
    fn matched(&mut self, _searcher: &Searcher, sm: &SinkMatch) -> Result<bool, Self::Error> {
        let text = std::str::from_utf8(sm.bytes()).unwrap();
        let m = self.matcher.find(text.as_bytes())?.unwrap();
        let matched_text = &text[m.start()..m.end()];
        let matched_text_count = matched_text.chars().count();

        let start_y = (sm.line_number().unwrap() as usize) - 1;
        let start_x = text[..m.start()].chars().count();

        let end_x = matched_text
            .rfind('\n')
            .map(|x| matched_text_count - x - 1)
            .unwrap_or_else(|| matched_text_count);
        let end_y = start_y + matched_text.matches('\n').count();

        let range = Range::new(start_y, start_x, end_y, end_x);
        self.locations.push(Location { file: self.file.clone(), range });
        Ok(self.locations.len() < NUM_MATCHES_MAX)
    }
}

fn build_matcher(pat: &str) -> Result<RegexMatcher, Box<dyn std::error::Error>> {
    Ok(RegexMatcherBuilder::new()
        .case_smart(true)
        .multi_line(true)
        .build(pat)?)
}

fn build_literal_matcher(needle: &str) -> Result<RegexMatcher, Box<dyn std::error::Error>> {
    // XXX: RegexMatcherBuilder::build_literals() does not work. Escape special
    //      characters by ourselves.
    let mut escaped = String::with_capacity(needle.len() + 16);
    for ch in needle.chars() {
        if ch.is_ascii_punctuation() {
            escaped.push('\\');
        }
        escaped.push(ch);
    }

    build_matcher(&escaped)
}

fn build_searcher() -> Searcher {
    SearcherBuilder::new()
        .multi_line(true)
        .line_number(true)
        .build()
}

fn grep_buffer_by_matcher(dir: &Path, matcher: RegexMatcher) -> Vec<Location> {
    let mut searcher = build_searcher();
    let mut locs = Vec::new();
    let walker = WalkBuilder::new(dir).build();
    for e in walker {
        if let Ok(e) = e {
            // Ignore non-file entries.
            match e.file_type() {
                Some(file_type) if file_type.is_dir() => continue,
                _ => (),
            }

            let path = e.into_path();
            let display_name = path.to_str().unwrap().to_owned();
            let file = File {
                display_name,
                path: path.to_owned(),
                buffer_id: None,
            };
            let sink = GrepSink::new(file, &matcher, &mut locs);
            searcher.search_path(&matcher, &path, sink).ok();
        }

        if locs.len() >= NUM_MATCHES_MAX {
            break;
        }
    }

    locs
}

pub fn grep_buffer(buffer: &Buffer, pat: &str) -> Result<Vec<Location>, Box<dyn std::error::Error>> {
    let matcher = build_matcher(pat)?;
    let mut searcher = build_searcher();
    let mut locs = Vec::new();
    let file = File {
        display_name: buffer.name().to_owned(),
        buffer_id: Some(buffer.id()),
        path: buffer.tmpfile().to_path_buf(),
    };
    let sink = GrepSink::new(file, &matcher, &mut locs);
    searcher.search_slice(&matcher, buffer.text().as_bytes(), sink)?;
    Ok(locs)
}

pub fn grep_dir(dir: &Path, needle: &str) -> Result<Vec<Location>, Box<dyn std::error::Error>> {
    let matcher = build_literal_matcher(needle)?;
    Ok(grep_buffer_by_matcher(dir, matcher))
}

pub fn grep_dir_by_regex(dir: &Path, pat: &str) -> Result<Vec<Location>, Box<dyn std::error::Error>> {
    let matcher = build_matcher(pat)?;
    Ok(grep_buffer_by_matcher(dir, matcher))
}

pub fn list_files(dir: &Path, pat: &str) -> Vec<File> {
    let mut files = Vec::new();
    let walker = WalkBuilder::new(dir).build();
    for e in walker {
        if let Ok(e) = e {
            let path = e.into_path();
            let display_name = path.to_str().unwrap().to_owned();

            // TODO: fuzzy match
            if display_name.contains(pat) {
                files.push(File { display_name, path, buffer_id: None });
            }

            if files.len() >= NUM_MATCHES_MAX {
                break;
            }
        }
    }

    files
}
