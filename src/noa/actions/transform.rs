use super::{Action, Context};

pub struct ToUppercase;

impl Action for ToUppercase {
    fn id(&self) -> &'static str {
        "transform.to_upper"
    }

    fn title(&self) -> &'static str {
        "Transform to Uppercase"
    }

    fn execute<'a>(&self, ctx: &Context<'a>) {
        ctx.buffers
            .write()
            .current_file()
            .write()
            .buffer
            .transform_selections_with(|_, text| text.to_ascii_uppercase());
    }
}

pub struct ToLowercase;

impl Action for ToLowercase {
    fn id(&self) -> &'static str {
        "transform.to_lower"
    }

    fn title(&self) -> &'static str {
        "Transform to Lowercase"
    }

    fn execute<'a>(&self, ctx: &Context<'a>) {
        ctx.buffers
            .write()
            .current_file()
            .write()
            .buffer
            .transform_selections_with(|_, text| text.to_ascii_lowercase());
    }
}
