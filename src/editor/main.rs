#![feature(test)]
#![feature(vec_retain_mut)]

extern crate test;

#[macro_use]
extern crate log;

use std::path::PathBuf;

use clap::Parser;

use noa_common::{logger::install_logger, time_report::TimeReport};

use theme::parse_default_theme;

#[macro_use]
mod notification;

mod actions;
mod application;
mod clipboard;
mod completion;
mod document;
mod editor;
mod event_listener;
mod file_watch;
mod finder;
mod flash;
mod git;
mod hook;
mod job;
mod keybindings;
mod linemap;
mod movement;
mod plugins;
mod theme;
mod ui;
mod view;

#[derive(Parser, Debug)]
struct Args {
    #[clap(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    let _boot_time = TimeReport::new("boot time");

    // Parse the default theme here to print panics in stderr.
    parse_default_theme();

    install_logger("main");
    let args = Args::parse();

    let _workspace_dir = args
        .files
        .iter()
        .find(|path| path.is_dir())
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."));

    // let mut no_files_opened = true;
    // for path in args.files {
    //     if !path.is_dir() {
    //         match open_file(&mut compositor, &mut editor, &path, None) {
    //             Ok(id) => {
    //                 editor.documents.switch_by_id(id);
    //             }
    //             Err(err) => {
    //                 notify_anyhow_error!(err);
    //             }
    //         }

    //         no_files_opened = false;
    //     }
    // }

    // if no_files_opened {
    //     open_finder(&mut compositor, &mut editor);
    // }

    // compositor.render_to_terminal(&mut editor);
    // drop(boot_time);

    // Drop compoisitor first to restore the terminal.
    // notification::set_stdout_mode(true);
}
