use std::io::prelude::*;
use std::process::{Command, Stdio};

/// The clipboard (tmux) manager.
pub struct Clipboard {
}

impl Clipboard {
    pub fn new() -> Clipboard {
        Clipboard {
        }
    }

    pub fn get(&mut self) -> String {
        Command::new("tmux")
            .args(&["show-buffer"])
            .output()
            .map(|output| {
                String::from_utf8(output.stdout)
                    .unwrap_or_else(|_| String::new())
            })
            .unwrap_or_else(|_| String::new())
    }

    pub fn set(&mut self, text: &str) {
        let cmd = Command::new("tmux")
            .args(&["load-buffer", "-"])
            .stdin(Stdio::piped())
            .spawn();

        match cmd {
            Ok(mut cmd) => {
                write!(cmd.stdin.take().unwrap(), "{}", text).ok();
                cmd.wait().ok();
            }
            Err(err) => {
                error!("failed to invoke tmux: {:?}", err);
            }
        }
    }
}
