mod config;
mod error;
mod pacdiff;

use crate::config::Config;
use crate::pacdiff::run;

use std::process;

use structopt::StructOpt;

fn main() {
    let config = Config::from_args();
    let res = run(&config);

    if let Err(err) = res {
        eprintln!("{} {}", config.color.error.paint("error:"), err);
        process::exit(1);
    }
}
