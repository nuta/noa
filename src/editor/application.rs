use std::{
    ops::ControlFlow,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use noa_common::time_report::TimeReport;
use noa_proxy::protocol::Notification;
use noa_terminal::terminal::Event;
use tokio::sync::{
    mpsc::{self, unbounded_channel, UnboundedReceiver, UnboundedSender},
    Notify,
};

use crate::{
    editor::Editor,
    finder::open_finder,
    hook::HookManager,
    job::CompletedJob,
    ui::{
        compositor::Compositor,
        surface::UIContext,
        views::{
            buffer_view::BufferView,
            bump_view::BumpView,
            completion_view::CompletionView,
            meta_line_view::MetaLineView,
            prompt_view::{prompt, PromptMode, PromptView},
            selector_view::SelectorView,
            too_small_view::TooSmallView,
        },
    },
};

pub struct Application {
    // `compositor` should come first to restore the terminal before dropping
    // DocumentManager since it use eprintln!.
    compositor: Compositor,
    editor: Editor,
    hooks: HookManager,
    force_quit_rx: UnboundedReceiver<()>,
    force_quit_tx: UnboundedSender<()>,
    quit_rx: UnboundedReceiver<()>,
    notification_rx: UnboundedReceiver<Notification>,
    render_request: Arc<Notify>,
}

impl Application {
    pub fn new(workspace_dir: &Path, files: &[PathBuf]) -> Application {
        let render_request = Arc::new(Notify::new());
        let (notification_tx, notification_rx) = mpsc::unbounded_channel();

        let mut editor = Editor::new(workspace_dir, render_request.clone(), notification_tx);

        let mut compositor = Compositor::new();
        let (quit_tx, quit_rx) = unbounded_channel();
        let (force_quit_tx, force_quit_rx) = unbounded_channel();
        compositor.add_frontmost_layer(Box::new(TooSmallView::new("too small!")));
        compositor.add_frontmost_layer(Box::new(BufferView::new(quit_tx, render_request.clone())));
        compositor.add_frontmost_layer(Box::new(BumpView::new()));
        compositor.add_frontmost_layer(Box::new(MetaLineView::new()));
        compositor.add_frontmost_layer(Box::new(SelectorView::new()));
        compositor.add_frontmost_layer(Box::new(PromptView::new()));
        compositor.add_frontmost_layer(Box::new(CompletionView::new()));

        let mut no_files_opened = true;
        for path in files {
            if !path.is_dir() {
                match editor.documents.open_file(path, None) {
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

        if no_files_opened {
            open_finder(&mut compositor, &mut editor);
        }

        Application {
            compositor,
            editor,
            hooks: HookManager::new(),
            force_quit_rx,
            force_quit_tx,
            quit_rx,
            notification_rx,
            render_request,
        }
    }

    pub async fn run(&mut self) {
        let mut ctx = UIContext {
            editor: &mut self.editor,
            // hooks: &mut self.hooks,
        };
        self.compositor.render_to_terminal(&mut ctx);

        loop {
            if self.process_event().await == ControlFlow::Break(()) {
                break;
            }
        }
    }

    async fn process_event(&mut self) -> ControlFlow<()> {
        let mut idle_timer = tokio::time::interval(Duration::from_millis(1200));
        let mut skip_rendering = false;
        tokio::select! {
            biased;

            _ = self.force_quit_rx.recv() => {
                return ControlFlow::Break(());
           }

            Some(()) =  self.quit_rx.recv() => {
                self.check_if_dirty();
            }

            Some(ev) = self.compositor.recv_terminal_event() => {
                let _event_tick_time = Some(TimeReport::new("I/O event handling"));
                match ev {
                    Event::Input(input) => {
                        let mut ctx =  UIContext {
                            editor: &mut self.editor,
                            // hooks: &mut self.hooks,
                        };
                        self.compositor.handle_input(&mut ctx, input);
                    }
                    Event::Resize { height, width } => {
                        self.compositor.resize_screen(height, width);
                    }
                }
            }

            Some(noti) = self.notification_rx.recv() => {
                trace!("proxy notification: {:?}", noti);
                match noti {
                    noa_proxy::protocol::Notification::Diagnostics { diags, path } => {
                        if path == self.editor.documents.current().path() {
                            if let Some(diag) = diags.first() {
                                notify_warn!("{}: {:?}", diag.range.start.line + 1, diag.message);
                            }
                        }
                    }
                }
            }

            Some(completed) = self.editor.jobs.get_completed() => {
                match completed {
                    CompletedJob::Completed(callback) => {
                        callback(&mut self.editor, &mut self.compositor);
                    }
                    CompletedJob::Notified { id, mut callback } => {
                        callback(&mut self.editor, &mut self.compositor);
                        self.editor.jobs.insert_back_notified(id, callback);
                    }
                }
            }

            _ = self.render_request.notified() => {
            }

            _ = idle_timer.tick()  => {
                self.editor.documents.current_mut().idle_job();
                skip_rendering = true;
            }
        }

        if !skip_rendering {
            let mut ctx = UIContext {
                editor: &mut self.editor,
                // hooks: &mut self.hooks,
            };
            self.compositor.render_to_terminal(&mut ctx);
        }
        idle_timer.reset();

        ControlFlow::Continue(())
    }

    fn check_if_dirty(&mut self) {
        let mut dirty_doc = None;
        let mut num_dirty_docs = 0;
        for doc in self.editor.documents.documents().values() {
            if doc.is_dirty() && !doc.is_virtual_file() {
                dirty_doc = Some(doc);
                num_dirty_docs += 1;
            }
        }

        if num_dirty_docs == 0 {
            let _ = self.force_quit_tx.send(());
            return;
        }

        let title = if num_dirty_docs == 1 {
            format!("save {}? [yn]", dirty_doc.unwrap().name())
        } else {
            format!("save {} dirty buffers? [yn]", num_dirty_docs)
        };

        if self.compositor.contains_surface_with_name(&title) {
            // Ctrl-Q is pressed twice. Save all dirty documents and quit.
            self.editor.documents.save_all_on_drop(true);
            return;
        }

        let force_quit_tx = self.force_quit_tx.clone();
        prompt(
            &mut self.compositor,
            &mut self.editor,
            PromptMode::SingleChar,
            title,
            move |compositor, editor, answer| {
                match answer {
                    Some(answer) if answer == "y" => {
                        info!("saving dirty buffers...");
                        editor.documents.save_all_on_drop(true);
                        let _ = force_quit_tx.send(());
                    }
                    Some(answer) if answer == "n" => {
                        // Quit without saving dirty files.
                        info!("quitting without saving dirty buffers...");
                        editor.documents.save_all_on_drop(false);
                        let _ = force_quit_tx.send(());
                    }
                    None => {
                        // Abort.
                    }
                    _ => {
                        let prompt_view: &mut PromptView =
                            compositor.get_mut_surface_by_name("prompt");
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
}
