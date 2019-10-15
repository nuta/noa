#![allow(dead_code)]

#[macro_use]
extern crate log;

mod buffer;
mod editor;
mod file;
mod fuzzy;
mod frontend;
mod frontends;
mod highlight;
mod logger;
mod plugin;
mod plugins;
mod screen;

use structopt::StructOpt;
use std::path::PathBuf;

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    /// The file to edit.
    #[structopt(parse(from_os_str))]
    file: PathBuf,
}

fn main() {
    logger::init();
    std::panic::set_hook(Box::new(|info| {
        error!("{}", info);
        error!("{:#?}", backtrace::Backtrace::new());
    }));

    let opt = Opt::from_args();

    trace!("starting noa...");
    let ui = frontends::terminal::Terminal::new();
    let mut editor = editor::Editor::new(ui);
    editor.add_plugin(plugins::PrimitivePlugin::new());
    editor.add_plugin(plugins::CommandMenuPlugin::new());
    editor.open_file(&opt.file).unwrap();
    editor.run();
}
