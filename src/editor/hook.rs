use std::collections::HashMap;

use anyhow::Result;

use crate::ui::compositor::Compositor;

use crate::{document::DocumentId, editor::Editor};

pub struct HookContext<'a> {
    pub editor: &'a mut Editor,
    pub compositor: &'a mut Compositor,
}

pub type HookCallback<T> = dyn FnMut(&mut HookContext<'_>, &T);

pub struct Hook<T> {
    callbacks: Vec<Box<HookCallback<T>>>,
    invoke_queue: Vec<T>,
}

impl<T> Hook<T> {
    pub fn new() -> Self {
        Self {
            callbacks: Vec::new(),
            invoke_queue: Vec::new(),
        }
    }

    pub fn register<F>(&mut self, callback: F)
    where
        F: FnMut(&mut HookContext<'_>, &T) + 'static,
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

impl<T> Default for Hook<T> {
    fn default() -> Hook<T> {
        Hook::new()
    }
}

pub struct HookManager {
    pub after_save: Hook<DocumentId>,
}

impl HookManager {
    pub fn new() -> HookManager {
        HookManager {
            after_save: Hook::new(),
        }
    }
}

impl Default for HookManager {
    fn default() -> Self {
        HookManager::new()
    }
}
