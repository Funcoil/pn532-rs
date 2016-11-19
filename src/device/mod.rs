mod proto;

use ::bus;
use self::proto::PN532Proto;
use ::error::{CommResult, CommError, RecvError, DataError};

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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PollingMethod {
    Probabilistic,
    Timeslot,
}

impl PollingMethod {
    fn code(self) -> u8 {
        match self {
            PollingMethod::Probabilistic => 0x01,
            PollingMethod::Timeslot => 0x00,
        }
    }
}

impl Default for PollingMethod {
    fn default() -> Self {
        PollingMethod::Timeslot
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Limit {
    One,
    Two,
}

impl From<Limit> for u8 {
    fn from(limit: Limit) -> u8 {
        match limit {
            Limit::One => 1,
            Limit::Two => 2,
        }
    }
}

pub enum ListTagData<'a> {
    ISO14443A(Limit, Option<&'a[u8]>),
    FeliCa212(Limit, [u8; 5]),
    FeliCa424(Limit, [u8; 5]),
    ISO14443B(Limit, u8, Option<PollingMethod>),
    JewelTag,
}

impl<'a> ListTagData<'a> {
    fn limit_u8(&self) -> u8 {
        use self::ListTagData::*;

        match *self {
            ISO14443A(ref l, _) => *l,
            ISO14443B(ref l, _, _) => *l,
            FeliCa212(ref l, _) => *l,
            FeliCa424(ref l, _) => *l,
            JewelTag => Limit::One,
        }.into()
    }

    fn fill_buf(&self, buf: &mut [u8]) -> usize {
        use self::ListTagData::*;
        use ::std::cmp::min;

        buf[0] = self.limit_u8();

        match *self {
            ISO14443A(_, Some(ref data)) => {
                buf[1] = 0x00;
                let to_copy = min(buf.len(), data.len());
                buf[2..(2 + to_copy)].copy_from_slice(&data[0..to_copy]);
                to_copy + 1
            },
            ISO14443A(_, None) => {
                buf[1] = 0x00;
                2
            },
            ISO14443B(_, ref afi, Some(ref polling_method)) => {
                buf[1] = 0x03;
                buf[2] = *afi;
                buf[3] = polling_method.code();
                4
            },
            ISO14443B(_, ref afi, None) => {
                buf[1] = 0x03;
                buf[2] = *afi;
                3
            },
            FeliCa212(_, ref payload) => {
                buf[1] = 0x01;
                buf[2..7].copy_from_slice(payload);
                7
            },
            FeliCa424(_, ref payload) => {
                buf[1] = 0x02;
                buf[2..7].copy_from_slice(payload);
                7
            },
            JewelTag => {
                buf[1] = 0x04;
                2
            },
        }
    }

    fn calc_recved_size(&self, data: &[u8]) -> usize {
        use self::ListTagData::*;

        match *self {
            ISO14443A(_, _) => {
                // TODO: check
                let nfcid_len = data[3] as usize;
                4 + nfcid_len + 1 + (data[4 + nfcid_len] as usize)
            },
            ISO14443B(_, _, _) => {
                // TODO: check
                let atrib_res_len = data[12] as usize;
                12 + 1 + atrib_res_len
            },
            FeliCa212(_, _) => {
                // TODO: check
                let pol_res_len = data[1] as usize;
                pol_res_len
            },
            FeliCa424(_, _) => {
                // TODO: check
                let pol_res_len = data[1] as usize;
                pol_res_len
            },
            JewelTag => 6,
        }
    }
}

impl<'a> Default for ListTagData<'a> {
    fn default() -> Self {
        ListTagData::ISO14443A(Limit::One, None)
    }
}

pub struct PN532<D: bus::WaitRead + bus::BusWrite> {
    device: PN532Proto<D>,
}

impl <D: bus::WaitRead + bus::BusWrite> PN532<D> {
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

    pub fn list_tags(&mut self, op_data: ListTagData) -> CommResult<Vec<Vec<u8>>, D::ReadError, D::WriteError> {
        let mut buf = [0u8; 256];
        buf[0] = 0x4A;
        let len = op_data.fill_buf(&mut buf[1..]);

        try!(self.device.send_wait_ack(&buf[..(1 + len)]));
        try!(self.device.recv_reply_ack(&mut buf));

        // TODO check buf[0] == 0x4B, buf[1] <= 2
        let ntags = buf[1] as usize;
        let mut res = Vec::with_capacity(ntags);
        
        let mut offset = 4;
        for _ in 0..ntags {
            let tag_len = op_data.calc_recved_size(&buf[offset..]);
            res.push((&buf[offset..(offset + tag_len)]).into());
            offset += tag_len + 1;
        }

        Ok(res)
    }
}
