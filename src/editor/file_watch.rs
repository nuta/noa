use std::{path::Path, time::Duration};

use anyhow::{Context, Result};
use notify::{DebouncedEvent, Watcher};

use crate::{
    document::Document,
    event_listener::{self, EventListener},
    job::JobManager,
};

pub struct FileWatcher {
    _watcher: notify::RecommendedWatcher,
}

pub fn watch_file(path: &Path) -> Result<(FileWatcher, EventListener)> {
    let (modified_tx, modified_rx) = std::sync::mpsc::channel();
    let mut watcher = notify::watcher(modified_tx, Duration::from_secs(1))
        .context("failed to create a file watcher")?;
    watcher
        .watch(path, notify::RecursiveMode::NonRecursive)
        .with_context(|| format!("failed to watch {}", path.display()))?;

    let modified_event = event_listener::event_pair();
    let producer = modified_event.producer;
    let path = path.to_path_buf();
    std::thread::spawn(move || {
        while let Ok(ev) = modified_rx.recv() {
            trace!("received a file event: {:?}", ev);
            match ev {
                DebouncedEvent::Create(p) | DebouncedEvent::Write(p) if p == path => {
                    producer.notify_all();
                }
                _ => {}
            }
        }
    });

    Ok((FileWatcher { _watcher: watcher }, modified_event.listener))
}

pub fn after_open_hook(jobs: &mut JobManager, doc: &Document) {
    // Watch changes on disk and reload it if changed.
    if let Some(listener) = doc.modified_listener().cloned() {
        let doc_id = doc.id();
        jobs.listen_in_mainloop(listener, move |editor, _compositor| {
            let current_id = editor.documents.current().id();
            let doc = match editor.documents.get_mut_document_by_id(doc_id) {
                Some(doc) => doc,
                None => {
                    warn!("document {:?} was closed", doc_id);
                    return;
                }
            };

            match doc.reload() {
                Ok(_) => {
                    if current_id == doc.id() {
                        notify_info!("reloaded from the disk");
                    }
                }
                Err(err) => {
                    warn!("failed to reload {}: {:?}", doc.path().display(), err);
                }
            }
        });
    }
}
