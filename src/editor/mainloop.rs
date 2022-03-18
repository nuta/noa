use std::{ops::ControlFlow, path::PathBuf, sync::Arc, time::Duration};

use noa_common::time_report::TimeReport;
use noa_compositor::{terminal::Event, Compositor};
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    Notify,
};

use crate::{
    editor::Editor,
    job::JobManager,
    notification::set_stdout_mode,
    ui::{
        buffer_view::BufferView,
        completion_view::CompletionView,
        finder_view::FinderView,
        meta_line_view::MetaLineView,
        prompt::prompt,
        prompt_view::{PromptMode, PromptView},
        too_small_view::TooSmallView,
    },
};

pub async fn mainloop(
    mut editor: Editor,
    mut compositor: Compositor<Editor>,
    workspace_dir: PathBuf,
    open_finder: bool,
    render_request: Arc<Notify>,
    mut notification_rx: UnboundedReceiver<noa_proxy::protocol::Notification>,
) {
    let (quit_tx, mut quit_rx) = unbounded_channel();
    let (force_quit_tx, mut force_quit_rx) = unbounded_channel();
    compositor.add_frontmost_layer(Box::new(TooSmallView::new("too small!")));
    compositor.add_frontmost_layer(Box::new(BufferView::new(quit_tx, render_request.clone())));
    compositor.add_frontmost_layer(Box::new(MetaLineView::new()));
    compositor.add_frontmost_layer(Box::new(FinderView::new(
        &editor,
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

    let mut job_manager = JobManager::new();
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

            Some(callback) = job_manager.get_completed_job() => {
                callback(&mut editor, &mut compositor);
            }

            _ = render_request.notified() => {
            }

            _ = idle_timer.tick()  => {
                editor.documents.current_mut().idle_job();
                skip_rendering = true;
            }
        }

        editor.run_pending_callbacks(&mut compositor);

        if !skip_rendering {
            compositor.render_to_terminal(&mut editor);
        }
        idle_timer.reset();
    }

    // Drop compoisitor first to restore the terminal.
    drop(compositor);
    set_stdout_mode(true);
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
