#[cfg(test)] #[macro_use] extern crate pretty_assertions;
#[macro_use] extern crate log;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate maplit;

mod buffer;
mod clipboard;
mod command_box;
mod editor;
mod rope;
mod terminal;
mod view;
mod worker;
mod completion;
mod lsp;
mod language;
mod highlight;
mod theme;
mod search;
mod fuzzy;
mod watcher;
mod editorconfig;
mod status_map;
mod helpers;

use std::env::current_dir;
use std::path::PathBuf;
use structopt::StructOpt;
use fern::{
    colors::{Color, ColoredLevelConfig}
};

#[derive(StructOpt)]
struct Opt {
    #[structopt(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

fn main() {
    let log_colors = ColoredLevelConfig::new()
        .info(Color::Green);
    let log_file = fern::log_file(dirs::home_dir().unwrap().join(".noa.log"))
        .expect("failed to open ~/.noa.log");
    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "\x1b[1m[{}\t]\x1b[0m \x1b[36m{}\x1b[0m: {}",
                log_colors.color(record.level()),
                record.file().unwrap_or_else(|| record.target()),
                message,
            ))
        })
        .chain(log_file)
        .apply()
        .expect("failed to initialize the logger");

    std::panic::set_hook(Box::new(|info| {
        error!("{}", info);
        error!("{:#?}", backtrace::Backtrace::new());
    }));

    trace!("starting noa...");
    let opt = Opt::from_args();
    let mut editor = editor::Editor::new(current_dir().unwrap());
    for file in opt.files.iter().rev() {
        editor.open_file(file);
    }

    editor.run();
}
