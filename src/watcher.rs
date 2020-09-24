use std::sync::mpsc::{channel, Sender};
use std::path::Path;
use std::time::Duration;
use notify::{RecommendedWatcher, Watcher, DebouncedEvent, RecursiveMode};
use crate::editor::Event;

pub struct FileWatcher {
    watcher: RecommendedWatcher,
}

impl FileWatcher {
    pub fn new(event_queue: Sender<Event>) -> FileWatcher {
        let (tx, rx) = channel();
        let watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(2))
            .expect("failed to initialize the fs change watcher");

        std::thread::spawn(move || {
            loop {
                match rx.recv() {
                    Ok(DebouncedEvent::Write(path)) => {
                        trace!("file changed: {}", path.display());
                        event_queue.send(Event::FileChanged(path)).ok();
                    }
                    _ => {}
                }
            }
        });

        FileWatcher {
            watcher,
        }
    }

    pub fn start_watching(&mut self, path: &Path) {
        self.watcher.watch(path, RecursiveMode::NonRecursive).ok();
    }
}
