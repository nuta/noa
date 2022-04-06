use anyhow::Result;
use futures::{future::BoxFuture, stream::FuturesUnordered, Future, FutureExt, StreamExt};
use noa_compositor::Compositor;

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

    pub fn is_busy(&self) -> bool {
        !self.futures.is_empty()
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
        Fut: Future<Output = Ret> + Send + 'static,
        Ret: Send + 'static,
        Callback: FnOnce(&mut Editor, &mut Compositor<Editor>, Ret) + Send + 'static,
    {
        self.futures.push(
            async move {
                let result = tokio::spawn(future).await.unwrap();

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
