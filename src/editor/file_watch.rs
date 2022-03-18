use std::time::Duration;

use anyhow::{Context, Result};
use notify::{DebouncedEvent, Watcher};
use tokio::sync::mpsc;

use crate::{
    document::{Document, DocumentId},
    editor::Editor,
};

pub enum WatchEventKind {
    Modified,
}

pub struct WatchEvent {
    kind: WatchEventKind,
    doc_id: DocumentId,
}

pub fn after_open_hook(watch_tx: mpsc::UnboundedSender<WatchEvent>, doc: &Document) -> Result<()> {
    let doc_id = doc.id();
    let path = doc.path().to_path_buf();

    let (raw_tx, raw_rx) = std::sync::mpsc::channel();
    let mut watcher = notify::watcher(raw_tx, Duration::from_secs(1))
        .context("failed to create a file watcher")?;
    watcher
        .watch(&path, notify::RecursiveMode::NonRecursive)
        .with_context(|| format!("failed to watch {}", path.display()))?;

    std::thread::spawn(move || {
        while let Ok(ev) = raw_rx.recv() {
            trace!("received a file event: {:?}", ev);
            match ev {
                DebouncedEvent::Create(p) | DebouncedEvent::Write(p) if p == path => {
                    let _ = watch_tx.send(WatchEvent {
                        kind: WatchEventKind::Modified,
                        doc_id,
                    });
                }
                _ => {}
            }
        }

        // Drop watcher here otherwise it'll be dropped when returning from
        // `after_open_hook`.
        drop(watcher);
    });

    Ok(())
}

/// Reloads a buffer from the disk if changed.
pub fn watch_event_hook(editor: &mut Editor, ev: &WatchEvent) {
    if !matches!(ev.kind, WatchEventKind::Modified) {
        return;
    }

    let current_id = editor.documents.current().id();
    let doc = match editor.documents.get_mut_document_by_id(ev.doc_id) {
        Some(doc) => doc,
        None => {
            warn!("document {:?} was closed", ev.doc_id);
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
}
