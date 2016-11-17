//! Crate for communication with PN532 (NFC chip by NXP)

#[cfg(with_i2c)]
extern crate i2cdev;

pub mod error;
pub mod bus;
