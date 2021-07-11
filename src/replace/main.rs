#![allow(unused)]

#[macro_use]
extern crate log;

mod path_scanner;
mod search_query;
mod ui;

use anyhow::Result;
use grep::{
    matcher::Match,
    searcher::{Searcher, Sink, SinkError, SinkMatch},
};
use noa_common::logger::install_logger;
use std::{
    fs::OpenOptions,
    io::{self, prelude::*, SeekFrom},
    ops::Range,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

use crate::{path_scanner::PathScanner, ui::Ui};

#[derive(StructOpt)]
struct Opt {
    #[structopt(long, parse(from_os_str))]
    dir: Option<PathBuf>,
}

fn do_replace(path: &Path, matches: &[Range<usize>], replacement: &str) -> Result<()> {
    // FIXME: FIXME: FIXME: TODO:
    return Ok(());

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)?;

    let mut text = String::new();
    file.read_to_string(&mut text)?;

    for range in matches.iter().rev() {
        text.replace_range(range.clone(), replacement);
    }

    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(text.as_bytes())?;
    Ok(())
}

pub struct Utf8Sink<F>(F)
where
    F: FnMut(u64, Range<usize>, &str) -> Result<bool, io::Error>;

impl<F> Sink for Utf8Sink<F>
where
    F: FnMut(u64, Range<usize>, &str) -> Result<bool, io::Error>,
{
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, io::Error> {
        let text = match std::str::from_utf8(mat.bytes()) {
            Ok(text) => text,
            Err(err) => {
                return Err(io::Error::error_message(err));
            }
        };
        (self.0)(
            mat.line_number().unwrap(),
            mat.bytes_range_in_buffer(),
            text,
        )
    }
}

#[tokio::main]
async fn main() {
    install_logger("replace");
    trace!("starting");

    let opt = Opt::from_args();
    let ui = Ui::new();
    let base_dir = opt.dir.unwrap_or_else(|| std::env::current_dir().unwrap());

    use grep::matcher::Matcher;
    use grep::regex::RegexMatcher;
    use grep::searcher::Searcher;

    let query = "oops";

    let matcher = RegexMatcher::new(query).unwrap();

    let path_scanner = PathScanner::new(&base_dir);

    path_scanner.scan(Box::new(|path: PathBuf| {
        // println!("{}", path.display());

        // Grep'ing in the file.
        let mut matches = Vec::new();
        Searcher::new()
            .search_path(
                &matcher,
                &path,
                Utf8Sink(|lineno, range, line| {
                    matcher
                        .find_iter(line.as_bytes(), |m| {
                            let before_text = &line[..m.start()];
                            let matched_text = &line[m.start()..m.end()];
                            let after_text = &line[m.end()..];
                            matches.push(range.start + m.start()..range.start + m.end());
                            println!(
                                "{}:{}: {}\x1b[1;31m{}\x1b[0m{}",
                                path.display(),
                                lineno,
                                before_text,
                                matched_text,
                                after_text
                            );

                            /* continue finding */
                            true
                        })
                        .unwrap();

                    /* continue searching */
                    Ok(true)
                }),
            )
            .unwrap();

        let replacement = "YAY";
        // We now got the list of matched ranges. Let's replace them.
        match do_replace(&path, &matches, replacement) {
            Ok(()) => {}
            Err(err) => {
                // TODO: error reporting
            }
        }

        true
    }));
}
