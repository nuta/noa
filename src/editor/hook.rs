use std::{any::Any, collections::HashMap};

use anyhow::{anyhow, Result};

use noa_compositor::Compositor;
use once_cell::sync::Lazy;

use crate::editor::Editor;

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Hook {
    AfterOpen,
    BeforeSave,
}

pub trait HookCallback: Any + Send + Sync {
    fn run(&self, editor: &mut Editor, compositor: &mut Compositor<Editor>) -> Result<()>;
}

struct Entry {
    name: &'static str,
    callback: Box<dyn HookCallback>,
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

    pub fn register(&mut self, hook: Hook, name: &'static str, callback: impl HookCallback) {
        self.callbacks
            .entry(hook)
            .or_insert_with(Vec::new)
            .push(Entry {
                name,
                callback: Box::new(callback),
            });
    }

    pub fn invoke(&mut self, editor: &mut Editor, compositor: &mut Compositor<Editor>, hook: Hook) {
        if let Some(entries) = self.callbacks.get(&hook) {
            for entry in entries {
                if let Err(err) = entry.callback.run(editor, compositor) {
                    warn!("hook {} ({:?}) failed: {}", entry.name, hook, err);
                }
            }
        }
    }
}
