use std::{
    collections::HashMap,
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::Result;
use futures::{future::BoxFuture, stream::FuturesUnordered, Future, FutureExt, StreamExt};
use noa_compositor::Compositor;

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::editor::Editor;

type CompletedCallback = dyn FnOnce(&mut Editor, &mut Compositor<Editor>) + Send + 'static;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub struct CallbackId(NonZeroUsize);

impl CallbackId {
    fn alloc() -> CallbackId {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        CallbackId(unsafe { NonZeroUsize::new_unchecked(NEXT_ID.fetch_add(1, Ordering::SeqCst)) })
    }
}

pub struct JobManager {
    futures: FuturesUnordered<BoxFuture<'static, Result<Box<CompletedCallback>>>>,
}

impl JobManager {
    pub fn new() -> JobManager {
        JobManager {
            futures: FuturesUnordered::new(),
        }
    }

    pub async fn get_completed(&mut self) -> Option<Box<CompletedCallback>> {
        tokio::select! {
            Some(callback) = self.futures.next() => {
                match callback {
                    Ok(callback) => Some(callback),
                    Err(err) => {
                        warn!("job returned error: {}", err);
                        None
                    }
                }
            }

            else => {
                None
            }
        }
    }

    pub fn await_in_mainloop<Fut, Ret, Callback>(&mut self, future: Fut, callback: Callback)
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

impl Drop for JobManager {
    fn drop(&mut self) {}
}
