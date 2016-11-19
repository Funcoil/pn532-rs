//! Crate for communication with PN532 (NFC chip by NXP)

#[cfg(with_i2c)]
extern crate i2cdev;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

pub mod error;
pub mod bus;
mod device;

pub use device::{PN532, SAMMode, PollingMethod, ListTagData};
pub use device::Limit as ListTagLimit;
