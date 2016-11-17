//! Contains traits for implementations of buses supported by PN532

use ::error::{WaitError, WaitResult};
use std::time::Duration;
use std::{time, thread};
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

    /// Blocks until device sends data or operation times out,
    /// then reads the data or returns `Err(WaitError::Timeout)`.
    /// The timeout doesn't need to be exact.
    fn wait_read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> WaitResult<usize, Self::ReadError>;
}

/// Implements busy waiting for PN532 to be ready
pub struct BusyWait<D: BusRead + BusWrite> {
    device: D,
    delay: Duration,
}

impl<D: BusRead + BusWrite> BusyWait<D> {
    /// Enables busy waiting with default delay.
    pub fn new(device: D) -> Self {
        BusyWait {
            device: device,
            delay: Duration::from_millis(190)
        }
    }

    /// Enables busy waiting with custom delay.
    pub fn with_delay(device: D, delay: Duration) -> Self {
        BusyWait {
            device: device,
            delay: delay
        }
    }

    // One wait iteration
    fn wait_iter(&mut self, buf: &mut [u8]) -> Result<bool, D::ReadError> {
        thread::sleep(self.delay);

        try!(self.device.read(buf));

        Ok(buf[0] & 1 == 1)
    }
}

impl<D: BusRead + BusWrite> WaitRead for BusyWait<D> {
    type ReadError = D::ReadError;

    fn wait_read(&mut self, buf: &mut [u8]) -> Result<usize, Self::ReadError> {
        loop {
            if try!(self.wait_iter(buf)) {
                return Ok(buf.len());
            }
        }
    }

    fn wait_read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> WaitResult<usize, Self::ReadError> {
        let start_time = time::Instant::now();
        loop {
            if try!(self.wait_iter(buf)) {
                return Ok(buf.len());
            }

            if start_time.elapsed() > timeout {
                return Err(WaitError::Timeout);
            }
        }
    }
}

impl <D: BusRead + BusWrite> BusWrite for BusyWait<D> {
    type WriteError = D::WriteError;

    fn write(&mut self, buf: &[u8]) -> Result<(), Self::WriteError> {
        self.device.write(buf)
    }
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
