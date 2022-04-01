use std::{any::Any, collections::HashMap};

use anyhow::{anyhow, Result};

use noa_compositor::Compositor;
use once_cell::sync::Lazy;

use crate::editor::Editor;

mod basic_editing;
mod change_case;
mod goto;
mod linemap;
mod scrolling;

pub const ACTIONS: &[&dyn Action] = &[
    &basic_editing::Save,
    &basic_editing::SaveAll,
    &basic_editing::OpenFilder,
    &basic_editing::BackspaceWord,
    &basic_editing::Truncate,
    &basic_editing::Delete,
    &basic_editing::MoveToBeginningOfLine,
    &basic_editing::MoveToEndOfLine,
    &basic_editing::MoveToNextWord,
    &basic_editing::MoveToPrevWord,
    &basic_editing::FindCurrentWord,
    &basic_editing::SelectAllCurrentWord,
    &basic_editing::SelectPrevWord,
    &basic_editing::SelectNextWord,
    &basic_editing::MoveLineUp,
    &basic_editing::MoveLinesDown,
    &basic_editing::AddCursorsUp,
    &basic_editing::AddCursorsDown,
    &basic_editing::DuplicateLinesUp,
    &basic_editing::DuplicateLinesDown,
    &basic_editing::SelectUntilBeginningOfLine,
    &basic_editing::SelectUntilEndOfLine,
    &basic_editing::Cut,
    &basic_editing::Copy,
    &basic_editing::Paste,
    &basic_editing::Undo,
    &basic_editing::UndoCursors,
    &basic_editing::Redo,
    &basic_editing::SoftWrap,
    &basic_editing::CommentOut,
    &basic_editing::ExpandSelection,
    &change_case::ToUpperCase,
    &change_case::ToLowerCase,
    &linemap::MoveToNextDiff,
    &linemap::MoveToPrevDiff,
    &scrolling::PageUp,
    &scrolling::PageDown,
    &scrolling::Centering,
    &goto::GoToLine,
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
