use std::fmt;
use crate::parser::ParseError;

#[derive(Debug)]
pub enum YamamvaError {
    Parse(String),
    Runtime(String),
    Io(String),
}

impl fmt::Display for YamamvaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            YamamvaError::Parse(msg) => write!(f, "parse error: {}", msg),
            YamamvaError::Runtime(msg) => write!(f, "runtime error: {}", msg),
            YamamvaError::Io(msg) => write!(f, "io error: {}", msg),
        }
    }
}

impl std::error::Error for YamamvaError {}

impl From<ParseError> for YamamvaError {
    fn from(e: ParseError) -> Self {
        YamamvaError::Parse(e.message)
    }
}
