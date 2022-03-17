use std::collections::HashMap;

use anyhow::Result;

use noa_compositor::Compositor;

use crate::editor::Editor;

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Hook {
    AfterOpen,
    BeforeSave,
}

struct Entry {
    name: &'static str,
    callback: Box<dyn FnMut(&mut Editor, &mut Compositor<Editor>) -> Result<()> + Send>,
}

pub struct HookManager {
    callbacks: HashMap<Hook, Vec<Entry>>,
}

impl HookManager {
    pub fn new() -> Self {
        Self {
            callbacks: HashMap::new(),
        }
    }

    pub fn register<F>(&mut self, hook: Hook, name: &'static str, callback: F)
    where
        F: FnMut(&mut Editor, &mut Compositor<Editor>) -> Result<()> + Send + 'static,
    {
        self.callbacks
            .entry(hook)
            .or_insert_with(Vec::new)
            .push(Entry {
                name,
                callback: Box::new(callback),
            });
    }

    pub fn invoke(&mut self, editor: &mut Editor, compositor: &mut Compositor<Editor>, hook: Hook) {
        if let Some(entries) = self.callbacks.get_mut(&hook) {
            for entry in entries {
                if let Err(err) = (entry.callback)(editor, compositor) {
                    warn!("hook {} ({:?}) failed: {}", entry.name, hook, err);
                }
            }
        }
    }
}

pub type HookCallback<T> = dyn FnMut(&mut Editor, &mut Compositor<Editor>, &T) -> Result<()>;

pub struct Hook2<T> {
    callbacks: Vec<Box<HookCallback<T>>>,
    invoke_queue: Vec<T>,
}

impl<T> Hook2<T> {
    pub fn new() -> Self {
        Self {
            callbacks: Vec::new(),
            invoke_queue: Vec::new(),
        }
    }

    pub fn register<F>(&mut self, _hook: Hook, _name: &'static str, callback: F)
    where
        F: FnMut(&mut Editor, &mut Compositor<Editor>, &T) -> Result<()> + 'static,
    {
        self.callbacks.push(Box::new(callback));
    }

    pub fn register_from_vec<F>(&mut self, callbacks: Vec<Box<HookCallback<T>>>) {
        self.callbacks.extend(callbacks);
    }

    pub fn invoke(&mut self, value: T) {
        self.invoke_queue.push(value);
    }

    pub fn queued_invocations(&mut self) -> (Vec<Box<HookCallback<T>>>, Vec<T>) {
        let callbacks = std::mem::take(&mut self.callbacks);
        let invokes = std::mem::take(&mut self.invoke_queue);
        (callbacks, invokes)
    }
}
