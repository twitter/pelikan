use std::error;
use std::ffi::NulError;
use std::fmt;
use std::io;
use std::num::ParseIntError;
use std::str::Utf8Error;

#[derive(Debug)]
pub enum CDBError {
    IOError(io::Error),
    UTF8Error(::std::str::Utf8Error),
    ParseError(ParseIntError),
    NulError(NulError),
}

impl From<ParseIntError> for CDBError {
    fn from(err: ParseIntError) -> CDBError {
        CDBError::ParseError(err)
    }
}

impl From<Utf8Error> for CDBError {
    fn from(err: Utf8Error) -> CDBError {
        CDBError::UTF8Error(err)
    }
}

impl From<io::Error> for CDBError {
    fn from(err: io::Error) -> CDBError {
        CDBError::IOError(err)
    }
}

impl From<NulError> for CDBError {
    fn from(err: NulError) -> CDBError {
        CDBError::NulError(err)
    }
}

impl error::Error for CDBError {
    fn description(&self) -> &str {
        match *self {
            CDBError::IOError(ref err) => err.description(),
            CDBError::UTF8Error(ref err) => err.description(),
            CDBError::ParseError(ref err) => err.description(),
            CDBError::NulError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            CDBError::IOError(ref err) => Some(err),
            CDBError::UTF8Error(ref err) => Some(err),
            CDBError::ParseError(ref err) => Some(err),
            CDBError::NulError(ref err) => Some(err),
        }
    }
}

impl fmt::Display for CDBError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CDBError::IOError(ref err) => err.fmt(f),
            CDBError::UTF8Error(ref err) => err.fmt(f),
            CDBError::ParseError(ref err) => err.fmt(f),
            CDBError::NulError(ref err) => err.fmt(f),
        }
    }
}
