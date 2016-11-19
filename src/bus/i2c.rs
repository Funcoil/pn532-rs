use ::i2cdev::core::I2CDevice;
use super::{BusRead, BusWrite};
use ::std::path::Path;

impl<D: I2CDevice> BusRead for D {
    type ReadError = D::Error;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::ReadError> {
        self.read(buf).map(|_| buf.len())
    }
}

impl<D: I2CDevice> BusWrite for D {
    type WriteError = D::Error;

    fn write(&mut self, buf: &[u8]) -> Result<(), Self::WriteError> {
        self.write(buf)
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
use ::i2cdev::linux::{LinuxI2CDevice, LinuxI2CError};

/// Opens i2c device with default address.
///
/// On Linux, the path should be "/dev/i2c-N", where N is non-negative integer.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn open<P: AsRef<Path>>(i2c_path: P) -> Result<LinuxI2CDevice, LinuxI2CError> {
    LinuxI2CDevice::new(i2c_path, 0x24)
}
