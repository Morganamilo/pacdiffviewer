use ansi_term::Color::{Blue, Green, Red};
use ansi_term::Style;
use atty::Stream::Stdout;
use structopt::clap::{arg_enum, AppSettings};
use structopt::StructOpt;

arg_enum! {
    #[allow(non_camel_case_types)]
    #[derive(Debug)]
    pub enum Action {
        view,
        skip,
        remove,
        overrwrite,
        quit,
    }
}

#[derive(StructOpt, Debug)]
#[structopt(
    rename_all = "kebab-case",
    about = "pacfiles manager",
    template = "{bin} - {about}\n\nUsage: {usage}\n\nOptions:\n{unified}",
    version_message = "display the version",
    help_message = "display this help menu",
    raw(setting = "AppSettings::UnifiedHelpMessage")
)]
pub struct Config {
    #[structopt(
        raw(possible_values = "&[\"never\", \"auto\", \"always\"]"),
        parse(from_str = "parse_color"),
        long = "color",
        default_value = "auto",
        raw(takes_value = "true"),
        raw(require_equals = "true"),
        help = "specify when to enable color"
    )]
    pub color: Colors,

    #[structopt(long = "dbpath", short = "b", help = "the dbpath to use")]
    pub dbpath: Option<String>,

    #[structopt(long = "root", short = "r", help = "the root dir to use")]
    pub root: Option<String>,

    #[structopt(long = "config", short = "c", help = "the pacman.conf to use")]
    pub config: Option<String>,

    #[structopt(
        long = "all",
        short = "a",
        help = "manage all pacfiles instead of providing a selection menu"
    )]
    pub all: bool,

    #[structopt(
        long = "output",
        short = "o",
        help = "print pacfiles instead of managing them"
    )]
    pub output: bool,

    #[structopt(long = "show hidden errors", short = "v", help = "")]
    pub verbose: bool,

    #[structopt(
        raw(possible_values = "&Action::variants()"),
        long = "action",
        raw(takes_value = "true"),
        help = "automatically perform an action on each pacfile"
    )]
    pub action: Option<Action>,

    #[structopt(
        env = "DIFFPROG",
        long = "diffprog",
        short = "d",
        help = "diff program to use"
    )]
    pub diffprog: Option<String>,

    #[structopt(
        long = "nosudoedit",
        help = "don't use sudo -e to open the editor under your user account"
    )]
    pub nosudoedit: bool,

    #[structopt(
        env = "SUDO_USER",
        long = "sudouser",
        short = "u",
        help = "user to change to when editing files"
    )]
    pub sudouser: Option<String>,

    pub targets: Vec<String>,
}

fn parse_color(s: &str) -> Colors {
    match s {
        "auto" if atty::is(Stdout) => Colors::new(),
        "always" => Colors::new(),
        _ => Colors::default(),
    }
}

#[derive(Default, Debug)]
pub struct Colors {
    pub bold: Style,
    pub error: Style,
    pub prompt: Style,
    pub info: Style,
}

impl Colors {
    pub fn new() -> Colors {
        Colors {
            bold: Style::new().bold(),
            error: Style::new().fg(Red),
            prompt: Style::new().fg(Blue),
            info: Style::new().fg(Green),
        }
    }
}
