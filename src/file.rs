use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::cell::{RefCell, Ref, RefMut};
use crate::buffer::Buffer;

pub struct File {
    /// The file path. It's `None` if the buffer is pseudo one (e.g.
    /// scratch).
    path: Option<PathBuf>,
    buffer: Rc<RefCell<Buffer>>,
    /// It contains the y-axis offset from `top_left.line` from which the frontnend
    /// needs to redraw.
    /// TODO: Use this.
    needs_redraw: Option<usize>,
}

impl File {
    pub fn pseudo_file(name: &str) -> File {
        File {
            path: None,
            needs_redraw: Some(0),
            buffer: Rc::new(RefCell::new(Buffer::new(name))),
        }
    }

    pub fn open_file(name: &str, path: &Path) -> std::io::Result<File> {
        let handle = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path);

        let buffer = match handle {
            Ok(handle) => Buffer::from_file(name, &handle)?,
            Err(err) => {
                match err.kind() {
                    // TODO: Check the permission.
                    std::io::ErrorKind::NotFound => {
                        Buffer::new(name)
                    }
                    _ => {
                        return Err(err);
                    }
                }
            },

        };

        Ok(File {
            path: Some(path.to_owned()),
            buffer: Rc::new(RefCell::new(buffer)),
            needs_redraw: Some(0),
        })
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = match &self.path {
            Some(path) => path,
            None => return Ok(()),
        };

        trace!("saving the buffer to a file: {}", path.display());
        let mut handle = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        self.buffer.borrow_mut().write_to_file(&mut handle)
    }

    pub fn buffer<'a>(&'a self) -> Ref<'a, Buffer> {
        self.buffer.borrow()
    }

    pub fn buffer_mut<'a>(&'a mut self) -> RefMut<'a, Buffer> {
        self.buffer.borrow_mut()
    }

    pub fn mark_as_drawed(&mut self) {
        self.needs_redraw = None;
    }
}
