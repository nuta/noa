use std::process::{Command, Output, Stdio};

use anyhow::Result;
use futures::executor::block_on;
use noa_compositor::compositor::Compositor;

use crate::editor::Editor;

pub fn run_external_command(
    editor: &mut Editor,
    compositor: &mut Compositor<Editor>,
    mut cmd: Command,
) -> Result<Output> {
    info!("running {:?}", cmd);

    let result = compositor.run_in_cooked_mode(|| {
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .stdin(Stdio::inherit())
            .spawn()?
            .wait_with_output()
    });

    compositor.force_render(editor);

    Ok(result?)
}
