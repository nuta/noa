use std::{collections::HashMap, sync::Arc};

use once_cell::sync::Lazy;

use crate::ui::{Compositor, Context};

mod transform;

pub trait Action: Send + Sync {
    fn id(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn execute(&self, ctx: &mut Context, compositor: &mut Compositor);
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[action: {}]", self.id())
    }
}

pub static ACTIONS: Lazy<HashMap<&str, Arc<dyn Action>>> = Lazy::new(|| {
    let actions = [Arc::new(transform::ToUppercase) as Arc<dyn Action>];

    let mut actions_map = HashMap::with_capacity(actions.len());
    for action in actions {
        actions_map.insert(action.id(), action);
    }

    actions_map
});
