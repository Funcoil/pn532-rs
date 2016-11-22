mod proto;
pub mod tags_internal;

use ::bus;
use self::proto::PN532Proto;
use ::error::{CommResult, CommError, RecvError, DataError};
use device::tags_internal::{TagListOptions, TagResponse, TagBuffer, Tags};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SAMMode {
    Normal(Option<u8>),
    VirtualCard(u8),
    WiredCard(Option<u8>),
    DualCard(Option<u8>),
}

impl SAMMode {
    fn code(self) -> u8 {
        use self::SAMMode::*;

        match self {
            Normal(_) => 0x01,
            VirtualCard(_) => 0x02,
            WiredCard(_) => 0x03,
            DualCard(_) => 0x04,
        }
    }

    fn timeout(self) -> Option<u8> {
        use self::SAMMode::*;

        match self {
            Normal(to) => to,
            VirtualCard(to) => Some(to),
            WiredCard(to) => to,
            DualCard(to) => to,
        }
    }
}

pub struct PN532<D: bus::WaitRead + bus::BusWrite> {
    device: PN532Proto<D>,
}

impl<D: bus::WaitRead + bus::BusWrite> PN532<D> {
    pub fn new(device: D) -> Self {
        PN532 {
            device: PN532Proto::new(device)
        }
    }

    pub fn sam_configure(&mut self, mode: SAMMode) -> CommResult<(), D::ReadError, D::WriteError> {
        let mut cmd_buf = [0x14, mode.code(), 0x01, 0x01];
        let cmd = match mode.timeout() {
            Some(to) => {
                cmd_buf[2] = to;
                &cmd_buf as &[u8]
            }
            None => {
                &cmd_buf[0..3] as &[u8]
            }
        };

        try!(self.device.send_wait_ack(cmd));
        let mut rcvbuf = [0u8];
        let len = try!(self.device.recv_reply_ack(&mut rcvbuf));
        if len > 0 {
            if rcvbuf[0] == 0x15 {
                Ok(())
            } else {
                Err(CommError::RecvError(RecvError::InvalidData(DataError::InvalidByte(rcvbuf[0], "0x15"))))
            }
        } else {
            Err(CommError::RecvError(RecvError::UnexpectedEnd))
        }
    }

    pub fn list_tags<'buf, 's, O: TagListOptions<'buf>>(&'s mut self, options: O, buf: &'buf mut TagBuffer) -> CommResult<Tags<'s, 'buf, O::Response, Self>, D::ReadError, D::WriteError> {
        unsafe {
            let raw_buf = ::core::intrinsics::transmute::<&mut TagBuffer, &mut [u8; 256]>(buf);
            raw_buf[0] = 0x4A;
            let len = options.fill_buf(&mut raw_buf[1..]);

            try!(self.device.send_wait_ack(&raw_buf[..(1 + len)]));
            try!(self.device.recv_reply_ack(raw_buf as &mut [u8]));
        }

        unsafe {
            Ok(Tags::new(buf, self))
        }
    }
}

impl<D: bus::WaitRead + bus::BusWrite> tags_internal::PN532Transceive for PN532<D> {
    type TransceiveError = CommError<D::ReadError, D::WriteError>;

    fn transceive(&mut self, tag_number: u8, data_out: &[u8], data_in: &mut [u8]) -> CommResult<usize, D::ReadError, D::WriteError> {
        use ::std::cmp::min;

        let mut buf = [0u8; 256];
        buf[0] = 0x40;
        buf[1] = tag_number.into();
        let to_copy = min(buf.len(), data_out.len());
        buf[2..(2 + to_copy)].copy_from_slice(&data_out[0..to_copy]);

        try!(self.device.send_wait_ack(&buf[..(2 + to_copy)]));
        let len = try!(self.device.recv_reply_ack(&mut buf));

        // TODO: check buf[0] == 0x41 && buf[1] is status OK
        let to_copy = min(len, data_in.len());
        data_in[0..to_copy].copy_from_slice(&buf[2..(2 + to_copy)]);

        Ok(to_copy)
    }
}
