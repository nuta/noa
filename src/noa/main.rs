#![allow(unused)]

#[macro_use]
extern crate log;

#[macro_use]
extern crate noa_common;

use std::path::PathBuf;

use clap::Parser;
use noa_common::logger::install_logger;
use tokio::{
    fs::{create_dir_all, OpenOptions},
    io::AsyncWriteExt,
    sync::mpsc,
};

use crate::finder::warm_up_search_cache;

mod document;
mod editor;
mod finder;
mod ui;

#[derive(Parser, Debug)]
struct Args {
    #[clap(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if std::env::var("NOA_TOKIO_TRACE").is_ok() {
        console_subscriber::init();
    }

    // warm_up_search_cache();

    let mut editor = editor::Editor::new();

    for file in args.files {
        let doc = document::Document::open(&file)
            .await
            .expect("failed to open file");
        editor.add_document(doc);
    }

    install_logger("main");

    let mut ui = ui::Ui::new(editor);
    tokio::spawn(ui.run()).await;
}
