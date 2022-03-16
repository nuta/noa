use std::{collections::HashMap};

use anyhow::{Result};

use noa_compositor::Compositor;


use crate::editor::Editor;

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Hook {
    AfterOpen,
    BeforeSave,
}

struct Entry {
    name: &'static str,
    callback: Box<dyn FnMut(&mut Editor, &mut Compositor<Editor>) -> Result<()>>,
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
        F: FnMut(&mut Editor, &mut Compositor<Editor>) -> Result<()> + 'static,
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
