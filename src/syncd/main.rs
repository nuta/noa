#[macro_use]
extern crate log;

mod eventloop;
mod lsp;

use daemonize::Daemonize;
use log::LevelFilter;
use noa_common::{
    dirs::{log_file_path, lsp_pid_path, lsp_sock_path},
    syncd_protocol::Notification,
};
use simplelog::{CombinedLogger, Config, TermLogger, TerminalMode, WriteLogger};
use std::{
    fs::{read_to_string, File, OpenOptions},
    path::PathBuf,
};
use structopt::StructOpt;

use crate::{eventloop::eventloop, lsp::LspDaemon};

#[derive(StructOpt)]
struct Opt {
    #[structopt(long, parse(from_os_str))]
    workspace_dir: PathBuf,
    #[structopt(long, name = "type")]
    daemon_type: String,
    #[structopt(long)]
    lang: String,
    #[structopt(long)]
    kill_existing_daemon: bool,
    #[structopt(long, short)]
    foreground: bool,
}

#[tokio::main]
async fn main() {
    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Trace,
            Config::default(),
            TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Trace,
            Config::default(),
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(log_file_path("syncd"))
                .unwrap(),
        ),
    ])
    .unwrap();

    std::panic::set_hook(Box::new(|info| {
        error!("{}", info);
        error!("{:#?}", backtrace::Backtrace::new());
    }));

    trace!("starting");

    let opt = Opt::from_args();

    let pid_file = match opt.daemon_type.as_str() {
        "lsp" => lsp_pid_path(&opt.workspace_dir, &opt.lang),
        _ => panic!("unknown daemon type: {}", opt.daemon_type),
    };

    if opt.kill_existing_daemon {
        if let Ok(pid) = read_to_string(&pid_file) {
            let pid = pid.parse().expect("failed to parse pid file");
            info!("found an existing daemon process (pid={}), killing...", pid);
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }

            std::fs::remove_file(&pid_file).ok();
        }
    }

    if !opt.foreground {
        Daemonize::new()
            .pid_file(pid_file)
            .working_directory(std::env::current_dir().unwrap())
            .stdout(File::open("/dev/null").unwrap())
            .stderr(File::open("/dev/null").unwrap())
            .start()
            .expect("failed to daemonize the process");
    }

    match opt.daemon_type.as_str() {
        "lsp" => {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Notification>();
            let sock_path = lsp_sock_path(&opt.workspace_dir, &opt.lang);

            let mut daemon = LspDaemon::spawn(tx, &opt.workspace_dir, opt.lang)
                .await
                .expect("failed to start the LSP mode");
            daemon
                .initialize()
                .await
                .expect("failed to initialize the LSP server");
            eventloop(&sock_path, daemon, rx).await.unwrap();
        }
        _ => unreachable!(),
    };

    trace!("exiting");
}
