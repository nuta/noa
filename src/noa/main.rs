#![allow(unused)] // FIXME:

#[macro_use]
extern crate log;

#[macro_use]
extern crate noa_common;

use std::{path::PathBuf, process::Stdio, time::Duration};

use clap::Parser;
use editor::Editor;
use noa_common::logger::install_logger;
use noa_compositor::{
    compositor::Compositor,
    terminal::{Event, InputEvent, KeyCode, KeyModifiers},
};
use tokio::{
    sync::mpsc,
    time::{self, Instant},
};
use views::{buffer_view::BufferView, metaline_view::MetaLine};

mod actions;
mod clipboard;
mod config;
mod document;
mod editor;
mod notification;
mod views;

pub enum MainloopCommand {
    Quit,
    ExternalCommand(Box<std::process::Command>),
}

const FOREVER: Duration = Duration::from_secs(30 * 24 * 60 * 60 /* (almost) forever */);
const UNDO_TIMEOUT: Duration = Duration::from_millis(500);

async fn mainloop(mut editor: Editor) {
    let mut compositor = Compositor::new();
    let (mainloop_tx, mut mainloop_rx) = mpsc::unbounded_channel();
    compositor.add_frontmost_layer(Box::new(BufferView::new(mainloop_tx.clone())));
    compositor.add_frontmost_layer(Box::new(MetaLine::new()));

    let undo_timeout = time::sleep(FOREVER);
    tokio::pin!(undo_timeout);
    'outer: loop {
        trace_timing!("render", 5 /* ms */, {
            compositor.render(&mut editor);
        });

        let timeout = time::sleep(Duration::from_millis(5));
        tokio::pin!(timeout);

        // Handle all pending events until the timeout is reached.
        'inner: for i in 0.. {
            tokio::select! {
                biased;

                Some(command) = mainloop_rx.recv() => {
                    match command {
                        MainloopCommand::Quit => break 'outer,
                        MainloopCommand::ExternalCommand(mut cmd) => {
                            cmd.stdin(Stdio::inherit())
                            .stdout(Stdio::piped())
                            .stderr(Stdio::inherit());

                            let result = compositor.run_in_cooked_mode(&mut editor, || {
                                cmd.spawn().and_then(|child| child.wait_with_output())
                            }).await;

                            match result {
                                Ok(output) => {
                                    info!("output: {:?}", output);
                                }
                                Err(err) => notify_error!("failed to spawn: {}", err),
                            }
                        }
                    }
                }

                Some(ev) = compositor.receive_event() => {
                    trace_timing!("handle_event", 5 /* ms */, {
                        let prev_buffer = editor.current_document().raw_buffer().clone();

                        compositor.handle_event(&mut editor, ev);

                        let doc = editor.current_document();
                        if *doc.raw_buffer() != prev_buffer {
                            undo_timeout.as_mut().reset(Instant::now() + UNDO_TIMEOUT);
                        }
                    });
                }

                _ = &mut undo_timeout => {
                    editor.current_document_mut().save_undo();
                    undo_timeout.as_mut().reset(Instant::now() + FOREVER);
                }

                // No pending events.
                _ = futures::future::ready(()), if i > 0 => {
                    // Since we've already handled at least one event, if there're no
                    // pending events, we should break the loop to update the
                    // terminal contents.
                    break 'inner;
                }

                _ = &mut timeout, if i > 0 => {
                    // Taking too long to handle events. Break the loop to update the
                    // terminal contents.
                    break 'inner;
                }
            }
        }
    }
}

#[derive(Parser, Debug)]
struct Args {
    #[clap(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // TODO:
    // warm_up_search_cache();

    let mut editor = editor::Editor::new();

    for file in args.files {
        let doc = document::Document::open(&file)
            .await
            .expect("failed to open file");
        editor.add_and_switch_document(doc);
    }

    install_logger("main");

    tokio::spawn(mainloop(editor)).await;
}
