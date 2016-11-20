// ========================== Traits ==========================
pub trait TagType<'opt, 'inf> {
    type Options: TagListOptions;
    type Info;

    fn resp_len(response: &[u8]) -> usize;
    fn info(response: &'inf [u8]) -> Self::Info;
}

pub trait TagListOptions {
    fn fill_buf(&self, buf: &mut [u8]) -> usize;
}

pub trait PN532Transceive {
    type TransceiveError;

    fn transceive(&mut self, tag_number: u8, data_to_tag: &[u8], data_from_tag: &mut [u8]) -> Result<usize, Self::TransceiveError>;
}

// ========================== Tags ==========================
pub struct TagBuffer {
    buf: [u8; 256],
}

pub struct Tags<'buf, 'pn, T: for<'o> TagType<'o, 'buf>, P: 'pn + PN532Transceive> {
    buf: &'buf [u8; 256],
    // pn532 which detected the tag
    pn532: &'pn mut P,
    _phantom: ::core::marker::PhantomData<T>,
}

impl<'buf, 'pn, T: for<'o> TagType<'o, 'buf>, P: 'pn + PN532Transceive> Tags<'buf, 'pn, T, P> {
    pub unsafe fn new(buf: &'buf TagBuffer, pn532: &'pn mut P) -> Self {
        Tags {
            buf: &buf.buf,
            pn532: pn532,
            _phantom: Default::default(),
        }
    }

    pub fn count(&self) -> usize {
        self.buf[1] as usize
    }

    pub fn first(self) -> Tag<'buf, 'pn, T, P> {
        Tag {
            data: &self.buf[3..],
            device: self.pn532,
            last: self.buf[1] == 1,
            _phantom: Default::default(),
        }
    }
}

pub struct Tag<'buf, 'pn, T: for<'o> TagType<'o, 'buf>, P: 'pn + PN532Transceive> {
    data: &'buf [u8],
    device: &'pn mut P,
    last: bool,
    _phantom: ::core::marker::PhantomData<T>,
}

impl<'buf, 'pn, T: for<'o> TagType<'o, 'buf>, P: 'pn + PN532Transceive> Tag<'buf, 'pn, T, P> {
    pub fn next(self) -> Option<Self> {
        if self.last {
            None
        } else {
            Some(Tag {
                data: &self.data[T::resp_len(self.data)..],
                device: self.device,
                last: true,
                _phantom: self._phantom,
            })
        }
    }

    pub fn info<'o>(&self) -> <T as TagType<'o, 'buf>>::Info {
        <T as TagType<'o, 'buf>>::info(self.data)
    }

    pub fn transceive(&mut self, data_to_tag: &[u8], data_from_tag: &mut [u8]) -> Result<usize, P::TransceiveError> {
        self.device.transceive(self.data[0], data_to_tag, data_from_tag)
    }
}

// ======================= Specific tag impls =======================
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TagNumLimit {
    One,
    Two,
}

impl From<TagNumLimit> for u8 {
    fn from(limit: TagNumLimit) -> u8 {
        match limit {
            TagNumLimit::One => 1,
            TagNumLimit::Two => 2,
        }
    }
}

pub struct ISO14443ATagInfo<'a> {
    data: &'a [u8],
}

impl<'a> ISO14443ATagInfo<'a> {
    pub fn sens_res(&self) -> u16 {
        ((self.data[1] as u16) << 8) | (self.data[2] as u16)
    }

    pub fn sel_res(&self) -> u8 {
        self.data[3]
    }

    pub fn id_len(&self) -> usize {
        self.data[4] as usize
    }

    pub fn id(&self) -> &[u8] {
        &self.data[5..(5 + self.id_len())]
    }

    pub fn ats_len(&self) -> usize {
        self.data[5 + self.id_len()] as usize
    }

    pub fn ats(&self) -> &[u8] {
        &self.data[(5 + self.id_len() + 1)..]
    }
}

/*
impl<'t, 'inf, 'pn, T: for<'o> TagType<'o, 'inf>, P: 'pn + PN532Transceive> IntoIterator for &'t Tags<'inf, 'pn, T, P> where Tags<'inf, 'pn, T, P>: 't {
    type IntoIter = TagInfoIter<'inf, T>;

    fn into_iter(self) -> Self::IntoIter {
        TagInfoIter {
            data: self.data[2..],
            cnt: self.data[1],
        }
    }
}

pub struct TagInfoIter<'inf, T: for<'o> TagType<'o, 'inf>> {
    data: &'inf [u8],
    cnt: u8,
    _phantom: ::core::marker::PhantomData<T>,
}

impl<'opt, 'inf, T: TagType<'opt, 'inf>> Iterator for TagInfoIter<'inf, T> {
    type Item = <T as TagType<'opt, 'inf>>::Info;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cnt > 0 {
            self.cnt -= 1;
            let tag_info = ISO14443ATagInfo { data: self.data };
            let len = tag_info.id_len() + tag_info.ats_len() + 5;
            self.data = &self.data[len..];
            Some(tag_info)
        } else {
            None
        }
    }
}
*/

pub struct ISO14443A;

impl<'inf, 'opt> TagType<'opt, 'inf> for ISO14443A {
    type Info = ISO14443ATagInfo<'inf>;
    type Options = ISO14443AListOptions<'opt>;

    fn resp_len(response: &[u8]) -> usize {
        let info = Self::info(response);
        info.id_len() + info.ats_len() + 4
    }

    fn info(response: &'inf [u8]) -> Self::Info {
        ISO14443ATagInfo {
            data: response
        }
    }
}

pub struct ISO14443AListOptions<'a> {
    pub limit: TagNumLimit,
    pub uid: Option<&'a [u8]>,
}

impl<'a> TagListOptions for ISO14443AListOptions<'a> {
    fn fill_buf(&self, buf: &mut [u8]) -> usize {
        use ::core::cmp::min;

        buf[0] = self.limit.into();
        buf[1] = 0x00;
        self.uid.map_or(2, |data| {
            let to_copy = min(buf.len() - 2, data.len());
            buf[2..(2 + to_copy)].copy_from_slice(&data[0..to_copy]);
            to_copy + 2
        })
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
pub struct ISO14443BListOptions {
    pub limit: TagNumLimit,
    pub afi: u8,
    pub polling_method: Option<PollingMethod>,
}

impl TagListOptions for ISO14443BListOptions {
    fn fill_buf(&self, buf: &mut [u8]) -> usize {
        buf[0] = self.limit.into();
        buf[1] = 0x03;
        buf[2] = self.afi;
        self.polling_method.map_or(3, |method| {
            buf[3] = method.code();
            4
        })
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FeliCaBaudrate {
    Br212,
    Br424,
}

impl FeliCaBaudrate {
    fn code(self) -> u8 {
        match self {
            FeliCaBaudrate::Br212 => 0x01,
            FeliCaBaudrate::Br424 => 0x02,
        }
    }
}

pub struct FeliCaListOptions {
    pub limit: TagNumLimit,
    pub baudrate: FeliCaBaudrate,
    pub payload: [u8; 5],
}

impl TagListOptions for FeliCaListOptions {
    fn fill_buf(&self, buf: &mut [u8]) -> usize {
        buf[0] = self.limit.into();
        buf[1] = self.baudrate.code();
        buf[2..7].copy_from_slice(&self.payload);
        7
    }
}

pub struct JewelTagListOptions;

impl TagListOptions for JewelTagListOptions {
    fn fill_buf(&self, buf: &mut [u8]) -> usize {
        buf[0] = 1;
        buf[1] = 0x04;
        2
    }
}
