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
