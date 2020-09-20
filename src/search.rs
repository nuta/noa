use crate::command_box::{File, Location};
use crate::rope::{Cursor, Point, Range};
use grep::matcher::Matcher;
use grep::regex::{RegexMatcher, RegexMatcherBuilder};
use grep::searcher::{Searcher, SearcherBuilder, Sink, SinkMatch};
use ignore::WalkBuilder;
use std::path::Path;

struct GrepSink<'a> {
    path: &'a Path,
    matcher: &'a RegexMatcher,
    locations: &'a mut Vec<Location>,
}

impl<'a> GrepSink<'a> {
    pub fn new(
        path: &'a Path,
        matcher: &'a RegexMatcher,
        locations: &'a mut Vec<Location>,
    ) -> GrepSink<'a> {
        GrepSink {
            path,
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

        let start_y = sm.line_number().unwrap() as usize;
        let start_x = text[..m.start()].chars().count();

        let end_x = matched_text
            .rfind('\n')
            .map(|x| matched_text_count - x - 1)
            .unwrap_or_else(|| matched_text_count);
        let end_y = start_y + matched_text.matches('\n').count();

        let range = Range::new(start_y, start_x, end_y, end_x);

        let display_name = self.path.to_str().unwrap().to_owned();
        let file = File {
            display_name,
            path: self.path.to_owned(),
        };
        self.locations.push(Location { file, range });
        Ok(true)
    }
}

pub fn grep_dir(dir: &Path, pat: &str) -> Result<Vec<Location>, Box<dyn std::error::Error>> {
    let matcher = RegexMatcherBuilder::new()
        .case_smart(true)
        .multi_line(true)
        .build(pat)?;
    let mut searcher = SearcherBuilder::new()
        .multi_line(true)
        .line_number(true)
        .build();
    let mut locs = Vec::new();
    let walker = WalkBuilder::new(dir).build();
    for e in walker {
        if let Ok(e) = e {
            let path = e.into_path();
            let sink = GrepSink::new(&path, &matcher, &mut locs);
            searcher.search_path(&matcher, &path, sink);
        }
    }

    Ok(locs)
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
                files.push(File { display_name, path });
            }
        }
    }

    files
}
