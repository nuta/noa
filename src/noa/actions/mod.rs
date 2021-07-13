use std::{collections::HashMap, sync::Arc};

use once_cell::sync::Lazy;
use parking_lot::RwLock;

use crate::buffer_set::BufferSet;

mod movement;
mod selection;
mod transform;

pub struct Context<'a> {
    pub buffers: &'a Arc<RwLock<BufferSet>>,
}

pub trait Action: Send + Sync {
    fn id(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn execute<'a>(&self, ctx: &Context<'a>);
}

impl std::fmt::Debug for dyn Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[action: {}]", self.id())
    }
}

pub static ACTIONS: Lazy<HashMap<&str, Arc<dyn Action>>> = Lazy::new(|| {
    let actions = [
        Arc::new(transform::ToLowercase) as Arc<dyn Action>,
        Arc::new(transform::ToUppercase) as Arc<dyn Action>,
        Arc::new(selection::SelectAll) as Arc<dyn Action>,
        Arc::new(movement::MoveToBeginningOfBuffer) as Arc<dyn Action>,
        Arc::new(movement::MoveToEndOfBuffer) as Arc<dyn Action>,
    ];

    let mut actions_map = HashMap::with_capacity(actions.len());
    for action in actions {
        actions_map.insert(action.id(), action);
    }

    actions_map
});
