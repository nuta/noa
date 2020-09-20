use std::path::Path;
use grep::matcher::Matcher;
use grep::regex::RegexMatcher;
use grep::searcher::{Searcher, Sink, SinkMatch};
use ignore::WalkBuilder;
use crate::command_box::{Location, File};

struct GrepSink<'a> {
    locations: &'a mut Vec<Location>,
}

impl<'a> GrepSink<'a> {
    pub fn new(locations: &'a mut Vec<Location>) -> GrepSink<'a> {
        GrepSink {
            locations,
        }
    }
}

impl<'a> Sink for GrepSink<'a> {
    type Error = std::io::Error;
    fn matched(
        &mut self,
        _searcher: &Searcher,
        m: &SinkMatch,
    ) -> Result<bool, Self::Error> {
        /*
        let rest =  absolute_byte_offset;
        let start = line.len() - (&line[m.start()..]).len();
        let end = (&line[m.start()..m.end()]).len();
        let range = Range::new(y, start, y, end);
        trace!("range={:?}, text='{}'", range, &line[m.start()..m.end()]);
        */
        trace!("bytes={}", std::str::from_utf8(m.bytes()).unwrap());
        Ok(true)
    }
}

pub fn grep_dir(dir: &Path, pat: &str) -> Result<Vec<Location>, Box<dyn std::error::Error>> {
    let matcher = RegexMatcher::new(pat)?;
    let mut searcher = Searcher::new();
    let mut locs = Vec::new();
    let walker = WalkBuilder::new(dir).build();
    for e in walker {
        if let Ok(e) = e {
            trace!("e = {:?}", e);
            searcher.search_path(&matcher, e.into_path(), GrepSink::new(&mut locs));
        }
    }

    Ok(locs)
}

pub fn list_files(dir: &Path, pat: &str) -> Vec<File> {
    let mut files = Vec::new();
    let walker = WalkBuilder::new(dir).build();
    for e in walker {
        if let Ok(e) = e {
            let pathbuf = e.into_path().to_path_buf();
            let display_name = pathbuf.to_str().unwrap().to_owned();
            // TODO: fuzzy match
            if display_name.contains(pat) {
                files.push(File { display_name, path: pathbuf });
            }
        }
    }

    files
}
