use super::Action;

pub struct ToUppercase;

impl Action for ToUppercase {
    fn id(&self) -> &'static str {
        "transform.to_upper"
    }

    fn title(&self) -> &'static str {
        "Transform to Uppercase"
    }

    fn execute(&self, ctx: &mut crate::ui::Context, _compositor: &mut crate::ui::Compositor) {
        let mut f = ctx.editor.current_file().write();
        f.buffer
            .transform_selections_with(|_, text| text.to_ascii_uppercase());
    }
}
