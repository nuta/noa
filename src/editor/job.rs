use std::{
    collections::HashMap,
    marker::PhantomData,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use anyhow::Result;
use futures::{
    future::BoxFuture,
    stream::{BoxStream, FuturesUnordered, SelectAll},
    Future, FutureExt, Stream, StreamExt,
};
use noa_compositor::Compositor;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    watch, Notify,
};

use crate::editor::Editor;

type CompletedCallback = dyn FnOnce(&mut Editor, &mut Compositor<Editor>) + Send + 'static;
type NotifyCallback = dyn FnMut(&mut Editor, &mut Compositor<Editor>) + Send + 'static;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub struct NotifyCallbackId(NonZeroUsize);

impl NotifyCallbackId {
    fn alloc() -> NotifyCallbackId {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        NotifyCallbackId(unsafe {
            NonZeroUsize::new_unchecked(NEXT_ID.fetch_add(1, Ordering::SeqCst))
        })
    }
}

pub enum CompletedJob {
    Completed(Box<CompletedCallback>),
    /// Note: Once you used the callback, you must reeturn it into the job manager
    ///       again through `JobManager::insert_back_notified`.
    Notified {
        id: NotifyCallbackId,
        callback: Box<NotifyCallback>,
    },
}

pub struct JobManager {
    futures: FuturesUnordered<BoxFuture<'static, Result<Box<CompletedCallback>>>>,
    notified_tx: UnboundedSender<NotifyCallbackId>,
    notified_rx: UnboundedReceiver<NotifyCallbackId>,
    notify_callbacks: HashMap<NotifyCallbackId, Box<NotifyCallback>>,
}

impl JobManager {
    pub fn new() -> JobManager {
        let (notified_tx, notified_rx) = unbounded_channel();
        JobManager {
            futures: FuturesUnordered::new(),
            notified_tx,
            notified_rx,
            notify_callbacks: HashMap::new(),
        }
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
                let callback = self.notify_callbacks.remove(&id).unwrap();
                Some(CompletedJob::Notified{ id, callback })
            }

            else => {
                None
            }
        }
    }

    /// XXX: This is a hack to get around the ownership rule by taking the callback
    ///      out of `Editor` temporarily.
    pub fn insert_back_notified(&mut self, id: NotifyCallbackId, callback: Box<NotifyCallback>) {
        self.notify_callbacks.insert(id, callback);
    }

    pub fn push_notify<Callback>(&mut self, mut notify: Arc<Notify>, callback: Callback)
    where
        Callback: FnMut(&mut Editor, &mut Compositor<Editor>) + Send + 'static,
    {
        let id = NotifyCallbackId::alloc();
        self.notify_callbacks.insert(id, Box::new(callback));
        let notified_tx = self.notified_tx.clone();

        tokio::spawn(async move {
            loop {
                notify.notified().await;
                notified_tx.send(id);
            }
        });
    }

    pub fn push_future<Fut, Ret, Callback>(&mut self, future: Fut, callback: Callback)
    where
        Fut: Future<Output = Result<Ret>> + Send + 'static,
        Ret: Send + 'static,
        Callback: FnOnce(&mut Editor, &mut Compositor<Editor>, Ret) + Send + 'static,
    {
        self.futures.push(
            async move {
                let result = future.await?;

                // Curring the callback.
                let boxed_callback: Box<
                    dyn FnOnce(&mut Editor, &mut Compositor<Editor>) + Send + 'static,
                > = Box::new(move |editor, compositor| callback(editor, compositor, result));

                Ok(boxed_callback)
            }
            .boxed(),
        );
    }
}
