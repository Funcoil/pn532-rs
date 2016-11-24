//! Contains traits for implementations of buses supported by PN532

#[cfg(feature = "with_i2c")]
pub mod i2c;

pub mod busy_wait;

pub use self::busy_wait::BusyWait;

use ::error::WaitResult;
use std::error::Error;

/// Abstracts reading from device over different busses (I2C, SPI, ...)
pub trait BusRead {
    /// Type returned when bus IO fails.
    type ReadError: Error;

    /// Reads data from device to `buf`.
    /// May return `Ok(n)` where `n < buf.len()` but usually it's expected
    /// to fill whole buffer.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::ReadError>;
}

/// Abstracts writing to device over different busses (I2C, SPI, ...)
pub trait BusWrite {
    /// Type returned when bus IO fails.
    type WriteError: Error;

    /// Writes data from `buf` to device.
    /// Continuation is not allowed.
    fn write(&mut self, buf: &[u8]) -> Result<(), Self::WriteError>;
}

/// Abstracts method of waiting for device.
pub trait WaitRead {
    /// Type returned when bus IO fails.
    type ReadError: Error;

    /// Blocks until device sends data, then reads the data.
    fn wait_read(&mut self, buf: &mut [u8]) -> Result<usize, Self::ReadError>;
}

/// Extends ability to wait with ability to timeout.
pub trait WaitReadTimeout: WaitRead {
    type Duration;
    /// Blocks until device sends data or operation times out,
    /// then reads the data or returns `Err(WaitError::Timeout)`.
    /// The timeout doesn't need to be exact.
    fn wait_read_timeout(&mut self, buf: &mut [u8], timeout: Self::Duration) -> WaitResult<usize, Self::ReadError>;
}


#[cfg(test)]
mod test {
    use super::*;
    use ::error::WaitError;

    struct NeverReady;

    impl BusRead for NeverReady {
        type ReadError = ::std::io::Error;

        fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::ReadError> {
            if buf.len() < 1 {
                Ok(0)
            } else {
                buf[0] = 0;
                Ok(1)
            }
        }
    }

    impl BusWrite for NeverReady {
        type WriteError = ::std::io::Error;

        fn write(&mut self, _: &[u8]) -> Result<(), Self::WriteError> {
            Ok(())
        }
    }

    #[test]
    fn test_self() {
        let mut buf = [1u8; 42];
        let mut never = NeverReady;
        never.read(&mut buf).unwrap();
        assert_eq!(buf[0] & 1, 0);
    }

    #[test]
    fn test_timeout() {
        use ::std::time::{Duration, Instant};
        let mut buf = [0u8; 42];
        let begin = Instant::now();
        let mut busy_wait = BusyWait::new(NeverReady);
        match busy_wait.wait_read_timeout(&mut buf, Duration::from_secs(1)) {
            Err(WaitError::Timeout) => (),
            Err(e) => panic!("{}", e),
            Ok(_) => panic!("Operation should've time out"),
        }
        assert!(begin.elapsed() > Duration::from_secs(1));
    }
}
