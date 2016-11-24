//! This module contains types and routines for busy waiting strategy
//! of communicating with PN532.

use super::{BusRead, BusWrite, WaitRead, WaitReadTimeout};
use error::{WaitResult, WaitError};

/// It might be necessary to change this on other platforms.
/// So it's made to be type alias.
pub type Milliseconds = u64;

/// Represents type which can be constructed by specifying number of milliseconds.
pub trait FromMilliseconds {
    /// Constructs Self by converting from milliseconds.
    fn from_milliseconds(milliseconds: Milliseconds) -> Self;
}

/// Represents type which can perform timing operations: waiting and measuring time.
pub trait Timer {
    /// The duration type - that means the difference between two absolute times.
    type Duration: Ord;

    /// Returns object which represents current absolute time.
    fn now() -> Self;

    /// Calculates how much time elapsed since self was created (by now() method).
    fn elapsed(&self) -> Self::Duration;

    /// Suspends (by blocking) execution of code until given time elapses.
    fn wait(duration: &Self::Duration);
}

/// Implements busy waiting for PN532 to be ready
/// In order to support both std and bare-metal, it's parametrized.
pub struct BusyWait<D: BusRead + BusWrite, T: Timer> {
    device: D,
    delay: T::Duration,
}

impl<D: BusRead + BusWrite, T: Timer> BusyWait<D, T> where T::Duration: FromMilliseconds {
    /// Enables busy waiting with default delay.
    pub fn new(device: D) -> Self {
        BusyWait {
            device: device,
            delay: T::Duration::from_milliseconds(190)
        }
    }
}

impl<D: BusRead + BusWrite, T: Timer> BusyWait<D, T> {
    /// Enables busy waiting with custom delay.
    pub fn with_delay(device: D, delay: T::Duration) -> Self {
        BusyWait {
            device: device,
            delay: delay
        }
    }

    // One wait iteration
    fn wait_iter(&mut self, buf: &mut [u8]) -> Result<bool, D::ReadError> {
        T::wait(&self.delay);

        try!(self.device.read(buf));

        Ok(buf[0] & 1 == 1)
    }
}

impl<D: BusRead + BusWrite, T: Timer> WaitRead for BusyWait<D, T> {
    type ReadError = D::ReadError;

    fn wait_read(&mut self, buf: &mut [u8]) -> Result<usize, Self::ReadError> {
        loop {
            if try!(self.wait_iter(buf)) {
                return Ok(buf.len());
            }
        }
    }
}

impl<D: BusRead + BusWrite, T: Timer> WaitReadTimeout for BusyWait<D, T> {
    type Duration = T::Duration;

    fn wait_read_timeout(&mut self, buf: &mut [u8], timeout: Self::Duration) -> WaitResult<usize, Self::ReadError> {
        let start_time = T::now();
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

impl <D: BusRead + BusWrite, T: Timer> BusWrite for BusyWait<D, T> {
    type WriteError = D::WriteError;

    fn write(&mut self, buf: &[u8]) -> Result<(), Self::WriteError> {
        self.device.write(buf)
    }
}

/// Implements appropriate traits for std types.
/// TODO: make compilation of this module conditional.
/// (This module should be disabled in case of no_std.)
mod std_impls {
    use super::{Milliseconds, FromMilliseconds, Timer};
    use ::std::time::{Duration, Instant};
    use ::std::thread::sleep;

    impl FromMilliseconds for Duration {
        fn from_milliseconds(milliseconds: Milliseconds) -> Self {
            Duration::from_millis(milliseconds)
        }
    }

    impl Timer for Instant {
        type Duration = Duration;

        fn now() -> Self {
            Instant::now()
        }

        fn elapsed(&self) -> Self::Duration {
            Instant::elapsed(self)
        }

        fn wait(duration: &Self::Duration) {
            sleep(*duration);
        }
    }
}
