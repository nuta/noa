#![feature(test)]
#![feature(vec_retain_mut)]

extern crate test;

#[macro_use]
extern crate log;

use std::{path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;

use config::parse_config_files;
use editor::Editor;
use finder::open_finder;
use noa_common::{logger::install_logger, time_report::TimeReport};
use noa_compositor::{terminal::Event, Compositor};
use search::warm_up_search_cache;
use tokio::{
    sync::{
        mpsc::{self, unbounded_channel, UnboundedSender},
        Notify,
    },
    time::timeout,
};
use ui::{
    buffer_view::BufferView, bump_view::BumpView, completion_view::CompletionView,
    meta_line_view::MetaLineView, prompt_view::PromptView, selector_view::SelectorView,
    too_small_view::TooSmallView,
};

#[macro_use]
mod notification;

mod actions;
mod clipboard;
mod completion;
mod config;
mod document;
mod editor;
mod file_watch;
mod finder;
mod flash;
mod git;
mod job;
mod linemap;
mod movement;
mod search;
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
    parse_config_files();

    install_logger("main");
    let args = Args::parse();

    let workspace_dir = args
        .files
        .iter()
        .find(|path| path.is_dir())
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."));

    warm_up_search_cache(&workspace_dir);

    let render_request = Arc::new(Notify::new());
    let (watch_tx, mut watch_rx) = mpsc::unbounded_channel();
    let (updated_syntax_tx, mut updated_syntax_rx) = mpsc::unbounded_channel();
    let mut editor = editor::Editor::new(
        &workspace_dir,
        render_request.clone(),
        watch_tx,
        updated_syntax_tx,
    );
    let mut compositor = Compositor::new();

    let mut no_files_opened = true;
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

            no_files_opened = false;
        }
    }

    let (quit_tx, mut quit_rx) = unbounded_channel();
    let (force_quit_tx, mut force_quit_rx) = unbounded_channel();
    compositor.add_frontmost_layer(Box::new(TooSmallView::new("too small!")));
    compositor.add_frontmost_layer(Box::new(BufferView::new(quit_tx, render_request.clone())));
    compositor.add_frontmost_layer(Box::new(BumpView::new()));
    compositor.add_frontmost_layer(Box::new(MetaLineView::new()));
    compositor.add_frontmost_layer(Box::new(SelectorView::new()));
    compositor.add_frontmost_layer(Box::new(PromptView::new()));
    compositor.add_frontmost_layer(Box::new(CompletionView::new()));

    if no_files_opened {
        open_finder(&mut editor, &mut compositor);
    }

    compositor.render_to_terminal(&mut editor);
    drop(boot_time);

    let mut idle_timer = tokio::time::interval(Duration::from_millis(1200));
    'outer: loop {
        idle_timer.reset();

        // Consume pending (i.e. ready) events.
        'inner: for i in 0.. {
            tokio::select! {
                biased;

                _ = force_quit_rx.recv() => {
                    break 'outer;
               }

                Some(()) =  quit_rx.recv() => {
                    check_if_dirty(&mut editor, &mut compositor, force_quit_tx.clone());
                }

                Some(ev) = compositor.recv_terminal_event() => {
                    let _event_tick_time = Some(TimeReport::new("compositor event handling"));
                    match ev {
                        Event::Input(input) => {
                            compositor.handle_input(&mut editor, input);
                        }
                        Event::Resize { height, width } => {
                            compositor.resize_screen(height, width);
                        }
                    }
                }

                Some(ev) = watch_rx.recv() => {
                    file_watch::watch_event_hook(&mut editor, &ev);
                }

                Some((doc_id, doc_ver, new_tree)) = updated_syntax_rx.recv() => {
                    if let Some(doc) = editor.documents.get_mut_document_by_id(doc_id) {
                        doc.set_syntax_tree(doc_ver, new_tree);
                    }
                }

                Some(callback) = editor.jobs.get_completed() => {
                    callback(&mut editor, &mut compositor);
                }

                _ = render_request.notified() => {
                }

                _ = idle_timer.tick() => {
                    editor.documents.current_mut().idle_job();
                }

                _ = futures::future::ready(()), if i > 0 => {
                    // Since we've already handled the first event, if there're no
                    // pending events, we should break the loop to update the
                    // terminal contents.
                    break 'inner;
                }
            }
        }

        // Give the tree-sitter a chance to finish parsing the latest buffer
        // to prevent flickering.
        //
        // Interestingly, handling a compositor event and modifying a document
        // is super fast (less than 100 us in total in my machine).
        while editor.documents.current_mut().is_parsing_in_progress() {
            match timeout(Duration::from_millis(5), updated_syntax_rx.recv()).await {
                Ok(Some((doc_id, doc_ver, new_tree))) => {
                    if let Some(doc) = editor.documents.get_mut_document_by_id(doc_id) {
                        doc.set_syntax_tree(doc_ver, new_tree);
                    }
                }
                _ => {
                    break;
                }
            }
        }

        compositor.render_to_terminal(&mut editor);
    }

    // Drop compoisitor first to restore the terminal.
    drop(compositor);

    notification::set_stdout_mode(true);
}

fn check_if_dirty(
    editor: &mut Editor,
    compositor: &mut Compositor<Editor>,
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
        let _ = force_quit_tx.send(());
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

    let prompt = compositor.get_mut_surface_by_name::<PromptView>("prompt");
    prompt.open(
        title,
        Box::new(move |editor, _, prompt, _| {
            let input = prompt.text();
            match input.as_str() {
                "y" => {
                    info!("saving dirty buffers...");
                    editor.documents.save_all_on_drop(true);
                    let _ = force_quit_tx.send(());
                    prompt.close();
                }
                "n" => {
                    // Quit without saving dirty files.
                    info!("quitting without saving dirty buffers...");
                    editor.documents.save_all_on_drop(false);
                    let _ = force_quit_tx.send(());
                    prompt.close();
                }
                _ => {
                    notify_error!("should be y or n");
                    prompt.clear();
                }
            }
        }),
    );
}
