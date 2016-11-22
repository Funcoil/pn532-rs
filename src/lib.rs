//! Crate for communication with PN532 (NFC chip by NXP)

extern crate core;

#[cfg(feature = "with_i2c")]
extern crate i2cdev;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

pub mod error;
pub mod bus;
mod device;

pub use device::{PN532, SAMMode};

pub mod tags {
    pub use ::device::tags_internal::{
        TagBuffer,
        Tags,
        Tag,
        TagNumLimit,
        ISO14443A,
        ISO14443AListOptions,
        /*
        PollingMethod,
        ISO14443BListOptions,
        FeliCaBaudrate,
        FeliCaListOptions,
        JewelTagListOptions
        */
    };
}
