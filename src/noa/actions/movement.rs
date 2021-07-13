use super::{Action, Context};

pub struct MoveToBeginningOfBuffer;

impl Action for MoveToBeginningOfBuffer {
    fn id(&self) -> &'static str {
        "movement.move_to_beginning_of_buffer"
    }

    fn title(&self) -> &'static str {
        "Move to Beginning of Buffer"
    }

    fn execute<'a>(&self, ctx: &Context<'a>) {
        ctx.buffers
            .write()
            .current_file()
            .write()
            .buffer
            .move_to_beginning_of_buffer();
    }
}

pub struct MoveToEndOfBuffer;

impl Action for MoveToEndOfBuffer {
    fn id(&self) -> &'static str {
        "movement.move_to_end_of_buffer"
    }

    fn title(&self) -> &'static str {
        "Move to End of Buffer"
    }

    fn execute<'a>(&self, ctx: &Context<'a>) {
        ctx.buffers
            .write()
            .current_file()
            .write()
            .buffer
            .move_to_end_of_buffer();
    }
}
