#![allow(unused)]

#[macro_use]
extern crate log;

#[macro_use]
extern crate noa_common;

use std::path::PathBuf;

use clap::Parser;
use noa_common::logger::{install_logger, prettify_backtrace};
use tokio::{
    fs::{create_dir_all, OpenOptions},
    io::AsyncWriteExt,
    sync::mpsc,
};

mod actions;
mod clipboard;
mod config;
mod document;
mod editor;
mod extcmd;
mod notification;
mod ui;

#[derive(Parser, Debug)]
struct Args {
    #[clap(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // warm_up_search_cache();

    let mut editor = editor::Editor::new();

    for file in args.files {
        let doc = document::Document::open(&file)
            .await
            .expect("failed to open file");
        let doc_id = doc.id;
        editor.add_document(doc);
        editor.switch_document(doc_id);
    }

    install_logger("main");

    let mut ui = ui::Ui::new(editor);
    tokio::spawn(ui.run()).await;
}
