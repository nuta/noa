use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use noa_buffer::Point;

pub fn open_path_in_tmux(pane: &str, mouse_y: usize, mouse_x: usize) {
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

    if let Some((path, point)) = extract_path_and_point(&output.stdout, mouse_y, mouse_x) {
        println!("opening {:?} {:?}", path, point);
    }
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

    None
}
