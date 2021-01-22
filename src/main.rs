#![allow(unused)]

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;
#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate maplit;

mod buffer;
mod editor;
mod editorconfig;
mod language;
mod rope;
mod terminal;

use simplelog::{CombinedLogger, LevelFilter, WriteLogger};
use std::fs::OpenOptions;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opt {
    #[structopt(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

fn main() {
    better_panic::install();

    let log_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(dirs::home_dir().unwrap().join(".noa.log"))
        .expect("failed to open ~/.noa.log");
    let log_level = if cfg!(debug_assertions) {
        LevelFilter::Trace
    } else {
        LevelFilter::Info
    };

    let log_config = simplelog::Config::default();
    CombinedLogger::init(vec![WriteLogger::new(log_level, log_config, log_file)]).unwrap();

    std::panic::set_hook(Box::new(|info| {
        use crossterm::{
            cursor,
            event::DisableMouseCapture,
            execute,
            terminal::{disable_raw_mode, LeaveAlternateScreen},
        };
        use std::io::{self, Write};

        let mut stdout = io::stdout();
        execute!(stdout, LeaveAlternateScreen).unwrap();
        execute!(stdout, DisableMouseCapture).ok();
        execute!(stdout, cursor::Show).unwrap();
        disable_raw_mode().ok();
        error!("{}", info);

        let panic_handler = better_panic::Settings::auto()
            .most_recent_first(false)
            .create_panic_handler();
        panic_handler(info);
        std::process::exit(1);
    }));

    trace!("starting noa...");
    let opt = Opt::from_args();
    let mut editor = editor::Editor::new();
    for path in opt.files.iter().rev() {
        editor.open_file(path);
    }

    editor.run();
}
