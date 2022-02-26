use noa_common::fuzzyvec::FuzzyVec;
use noa_compositor::{line_edit::LineEdit, Compositor};

use crate::{
    editor::Editor,
    ui::prompt_view::{PromptMode, PromptView},
};

pub fn prompt<S, F, C>(
    compositor: &mut Compositor<Editor>,
    editor: &mut Editor,
    mode: PromptMode,
    title: S,
    enter_callback: F,
    mut completion_callback: Option<C>,
) where
    S: Into<String>,
    F: FnOnce(&mut Compositor<Editor>, &mut Editor, Option<String>) + 'static,
    C: FnMut(&mut Editor, &LineEdit) -> FuzzyVec<String> + 'static,
{
    let title = title.into();
    let enter_cb = {
        let title = title.clone();
        editor.register_once_callback(move |compositor, editor| {
            let prompt_view: &mut PromptView = compositor.get_mut_surface_by_name(&title);

            let result = if prompt_view.canceled {
                Some(prompt_view.input.text())
            } else {
                None
            };

            enter_callback(compositor, editor, result);
        })
    };

    let completion_cb = {
        let title = title.clone();
        editor.register_callback(move |compositor, editor| {
            let prompt_view: &mut PromptView = compositor.get_mut_surface_by_name(&title);

            let result = if prompt_view.canceled {
                Some(prompt_view.input.text())
            } else {
                None
            };

            if let Some(mut callback) = completion_callback.as_mut() {
                let prompt_view: &mut PromptView = compositor.get_mut_surface_by_name(&title);
                callback(editor, &prompt_view.input);
            }
        })
    };

    let prompt_view = PromptView::new(mode, title, enter_cb);
    compositor.add_frontmost_layer(Box::new(prompt_view));
}
