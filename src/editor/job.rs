use std::marker::PhantomData;

use anyhow::Result;
use futures::{future::BoxFuture, stream::FuturesUnordered, Future, FutureExt, StreamExt};
use noa_compositor::Compositor;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::editor::Editor;

type CompletedCallback = dyn FnOnce(&mut Editor, &mut Compositor<Editor>) + Send + 'static;

pub struct JobManager {
    futures: FuturesUnordered<BoxFuture<'static, Result<Box<CompletedCallback>>>>,
}

impl JobManager {
    pub fn new() -> JobManager {
        JobManager {
            futures: FuturesUnordered::new(),
        }
    }

    pub async fn get_completed_job(
        &mut self,
    ) -> Option<Box<dyn FnOnce(&mut Editor, &mut Compositor<Editor>) + Send + 'static>> {
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

    pub fn push<Fut, Ret, Callback>(&mut self, future: Fut, callback: Callback)
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
