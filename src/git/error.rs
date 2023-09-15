use std::io;
use std::string::FromUtf8Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    FromUtf8(FromUtf8Error),
    Command(String),
    LogParse(String),
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(value: FromUtf8Error) -> Self {
        Self::FromUtf8(value)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::FromUtf8(e) => write!(f, "{e}"),
            Self::Command(e) => write!(f, "{e}"),
            Self::LogParse(e) => write!(f, "Could not parse \"{e}\""),
        }
    }
}

impl std::error::Error for Error {}
