// ========================== Traits ==========================
pub trait TagResponse<'s>: 's + Sized {
    fn new(response: &'s [u8]) -> Self;
    fn buf(&self) -> &[u8];
    fn into_buf(self) -> &'s [u8];
    fn len(&self) -> usize;
    fn next(self) -> Self {
        let len = self.len();
        Self::new(&self.into_buf()[len..])
    }
    fn tag_num(&self) -> u8 {
        self.buf()[0]
    }
}

pub trait TagResponseMarker<'s>: TagResponse<'s> {}

impl<'a, T: TagResponse<'a>> TagResponseMarker<'a> for T {}

pub trait TagListOptions<'a> {
    type Response: TagResponse<'a>;

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

impl TagBuffer {
    pub fn new() -> Self {
        // This is safe because buffer can't be read before it's written
        // and even then, just what's been written.
        unsafe {
            ::core::mem::uninitialized()
        }
    }
}

pub struct Tags<'p, 'r, R: 'r + TagResponse<'r>, P: 'p + PN532Transceive> {
    response: R,
    // pn532 which detected the tags
    pn532: &'p mut P,
    count: usize,
    // Why the hell is this needed if 'r is actually used in R?
    _phantom: ::core::marker::PhantomData<&'r ()>,
}

impl<'p, 'r, R: 'r + TagResponse<'r>, P: 'p + PN532Transceive> Tags<'p, 'r, R, P> {
    // Unsafe because TagBuffer is not guaranteed to be initialized
    pub unsafe fn new(buf: &'r TagBuffer, pn532: &'p mut P) -> Self {
        Tags {
            response: R::new(&buf.buf[2..]),
            pn532: pn532,
            count: buf.buf[1] as usize,
            _phantom: Default::default(),
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn first(self) -> Tag<'p, 'r, R, P> {
        let count = self.count();
        Tag {
            response: self.response,
            pn532: self.pn532,
            last: count != 2,
            _phantom: Default::default(),
        }
    }
}

pub struct Tag<'p, 'r, R: 'r + TagResponse<'r>, P: 'p + PN532Transceive> {
    response: R,
    // pn532 which detected the tag
    pn532: &'p mut P,
    last: bool,
    // Why the hell is this needed if 'r is actually used in R?
    _phantom: ::core::marker::PhantomData<&'r ()>,
}

impl<'p, 'r, R: 'r + TagResponse<'r>, P: 'p + PN532Transceive> Tag<'p, 'r, R, P> {
    pub fn next(self) -> Option<Self> {
        if self.last {
            None
        } else {
            Some(Tag {
                response: self.response.next(),
                pn532: self.pn532,
                last: true,
                _phantom: Default::default(),
            })
        }
    }

    pub fn transceive(&mut self, data_to_tag: &[u8], data_from_tag: &mut [u8]) -> Result<usize, P::TransceiveError> {
        self.pn532.transceive(self.response.tag_num(), data_to_tag, data_from_tag)
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

pub struct ISO14443A<'a> {
    data: &'a [u8],
}

impl<'a> TagResponse<'a> for ISO14443A<'a> {
    fn new(buf: &'a [u8]) -> Self {
        ISO14443A {
            data: buf,
        }
    }

    fn len(&self) -> usize {
        self.id_len() + self.ats_len() + 4
    }

    fn buf(&self) -> &[u8] {
        self.data
    }

    fn into_buf(self) -> &'a [u8] {
        self.data
    }
}

impl<'a> ISO14443A<'a> {
    pub fn id_len(&self) -> usize {
        self.data[4] as usize
    }

    pub fn ats_len(&self) -> usize {
        self.data[5 + self.id_len()] as usize
    }
}

impl<'r, 'p, P: PN532Transceive> Tag<'p, 'r, ISO14443A<'r>, P> {
    pub fn sens_res(&self) -> u16 {
        ((self.response.buf()[1] as u16) << 8) | (self.response.buf()[2] as u16)
    }

    pub fn sel_res(&self) -> u8 {
        self.response.buf()[3]
    }

    pub fn id_len(&self) -> usize {
        self.response.id_len()
    }

    pub fn id(&self) -> &[u8] {
        &self.response.buf()[5..(5 + self.id_len())]
    }

    pub fn ats_len(&self) -> usize {
        self.response.ats_len()
    }

    pub fn ats(&self) -> &[u8] {
        &self.response.buf()[(5 + self.id_len() + 1)..]
    }
}

pub struct ISO14443AListOptions<'id> {
    pub limit: TagNumLimit,
    pub uid: Option<&'id [u8]>,
}

impl<'r, 'id> TagListOptions<'r> for ISO14443AListOptions<'id> {
    type Response = ISO14443A<'r>;

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

/*
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
*/
