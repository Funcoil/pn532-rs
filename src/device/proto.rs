use ::bus;
use std::error::Error;
use std::time::Duration;
use ::error::{DataError, ChecksumType, RecvError, SendError, WaitError, WaitResult, CommResult};
use ::std::default::Default;

// State machine to parse Preamble.
// Could have been bool, but that would be less readable.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum PreambleParser {
    Start,
    ZeroFound,
}

impl PreambleParser {
    // One iteration of state machine
    pub fn next(self, b: u8) -> Option<Self> {
        use self::PreambleParser::*;

        match (self, b) {
            (ZeroFound, 0xFF) => None,
            (_,         0x00) => Some(ZeroFound),
            _                 => Some(Start),
        }
    }
}

impl Default for PreambleParser {
    fn default() -> Self {
        PreambleParser::Start
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ResponseParser {
    Preamble(PreambleParser),
    Length,
    LengthChksum(u8),
    FrameIdentifier(u8),
    Done(u8),
}

impl ResponseParser {
    pub fn next(&mut self, b: u8) -> Result<bool, DataError> {
        use self::ResponseParser::*;

        *self = match *self {
            Preamble(pp)                              => pp.next(b)
                                                           .map_or(Length, Preamble),
            Length => LengthChksum(b),
            LengthChksum(l) if l.wrapping_add(b) == 0 => FrameIdentifier(l),
            FrameIdentifier(l) if b == 0xD5           => Done(l),
            Done(l)                                   => Done(l),

            LengthChksum(_)    => return Err(DataError::InvalidChecksum(ChecksumType::Length)),
            FrameIdentifier(_) => return Err(DataError::InvalidByte(b, "0xD5")),
        };

        if let Done(_) = *self {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    pub fn pkt_len(&self) -> Option<u8> {
        if let ResponseParser::Done(l) = *self {
            Some(l)
        } else {
            None
        }
    }
}

impl Default for ResponseParser {
    fn default() -> Self {
        ResponseParser::Preamble(PreambleParser::default())
    }
}

pub struct PN532Proto<D: bus::WaitRead + bus::BusWrite> {
    device: D,
}

impl<D: bus::WaitRead + bus::BusWrite> PN532Proto<D> {
    pub fn new(device: D) -> Self {
        PN532Proto { device: device }
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), SendError<D::WriteError>> {
        if data.len() > 254 {
            return Err(SendError::TooMuchData(data.len()));
        }
        let mut outbuf = [0u8; 262];
         
        outbuf[1] = 0xFF;
        outbuf[2] = (data.len() + 1) as u8;
        outbuf[3] = 0u8.wrapping_sub(outbuf[2]);
        outbuf[4] = 0xD4;
        outbuf[5+data.len()] = 0u8.wrapping_sub(calc_checksum(0xD4, data));
        outbuf[5..(5 + data.len())].copy_from_slice(data);

        self.device.write(&outbuf[0..(data.len() + 6)]).map_err(Into::into)
    }

    pub fn send_wait_ack(&mut self, data: &[u8]) -> ::error::CommResult<(), D::ReadError, D::WriteError> {
        try!(self.send(data));
        try!(self.recv_ack());
        Ok(())
    }

    fn process_packet(recved: &[u8], dst: &mut [u8]) -> Result<usize, RecvError<D::ReadError>> {
        use ::std::cmp::min;

        let mut iter = recved.iter();
        let mut parser = ResponseParser::default();
        for b in iter.by_ref() {
            if !try!(parser.next(*b)) {
                break;
            }
        }

        let len = try!(parser.pkt_len().ok_or(RecvError::UnexpectedEnd)) as usize;

        let pkt = iter.as_slice();
        if len > pkt.len() {
            return Err(RecvError::UnexpectedEnd);
        }

        if len == 0 {
            return Err(RecvError::InvalidData(DataError::InvalidByte(0, "value at least 0x01")));
        }

        let slice = &pkt[0..len];
        if calc_checksum(0xD5, &slice) != 0 {
            return Err(RecvError::InvalidData(DataError::InvalidChecksum(ChecksumType::Data)));
        }

        let to_copy = min(len - 1, dst.len());

        let slice = &slice[0..to_copy];
        let dst = &mut dst[0..to_copy];

        dst.copy_from_slice(slice);

        Ok(to_copy)
    }

    pub fn recv(&mut self, data: &mut[u8]) -> Result<usize, RecvError<D::ReadError>> {
        let mut buf = [0u8; 32];
        let len = try!(self.device.wait_read(&mut buf).map_err(RecvError::ReadError));

        Self::process_packet(&buf[0..len], data)
    }

    pub fn recv_ack(&mut self) -> Result<(), RecvError<D::ReadError>> {
        let mut buf = [0u8; 32];
        try!(self.device.wait_read(&mut buf).map_err(RecvError::ReadError));

        let mut parser = PreambleParser::default();
        for b in &buf {
            parser = match parser.next(*b) {
                Some(parser) => parser,
                None => return Ok(()),
            };
        }

        Err(RecvError::UnexpectedEnd)
    }

    pub fn recv_with_timeout(&mut self, data: &mut[u8], timeout: ::std::time::Duration) -> WaitResult<usize, RecvError<D::ReadError>> {
        let mut buf = [0u8; 32];
        let len = try!(self.device.wait_read_timeout(&mut buf, timeout).map_err(|e| e.map(RecvError::ReadError)));

        Self::process_packet(&buf[0..len], data).map_err(Into::into)
    }
}

fn calc_checksum(init: u8, data: &[u8]) -> u8 {
    data.iter().fold(init, |a, b| a.wrapping_add(*b))
}

#[cfg(test)]
mod test {
    use ::std::io;
    use ::bus::{BusRead, BusWrite};

    #[test]
    fn preamble_parser() {
        use super::PreambleParser;

        let mut pp = PreambleParser::default();
        let arr1 = [0, 1, 2, 3];

        for b in &arr1 {
            pp = pp.next(*b).unwrap()
        }

        let mut pp = PreambleParser::default();
        let arr2 = [0, 1, 2, 3, 0, 0xFF];

        for (n, b) in arr2.iter().enumerate() {
            pp = if let Some(pp) = pp.next(*b) {
                pp
            } else {
                assert_eq!((n, *b), (5, 0xFF));
                return;
            }
        }
    }

    #[test]
    fn response_parser() {
        use super::ResponseParser;

        let mut parser = ResponseParser::default();

        let arr1 = [0, 1, 2, 0, 0xFF, 1, 0xFF, 0xD5];
        let mut iter = arr1.iter();

        while parser.next(*iter.next().unwrap()).unwrap() {}

        assert_eq!(parser.pkt_len(), Some(1));

        let mut parser = ResponseParser::default();
        let arr2 = [0, 1, 2, 0, 0xFF, 1];
        for b in &arr2 {
            assert_eq!(parser.next(*b), Ok(true));
        }
        assert_eq!(parser.pkt_len(), None);
    }

    #[test]
    fn chksum() {
        let data = [1, 2, 3];
        assert_eq!(super::calc_checksum(0, &data), 6);
        assert_eq!(super::calc_checksum(2, &data), 8);
        assert_eq!(super::calc_checksum(253, &data), 3);
    }

    struct BufSender<'a> {
        pub buf_to_send: &'a [u8],
    }

    impl<'a> BusRead for BufSender<'a> {
        type ReadError = io::Error;

        fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
            use ::std::cmp::min;

            if buf.len() == 0 {
                Ok(0)
            } else {
                let to_copy = min(buf.len(), self.buf_to_send.len());
                buf[..to_copy].copy_from_slice(&self.buf_to_send[..to_copy]);
                Ok(buf.len())
            }
        }
    }

    impl<'a> BusWrite for BufSender<'a> {
        type WriteError = io::Error;

        fn write(&mut self, buf: &[u8]) -> Result<(), io::Error> {
            Ok(())
        }
    }

    struct Echo {
        buf: [u8; 262],
    }

    impl Echo {
        pub fn new() -> Self {
            Echo {
                buf: [0; 262]
            }
        }
    }

    impl BusRead for Echo {
        type ReadError = io::Error;

        fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
            use ::std::cmp::min;

            if buf.len() == 0 {
                Ok(0)
            } else {
                buf[0] = 0x01;
                let to_copy = min(buf.len() - 1, self.buf.len());
                buf[1..(1 + to_copy)].copy_from_slice(&self.buf[..to_copy]);
                Ok(buf.len())
            }
        }
    }

    impl BusWrite for Echo {
        type WriteError = io::Error;

        fn write(&mut self, buf: &[u8]) -> Result<(), io::Error> {
            use ::std::cmp::min;

            let to_copy = min(buf.len(), self.buf.len());
            self.buf[..to_copy].copy_from_slice(&buf[..to_copy]);
            self.buf[4] = 0xD5;
            self.buf[buf.len() - 1] = self.buf[buf.len() - 1].wrapping_sub(1);
            
            Ok(())
        }
    }

    // buf to proto
    fn b2p<'a>(buf: &'a [u8]) -> super::PN532Proto<::bus::BusyWait<BufSender<'a>>> {
        use super::PN532Proto;
        use ::bus::BusyWait;

        PN532Proto::new(BusyWait::new(BufSender {
            buf_to_send: buf
        }))
    }

    fn fn_chk<F: Fn(&[u8], Result<usize, ::error::RecvError<::std::io::Error>>)>(f: F) -> F {
        f
    }

    macro_rules! chk_recv {
        ($b:expr, $chfn:expr) => {
            let innerbuf = $b;
            let mut proto = b2p(&innerbuf);
            let mut rcvbuf = [0u8; 32];
            let res = proto.recv(&mut rcvbuf);
            fn_chk($chfn)(&rcvbuf, res);
        };
    }

    #[test]
    fn recv_unexpected_end() {
        use ::error::RecvError;

        chk_recv!([0x01],
                 |_, res| assert_matches!(res.unwrap_err(), RecvError::UnexpectedEnd));
        chk_recv!([0x01, 0x00],
                 |_, res| assert_matches!(res.unwrap_err(), RecvError::UnexpectedEnd));
        chk_recv!([0x01, 0x00, 0xFF, 0xFF, 0x01, 0xD5],
                 |_, res| assert_matches!(res.unwrap_err(), RecvError::UnexpectedEnd));
    }

    #[test]
    fn recv_correct_data() {
        chk_recv!([0x01, 0x00, 0xFF, 0x01, 0xFF, 0xD5, 0x2B],
                 |_, res| {
                     assert_eq!(res.unwrap(), 0);
                 });
        chk_recv!([0x01, 0x00, 0xFF, 0x02, 0xFE, 0xD5, 0x00, 0x2B],
                 |buf, res| {
                     assert_eq!(res.unwrap(), 1);
                     assert_eq!(buf[0], 0x00);
                 });
        chk_recv!([0x01, 0x00, 0xFF, 0x02, 0xFE, 0xD5, 0x01, 0x2A],
                 |buf, res| {
                     assert_eq!(res.unwrap(), 1);
                     assert_eq!(buf[0], 0x01);
                 });
        chk_recv!([0x01, 0x00, 0xFF, 0x02, 0xFE, 0xD5, 0xFF, 0x2C],
                 |buf, res| {
                     assert_eq!(res.unwrap(), 1);
                     assert_eq!(buf[0], 0xFF);
                 });
        chk_recv!([0x01, 0x00, 0x00, 0xFF, 0x02, 0xFE, 0xD5, 0xFF, 0x2C],
                 |buf, res| {
                     assert_eq!(res.unwrap(), 1);
                     assert_eq!(buf[0], 0xFF);
                 });
        chk_recv!([0x01, 0x33, 0x00, 0x00, 0xFF, 0x02, 0xFE, 0xD5, 0xFF, 0x2C],
                 |buf, res| {
                     assert_eq!(res.unwrap(), 1);
                     assert_eq!(buf[0], 0xFF);
                 });
        chk_recv!([0x01, 0x00, 0xFF, 0x02, 0xFE, 0xD5, 0xFF, 0x2C, 0x00],
                 |buf, res| {
                     assert_eq!(res.unwrap(), 1);
                     assert_eq!(buf[0], 0xFF);
                 });
        chk_recv!([0x01, 0x00, 0xFF, 0x02, 0xFE, 0xD5, 0xFF, 0x2C, 0x00, 0x33],
                 |buf, res| {
                     assert_eq!(res.unwrap(), 1);
                     assert_eq!(buf[0], 0xFF);
                 });
        chk_recv!([0x01, 0x00, 0x00, 0xFF, 0x02, 0xFE, 0xD5, 0xFF, 0x2C, 0x00],
                 |buf, res| {
                     assert_eq!(res.unwrap(), 1);
                     assert_eq!(buf[0], 0xFF);
                 });
    }

    #[test]
    fn recv_invalid_chksum() {
        use ::error::{RecvError, DataError, ChecksumType};

        chk_recv!([0x01, 0x00, 0xFF, 0x02, 0xFF, 0xD5],
                 |_, res| assert_matches!(res.unwrap_err(), RecvError::InvalidData(DataError::InvalidChecksum(ChecksumType::Length))));
        chk_recv!([0x01, 0x00, 0xFF, 0x00, 0xFF, 0xD5],
                 |_, res| assert_matches!(res.unwrap_err(), RecvError::InvalidData(DataError::InvalidChecksum(ChecksumType::Length))));
        chk_recv!([0x01, 0x00, 0xFF, 0x01, 0xFF, 0xD5, 0x01],
                 |_, res| assert_matches!(res.unwrap_err(), RecvError::InvalidData(DataError::InvalidChecksum(ChecksumType::Data))));
        chk_recv!([0x01, 0x00, 0xFF, 0x02, 0xFE, 0xD5, 0x00, 0x00],
                 |_, res| assert_matches!(res.unwrap_err(), RecvError::InvalidData(DataError::InvalidChecksum(ChecksumType::Data))));
    }

    #[test]
    fn recv_invalid_byte() {
        use ::error::{RecvError, DataError};

        chk_recv!([0x01, 0x00, 0xFF, 0x01, 0xFF, 0xD4],
                 |_, res| assert_matches!(res.unwrap_err(), RecvError::InvalidData(DataError::InvalidByte(0xD4, "0xD5"))));
        chk_recv!([0x01, 0x00, 0xFF, 0x00, 0x00, 0xD5],
                 |_, res| assert_matches!(res.unwrap_err(), RecvError::InvalidData(DataError::InvalidByte(0x00, "value at least 0x01"))));
    }

    #[test]
    fn send() {
        use ::bus::BusyWait;
        use super::PN532Proto;

        let mut recvbuf = [0u8; 256];
        let echo = Echo::new();
        let busy_wait = BusyWait::new(echo);
        let mut proto = PN532Proto::new(busy_wait);

        proto.send(&[]).unwrap();
        assert_eq!(proto.recv(&mut recvbuf).unwrap(), 0);
        assert_eq!(recvbuf[0], 0);

        proto.send(&[42]).unwrap();
        assert_eq!(proto.recv(&mut recvbuf).unwrap(), 1);
        assert_eq!(recvbuf[0], 42);

        proto.send(&[42, 47]).unwrap();
        assert_eq!(proto.recv(&mut recvbuf).unwrap(), 2);
        assert_eq!(recvbuf[0], 42);
        assert_eq!(recvbuf[1], 47);

        proto.send(&[0, 1, 2]).unwrap();
        assert_eq!(proto.recv(&mut recvbuf).unwrap(), 3);
        assert_eq!(recvbuf[0], 0);
        assert_eq!(recvbuf[1], 1);
        assert_eq!(recvbuf[2], 2);
    }
}
