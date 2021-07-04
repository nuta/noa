use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{bail, Context, Result};
use noa_buffer::Point;
use once_cell::sync::Lazy;

pub fn in_tmux() -> bool {
    static IN_TMUX: Lazy<bool> = Lazy::new(|| std::env::var("TMUX").is_ok());
    *IN_TMUX
}

pub fn get_this_tmux_pane_id() -> Option<&'static str> {
    static TMUX_PANE: Lazy<Option<String>> = Lazy::new(|| std::env::var("TMUX_PANE").ok());
    (*TMUX_PANE).as_deref()
}

pub fn resolve_path_on_cursor(
    pane: &str,
    mouse_y: usize,
    mouse_x: usize,
) -> Result<(PathBuf, Point)> {
    let output = Command::new("tmux")
        .args(&[
            "capture-pane",
            "-p", /* stdout */
            "-S", /* from the very beginning*/
            "-",
            "-E", /* until the end */
            "-",
            "-t",
            pane,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to execve tmux")
        .wait_with_output()
        .expect("failed to dump the pane contents from tmux");

    extract_path_and_point(&output.stdout, mouse_y, mouse_x).context("failed to extract a path")
}

pub fn select_pane(pane_id: &str) -> Result<()> {
    Command::new("tmux")
        .args(&["select-pane", "-t", pane_id])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

pub fn get_other_noa_pane_id() -> Result<String> {
    let output = Command::new("tmux")
        .args(&["list-panes", "-F", "#{pane_id} #{pane_current_command}"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?
        .wait_with_output()
        .expect("failed to dump the pane contents from tmux");

    let stdout = std::str::from_utf8(&output.stdout)?;
    let current_pane_id = match get_this_tmux_pane_id() {
        Some(pane) => pane,
        None => bail!("not in tmux"),
    };

    for pane in stdout.split('\n') {
        let mut word = pane.split(' ');
        let pane_id = word.next().context("invalid list-panes output")?;
        let program = word.next().context("invalid list-panes output")?;

        if program == "noa" && pane_id != current_pane_id {
            return Ok(pane_id.to_owned());
        }
    }

    bail!("no noa panes");
}

fn is_valid_path_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || byte == b'/'
        || byte == b':'
        || byte == b'.'
        || byte == b'-'
        || byte == b'_'
}

fn extract_path_and_point(
    heystack: &[u8],
    mouse_y: usize,
    mouse_x: usize,
) -> Option<(PathBuf, Point)> {
    let mut skip = mouse_y;
    let mut cursor_i = None;
    for (i, &byte) in heystack.iter().enumerate() {
        if skip == 0 {
            cursor_i = Some(i);
            break;
        }

        if byte == b'\n' {
            skip -= 1;
        }
    }

    let cursor_i = cursor_i.expect("invalid mouse cursor position");
    let mut start = cursor_i + mouse_x;
    while start > 0 {
        match heystack.get(start) {
            Some(&byte) if byte.is_ascii_control() => {}
            Some(&byte) if is_valid_path_char(byte) => {}
            _ => break,
        }

        start -= 1;
    }

    let mut end = cursor_i + mouse_x;
    loop {
        match heystack.get(end) {
            Some(&byte) if byte.is_ascii_control() => {}
            Some(&byte) if is_valid_path_char(byte) => {}
            _ => break,
        }

        end += 1;
    }

    let matched_bytes = &heystack[start..end];
    let mut matched_substr = std::str::from_utf8(matched_bytes)
        .expect("non-utf8 matched text")
        .to_string();
    matched_substr.retain(|c| !"\n\r".contains(c));

    let mut words = matched_substr.split(':');
    let path = words.next().map(|s| s.trim());
    let lineno = words.next().map(|s| s.trim());
    let colno = words.next().map(|s| s.trim());
    match (path, lineno, colno) {
        // foo.rs:10:5
        (Some(path), Some(lineno), Some(colno)) => {
            match (Path::new(path), lineno.parse::<usize>(), colno.parse()) {
                (path, Ok(lineno), Ok(colno)) if path.exists() && lineno > 0 => {
                    return Some((path.to_owned(), Point::new(lineno - 1, colno)));
                }
                _ => {}
            }
        }
        // foo.rs:10
        (Some(path), Some(lineno), None) => match (Path::new(path), lineno.parse::<usize>()) {
            (path, Ok(lineno)) if path.exists() && lineno > 0 => {
                return Some((path.to_owned(), Point::new(lineno - 1, 0)));
            }
            _ => {}
        },
        // foo.rs
        (Some(path), None, None) => {
            let path = Path::new(path);
            if path.exists() {
                return Some((path.to_owned(), Point::new(0, 0)));
            }
        }
        _ => {}
    }

    warn!("failed to parse a clicked path: '{}'", matched_substr);
    None
}
