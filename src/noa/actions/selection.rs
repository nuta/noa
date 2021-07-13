use super::{Action, Context};

pub struct SelectAll;

impl Action for SelectAll {
    fn id(&self) -> &'static str {
        "selection.select_all"
    }

    fn title(&self) -> &'static str {
        "Select All"
    }

    fn execute<'a>(&self, ctx: &Context<'a>) {
        ctx.buffers
            .write()
            .current_file()
            .write()
            .buffer
            .select_all();
    }
}
