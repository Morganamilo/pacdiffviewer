use std::{fmt, io};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    AlpmInit(alpm::Error, String, String),
    Alpm(alpm::Error),
    Pacmanconf(pacmanconf::Error),
    Io(io::Error),
    CommandNonZero(String, Vec<String>, Option<i32>),
    CommandFailed(String, Vec<String>, io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::AlpmInit(e, root, db) => write!(
                fmt,
                "failed to initialize alpm with root=\"{}\" dbpath=\"{}\": {}",
                root, db, e
            ),
            Error::Alpm(e) => e.fmt(fmt),
            Error::Pacmanconf(e) => e.fmt(fmt),
            Error::Io(e) => e.fmt(fmt),
            Error::CommandNonZero(bin, args, exit) => {
                if let Some(exit) = exit {
                    write!(
                        fmt,
                        "command failed: exited {}: '{} {}'",
                        exit,
                        bin,
                        args.join(" ")
                    )
                } else {
                    write!(
                        fmt,
                        "command failed: exited {}: '{} {}'",
                        "terminated by signal",
                        bin,
                        args.join(" ")
                    )
                }
            }
            Error::CommandFailed(bin, args, err) => {
                write!(fmt, "command failed: '{} {}': {}", bin, args.join(" "), err)
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<alpm::Error> for Error {
    fn from(err: alpm::Error) -> Self {
        Error::Alpm(err)
    }
}

impl From<pacmanconf::Error> for Error {
    fn from(err: pacmanconf::Error) -> Self {
        Error::Pacmanconf(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}
