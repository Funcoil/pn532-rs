//! Contains error types and corresponding impls.

use ::std::error;
use ::std::fmt;

/// Error type used for operations that may timeout.
#[derive(Debug)]
pub enum WaitError<E: error::Error> {
    /// Some other error occured.
    OtherError(E),

    /// Operation timed out.
    Timeout,
}

impl<E: error::Error> WaitError<E> {
    pub fn map<E2: error::Error, F: FnOnce(E) -> E2>(self, f: F) -> WaitError<E2> {
        match self {
            WaitError::OtherError(e) => WaitError::OtherError(f(e)),
            WaitError::Timeout => WaitError::Timeout,
        }
    }
}

impl<E: error::Error> fmt::Display for WaitError<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            WaitError::OtherError(ref e) => write!(f, "error while waiting for data: {}", e),
            WaitError::Timeout => write!(f, "operation timed out"),
        }
    }
}

impl <E: error::Error> error::Error for WaitError<E> {
    fn description(&self) -> &str {
        match *self {
            WaitError::OtherError(ref e) => e.description(),
            WaitError::Timeout => "operation timed out",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            WaitError::OtherError(ref e) => Some(e),
            WaitError::Timeout => None,
        }
    }
}

impl<E: error::Error> From<E> for WaitError<E> {
    fn from(e: E) -> Self {
        WaitError::OtherError(e)
    }
}

/// Type returned from functions which may timeout.
pub type WaitResult<T, E> = Result<T, WaitError<E>>;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ChecksumType {
    Length,
    Data,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DataError {
    InvalidChecksum(ChecksumType),
    InvalidByte(u8, &'static str),
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            DataError::InvalidChecksum(ref ct) => write!(f, "packet {} has invalid checksum", if *ct == ChecksumType::Length { "length" } else { "data" }),
            DataError::InvalidByte(ref b, ref expected) => write!(f, "invalid byte ({}) encountered. Expected {}.", b, expected),
        }
    }
}

#[derive(Debug)]
pub enum RecvError<E: error::Error> {
    ReadError(E),
    InvalidData(DataError),
    UnexpectedEnd,
}

impl<E: error::Error> From<DataError> for RecvError<E> {
    fn from(e: DataError) -> Self {
        RecvError::InvalidData(e)
    }
}

impl<E: error::Error> fmt::Display for RecvError<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            RecvError::ReadError(ref e) => write!(f, "read error: {}", e),
            RecvError::InvalidData(ref d) => write!(f, "error parsing packet: {}", d),
            RecvError::UnexpectedEnd => write!(f, "received message is too short"),
        }
    }
}

impl<E: error::Error> error::Error for RecvError<E> {
    fn description(&self) -> &str {
        "error receiving message from PN532"
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            RecvError::ReadError(ref e) => Some(e),
            RecvError::InvalidData(_) => None,
            RecvError::UnexpectedEnd => None,
        }
    }
}

#[derive(Debug)]
pub enum SendError<E: error::Error> {
    WriteError(E),
    TooMuchData(usize),
}

impl<E: error::Error> From<E> for SendError<E> {
    fn from(e: E) -> Self {
        SendError::WriteError(e)
    }
}

impl<E: error::Error> fmt::Display for SendError<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SendError::WriteError(ref e) => write!(f, "{}", e),
            SendError::TooMuchData(l) => write!(f, "tried to write {} bytes of data but writing more than 254 bytes is not supported", l),
        }
    }
}

impl<E: error::Error> error::Error for SendError<E> {
    fn description(&self) -> &str {
        "sending message to PN532 failed"
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            SendError::WriteError(ref e) => Some(e),
            SendError::TooMuchData(l) => None,
        }
    }
}
