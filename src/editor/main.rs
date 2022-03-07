#![feature(test)]
#![feature(vec_retain_mut)]
#![allow(unused)]

extern crate test;

#[macro_use]
extern crate log;

use std::{ops::ControlFlow, path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;

use editor::Editor;
use noa_common::{logger::install_logger, oops::OopsExt, time_report::TimeReport};
use noa_compositor::{terminal::Event, Compositor};
use theme::parse_default_theme;
use tokio::sync::{
    mpsc::{self, unbounded_channel, UnboundedSender},
    oneshot, Notify,
};
use ui::{
    buffer_view::BufferView,
    completion_view::CompletionView,
    finder_view::FinderView,
    meta_line_view::MetaLineView,
    prompt::prompt,
    prompt_view::{PromptMode, PromptView},
    too_small_view::TooSmallView,
};

use crate::job::CompletedJob;

#[macro_use]
mod notification;

mod actions;
mod clipboard;
mod completion;
mod document;
mod editor;
mod event_listener;
mod flash;
mod git;
mod job;
mod keybindings;
mod linemap;
mod movement;
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
    let boot_time = TimeReport::new("boot time");

    // Parse the default theme here to print panics in stderr.
    parse_default_theme();

    install_logger("main");
    let args = Args::parse();

    let workspace_dir = args
        .files
        .iter()
        .find(|path| path.is_dir())
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."));

    let render_request = Arc::new(Notify::new());
    let (notification_tx, mut notification_rx) = mpsc::unbounded_channel();
    let mut editor = editor::Editor::new(&workspace_dir, render_request.clone(), notification_tx);
    let mut compositor = Compositor::new();

    let mut open_finder = true;
    for path in args.files {
        if !path.is_dir() {
            match editor.open_file(&path, None) {
                Ok(id) => {
                    editor.documents.switch_by_id(id);
                }
                Err(err) => {
                    notify_anyhow_error!(err);
                }
            }

            open_finder = false;
        }
    }

    let (quit_tx, mut quit_rx) = unbounded_channel();
    let (force_quit_tx, mut force_quit_rx) = unbounded_channel();
    compositor.add_frontmost_layer(Box::new(TooSmallView::new("too small!")));
    compositor.add_frontmost_layer(Box::new(BufferView::new(quit_tx, render_request.clone())));
    compositor.add_frontmost_layer(Box::new(MetaLineView::new()));
    compositor.add_frontmost_layer(Box::new(FinderView::new(
        &mut editor,
        render_request.clone(),
        &workspace_dir,
    )));
    compositor.add_frontmost_layer(Box::new(PromptView::new()));
    compositor.add_frontmost_layer(Box::new(CompletionView::new()));

    if open_finder {
        compositor
            .get_mut_surface_by_name::<FinderView>("finder")
            .set_active(true);
    }

    compositor.render_to_terminal(&mut editor);
    drop(boot_time);

    let mut idle_timer = tokio::time::interval(Duration::from_millis(1200));
    loop {
        let mut skip_rendering = false;
        tokio::select! {
            biased;

            _ = force_quit_rx.recv() => {
                break;
           }

            Some(()) =  quit_rx.recv() => {
                check_if_dirty(&mut compositor, &mut editor, force_quit_tx.clone());
            }

            Some(ev) = compositor.recv_terminal_event() => {
                let _event_tick_time = Some(TimeReport::new("I/O event handling"));
                match ev {
                    Event::Input(input) => {
                        compositor.handle_input(&mut editor, input);
                    }
                    Event::Resize { height, width } => {
                        compositor.resize_screen(height, width);
                    }
                }
            }

            Some(noti) = notification_rx.recv() => {
                trace!("proxy notification: {:?}", noti);
                editor.handle_notification(noti);
            }

            Some(completed) = editor.jobs.get_completed() => {
                match completed {
                    CompletedJob::Completed(callback) => {
                        callback(&mut editor, &mut compositor);
                    }
                    CompletedJob::Notified { id, mut callback } => {
                        callback(&mut editor, &mut compositor);
                        editor.jobs.insert_back_notified(id, callback);
                    }
                }
            }

            _ = render_request.notified() => {
            }

            _ = idle_timer.tick()  => {
                editor.documents.current_mut().idle_job();
                skip_rendering = true;
            }
        }

        if !skip_rendering {
            compositor.render_to_terminal(&mut editor);
        }
        idle_timer.reset();
    }

    // Drop compoisitor first to restore the terminal.
    drop(compositor);
    notification::set_stdout_mode(true);
}

fn check_if_dirty(
    compositor: &mut Compositor<Editor>,
    editor: &mut Editor,
    force_quit_tx: UnboundedSender<()>,
) {
    let mut dirty_doc = None;
    let mut num_dirty_docs = 0;
    for doc in editor.documents.documents().values() {
        if doc.is_dirty() && !doc.is_virtual_file() {
            dirty_doc = Some(doc);
            num_dirty_docs += 1;
        }
    }

    if num_dirty_docs == 0 {
        force_quit_tx.send(());
        return;
    }

    let title = if num_dirty_docs == 1 {
        format!("save {}? [yn]", dirty_doc.unwrap().name())
    } else {
        format!("save {} dirty buffers? [yn]", num_dirty_docs)
    };

    if compositor.contains_surface_with_name(&title) {
        // Ctrl-Q is pressed twice. Save all dirty documents and quit.
        editor.documents.save_all_on_drop(true);
        return;
    }

    prompt(
        compositor,
        editor,
        PromptMode::SingleChar,
        title,
        move |compositor, editor, answer| {
            match answer {
                Some(answer) if answer == "y" => {
                    info!("saving dirty buffers...");
                    editor.documents.save_all_on_drop(true);
                    force_quit_tx.send(());
                }
                Some(answer) if answer == "n" => {
                    // Quit without saving dirty files.
                    editor.documents.save_all_on_drop(false);
                    force_quit_tx.send(());
                }
                None => {
                    // Abort.
                }
                _ => {
                    let prompt_view: &mut PromptView = compositor.get_mut_surface_by_name("prompt");
                    prompt_view.clear();

                    notify_error!("invalid answer");
                    return ControlFlow::Continue(());
                }
            }

            ControlFlow::Break(())
        },
        |_, _| None,
    );
}
