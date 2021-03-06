#![allow(dead_code)]

#[macro_use]
extern crate log;

mod buffer;
mod diff;
mod editor;
mod editorconfig;
mod finder;
mod logger;
mod terminal;
mod highlight;
mod language;
mod helpers;
mod clipboard;
mod lsp;

use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "noa", about = "A simple terminal text editor.")]
struct Opt {
    /// The file to edit.
    #[structopt(parse(from_os_str))]
    files: Vec<PathBuf>,
    /// Disable terminal rendering (for testing).
    #[structopt(long)]
    headless: bool,
}

fn main() {
    logger::init();
    std::panic::set_hook(Box::new(|info| {
        error!("{}", info);
        error!("{:#?}", backtrace::Backtrace::new());
    }));

    let opt = Opt::from_args();

    trace!("starting noa...");
    let mut editor = editor::Editor::new(opt.headless);
    for file in opt.files {
        editor.open_file(&file);
    }
    editor.run();
}
