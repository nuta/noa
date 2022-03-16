use std::{path::Path, time::Duration};

use anyhow::{Context, Result};
use notify::{DebouncedEvent, Watcher};

use crate::event_listener::{self, EventListener};

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
