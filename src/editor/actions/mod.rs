use std::{any::Any, collections::HashMap};

use anyhow::{anyhow, Result};
use crossbeam::epoch::Pointable;
use noa_compositor::Compositor;
use once_cell::sync::Lazy;

use crate::editor::Editor;

mod basic_editing;
mod change_case;
mod linemap;

pub const ACTIONS: &[&dyn Action] = &[
    &change_case::ToUpperCase,
    &change_case::ToLowerCase,
    &basic_editing::Truncate,
    &basic_editing::BackspaceWord,
    &basic_editing::Truncate,
    &basic_editing::Delete,
    &basic_editing::MoveToBeginningOfLine,
    &basic_editing::MoveToEndOfLine,
    &basic_editing::MoveToNextWord,
    &basic_editing::MoveToPrevWord,
    &basic_editing::Cut,
    &basic_editing::Copy,
    &basic_editing::Paste,
    &basic_editing::Undo,
    &basic_editing::Redo,
    &basic_editing::Save,
    &basic_editing::OpenFilder,
];

pub trait Action: Any + Send + Sync {
    fn name(&self) -> &'static str;
    fn run(&self, editor: &mut Editor, compositor: &mut Compositor<Editor>) -> Result<()>;
}

static ACTION_MAP: Lazy<HashMap<&'static str, &'static dyn Action>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for action in ACTIONS {
        map.insert(action.name(), *action);
    }
    map
});

pub fn execute_action(
    editor: &mut Editor,
    compositor: &mut Compositor<Editor>,
    action: &str,
) -> Result<()> {
    match ACTION_MAP.get(action) {
        Some(action) => action.run(editor, compositor),
        None => Err(anyhow!("unknown action \"{}\"", action)),
    }
}

pub fn execute_action_or_notify(
    editor: &mut Editor,
    compositor: &mut Compositor<Editor>,
    action: &str,
) {
    if let Err(err) = execute_action(editor, compositor, action) {
        notify_error!("action: {}", err);
    }
}
