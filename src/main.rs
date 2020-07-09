#![allow(unused)]

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

#[macro_use]
extern crate log;

mod buffer;
mod rope;
mod logger;

use structopt::StructOpt;

#[derive(StructOpt)]
struct Opt {
}

fn main() {
    let log_colors = fern::colors::ColoredLevelConfig::new()
        .info(fern::colors::Color::Green);
    let log_file = fern::log_file(dirs::home_dir().unwrap().join(".noa.log"))
        .expect("failed to open ~/.noa.log");
    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{}\t] {}: {}",
                log_colors.color(record.level()),
                record.file().unwrap_or(record.target()),
                message,
            ))
        })
        .chain(log_file)
        .apply()
        .expect("failed to initialize the logger");

    std::panic::set_hook(Box::new(|info| {
        error!("{}", info);
        error!("{:#?}", backtrace::Backtrace::new());
    }));

    trace!("starting noa...");
    info!("starting noa...");
    warn!("starting noa...");
    let opt = Opt::from_args();
}
