use buffer::Buffer;
use dirs::home_dir;
use log::LevelFilter;
use simplelog::{Config, WriteLogger};
use std::{
    collections::HashMap,
    env::current_dir,
    fs::OpenOptions,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

#[macro_use]
extern crate log;
#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

mod buffer;
mod editorconfig;
mod rope;

pub struct Mainloop {
    workspace_dir: PathBuf,
    buffers: HashMap<PathBuf, Buffer>,
}

impl Mainloop {
    pub fn new(workspace_dir: PathBuf) -> Mainloop {
        Mainloop {
            workspace_dir,
            buffers: HashMap::new(),
        }
    }

    pub fn open_file(&mut self, path: &Path) {
        let abspath = match path.canonicalize() {
            Ok(abspath) => abspath,
            Err(err) => {
                self.error(format!(
                    "failed to resolve path: {} ({})",
                    path.display(),
                    err
                ));
                return;
            }
        };

        let buffer = match Buffer::open_file(&abspath) {
            Ok(buffer) => buffer,
            Err(err) => {
                self.error(format!(
                    "failed to open file: {} ({})",
                    abspath.display(),
                    err
                ));
                return;
            }
        };

        self.buffers.insert(abspath, buffer);
    }

    pub fn run(&mut self) {}

    fn error<T: Into<String>>(&self, message: T) {}
}

#[derive(StructOpt)]
struct Opt {
    #[structopt(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

pub fn main() {
    WriteLogger::init(
        LevelFilter::Trace,
        Config::default(),
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(home_dir().unwrap().join(".noa.log"))
            .unwrap(),
    )
    .unwrap();

    std::panic::set_hook(Box::new(|info| {
        error!("{}", info);
        error!("{:#?}", backtrace::Backtrace::new());
    }));

    trace!("starting");

    let opt = Opt::from_args();
    let mut mainloop = Mainloop::new(current_dir().unwrap());
    for file in opt.files.iter().rev() {
        mainloop.open_file(file);
    }

    mainloop.run();
}
