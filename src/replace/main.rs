#[macro_use]
extern crate log;

mod path_scanner;
mod search_query;
mod ui;

use noa_common::logger::install_logger;
use regex::Regex;
use std::path::PathBuf;
use structopt::StructOpt;

use crate::{path_scanner::PathScanner, ui::Ui};

#[derive(StructOpt)]
struct Opt {
    #[structopt(long, parse(from_os_str))]
    dir: Option<PathBuf>,
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
    use grep::searcher::sinks::UTF8;
    use grep::searcher::Searcher;

    let query = "oops";

    let matcher = RegexMatcher::new(query).unwrap();

    let path_scanner = PathScanner::new(&base_dir);

    path_scanner.scan(Box::new(|path: PathBuf| {
        // println!("{}", path.display());

        Searcher::new()
            .search_path(
                &matcher,
                &path,
                UTF8(|lineno, line| {
                    let m = matcher.find(line.as_bytes())?.unwrap();
                    let before_text = &line[..m.start()];
                    let matched_text = &line[m.start()..m.end()];
                    let after_text = &line[m.end()..];
                    println!(
                        "{}:{}: {}\x1b[1;31m{}\x1b[0m{}",
                        path.display(),
                        lineno,
                        before_text,
                        matched_text,
                        after_text
                    );
                    Ok(true)
                }),
            )
            .unwrap();

        true
    }));
}
