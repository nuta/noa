use std::{
    collections::HashMap,
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::ui::compositor::Compositor;
use anyhow::Result;
use futures::{future::BoxFuture, stream::FuturesUnordered, Future, FutureExt, StreamExt};

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{editor::Editor, event_listener::EventListener};

type CompletedCallback = dyn FnOnce(&mut Editor, &mut Compositor) + Send + 'static;
type NotifyCallback = dyn FnMut(&mut Editor, &mut Compositor) + Send + 'static;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub struct CallbackId(NonZeroUsize);

impl CallbackId {
    fn alloc() -> CallbackId {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        CallbackId(unsafe { NonZeroUsize::new_unchecked(NEXT_ID.fetch_add(1, Ordering::SeqCst)) })
    }
}

pub enum CompletedJob {
    Completed(Box<CompletedCallback>),
    /// Note: Once you used the callback, you must reeturn it into the job manager
    ///       again through `JobManager::insert_back_notified`.
    Notified {
        id: CallbackId,
        callback: Box<NotifyCallback>,
    },
}

pub struct JobManager {
    futures: FuturesUnordered<BoxFuture<'static, Result<Box<CompletedCallback>>>>,
    notified_tx: UnboundedSender<CallbackId>,
    notified_rx: UnboundedReceiver<CallbackId>,
    mut_callbacks: HashMap<CallbackId, Box<NotifyCallback>>,
}

impl JobManager {
    pub fn new() -> JobManager {
        let (notified_tx, notified_rx) = unbounded_channel();
        JobManager {
            futures: FuturesUnordered::new(),
            notified_tx,
            notified_rx,
            mut_callbacks: HashMap::new(),
        }
    }

    pub fn listen_in_mainloop<Callback>(&mut self, mut listener: EventListener, callback: Callback)
    where
        Callback: FnMut(&mut Editor, &mut Compositor) + Send + 'static,
    {
        let id = CallbackId::alloc();
        self.mut_callbacks.insert(id, Box::new(callback));
        let notified_tx = self.notified_tx.clone();

        tokio::spawn(async move {
            while let Ok(()) = listener.notified().await {
                let _ = notified_tx.send(id);
            }
        });
    }

    pub fn await_in_mainloop<Fut, Ret, Then>(&mut self, future: Fut, then: Then)
    where
        Fut: Future<Output = Result<Ret>> + Send + 'static,
        Ret: Send + 'static,
        Then: FnOnce(&mut Editor, &mut Compositor, Ret) + Send + 'static,
    {
        self.futures.push(
            async move {
                let result = future.await?;

                // Curring the callback.
                let boxed_callback: Box<dyn FnOnce(&mut Editor, &mut Compositor) + Send + 'static> =
                    Box::new(move |editor, compositor| then(editor, compositor, result));

                Ok(boxed_callback)
            }
            .boxed(),
        );
    }

    pub async fn get_completed(&mut self) -> Option<CompletedJob> {
        tokio::select! {
            Some(callback) = self.futures.next() => {
                match callback {
                    Ok(callback) => Some(CompletedJob::Completed(callback)),
                    Err(err) => {
                        warn!("job returned error: {}", err);
                        None
                    }
                }
            }

            Some(id) = self.notified_rx.recv() => {
                // Remove the callback temporarily so that the caller don't
                // need self's mutable reference to use the callback.
                let callback = self.mut_callbacks.remove(&id).unwrap();
                Some(CompletedJob::Notified{ id, callback })
            }

            else => {
                None
            }
        }
    }

    /// XXX: This is a hack to get around the ownership rule by taking the callback
    ///      out of `Editor` temporarily.
    pub fn insert_back_notified(&mut self, id: CallbackId, callback: Box<NotifyCallback>) {
        self.mut_callbacks.insert(id, callback);
    }
}

impl Drop for JobManager {
    fn drop(&mut self) {}
}
