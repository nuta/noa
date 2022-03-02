use std::{any::Any, collections::HashMap};

use anyhow::{anyhow, Result};
use crossbeam::epoch::Pointable;
use noa_compositor::Compositor;
use once_cell::sync::Lazy;

use crate::editor::Editor;

mod change_case;
mod truncate;

pub const ACTIONS: &[&dyn Action] = &[&change_case::ToUpperCase, &change_case::ToLowerCase];

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
