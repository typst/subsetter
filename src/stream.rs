use crate::util::LazyArray16;

#[derive(Clone, Debug)]
/// A readable stream of binary data.
pub struct Reader<'a> {
    /// The underlying data of the reader.
    data: &'a [u8],
    /// The current offset in bytes. Is not guaranteed to be in range.
    offset: usize,
}

impl<'a> Reader<'a> {
    /// Create a new readable stream of binary data.
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    /// Create a new readable stream of binary data at a specific position.
    pub fn new_at(data: &'a [u8], offset: usize) -> Self {
        Self { data, offset }
    }

    /// The remaining data from the current offset.
    pub fn tail(&self) -> Option<&'a [u8]> {
        self.data.get(self.offset..)
    }

    // /// Returns the current offset.
    // pub fn offset(&self) -> usize {
    //     self.offset
    // }

    /// Try to read `T` from the data.
    pub fn read<T: Readable<'a>>(&mut self) -> Option<T> {
        T::read(self)
    }

    // TODO: Add skip function

    /// Read a certain number of bytes.
    pub fn read_bytes(&mut self, len: usize) -> Option<&'a [u8]> {
        let v = self.data.get(self.offset..self.offset + len)?;
        self.offset += len;
        Some(v)
    }

    /// Reads the next `count` types as a slice.
    #[inline]
    pub fn read_array16<T: Readable<'a>>(
        &mut self,
        count: u16,
    ) -> Option<LazyArray16<'a, T>> {
        let len = usize::from(count) * T::SIZE;
        self.read_bytes(len).map(LazyArray16::new)
    }

    /// Advances by `Readable::SIZE`.
    #[inline]
    pub fn skip<T: Readable<'a>>(&mut self) {
        self.skip_bytes(T::SIZE);
    }

    pub fn at_end(&self) -> bool {
        self.offset >= self.data.len()
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Jump to a specific location.
    pub fn jump(&mut self, offset: usize) {
        self.offset = offset;
    }

    /// Try to read a vector of `T` from the data.
    pub fn read_vector<T: Readable<'a>>(&mut self, count: usize) -> Option<Vec<T>> {
        let mut res = Vec::with_capacity(count);

        for _ in 0..count {
            res.push(self.read::<T>()?);
        }

        Some(res)
    }

    /// Skip the next `n` bytes from the stream.
    pub fn skip_bytes(&mut self, n: usize) {
        self.read_bytes(n).map(|_| ());
    }
}

/// A writable stream of binary data.
pub struct Writer(Vec<u8>);

impl Writer {
    /// Create a new writable stream of binary data.
    pub fn new() -> Self {
        Self(Vec::with_capacity(1024))
    }

    /// Create a new writable stream of binary data with a capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    /// Write `T` into the data.
    pub fn write<T: Writeable>(&mut self, data: T) {
        data.write(self);
    }

    pub fn write_vector<T: Writeable>(&mut self, data: &Vec<T>) {
        for el in data {
            el.write(self);
        }
    }

    /// Give bytes into the writer.
    pub fn extend(&mut self, bytes: &[u8]) {
        self.0.extend(bytes);
    }

    /// Align the contents to a byte boundary.
    pub fn align(&mut self, to: usize) {
        while self.0.len() % to != 0 {
            self.0.push(0);
        }
    }

    /// The number of written bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return the written bytes.
    pub fn finish(self) -> Vec<u8> {
        self.0
    }
}

pub trait Readable<'a>: Sized {
    const SIZE: usize;

    fn read(r: &mut Reader<'a>) -> Option<Self>;
}

pub trait Writeable: Sized {
    fn write(&self, w: &mut Writer);
}

impl<const N: usize> Readable<'_> for [u8; N] {
    const SIZE: usize = u8::SIZE * N;

    fn read(r: &mut Reader) -> Option<Self> {
        Some(r.read_bytes(N)?.try_into().unwrap_or([0; N]))
    }
}

impl<const N: usize> Writeable for [u8; N] {
    fn write(&self, w: &mut Writer) {
        w.extend(self)
    }
}

impl Readable<'_> for u8 {
    const SIZE: usize = 1;

    fn read(r: &mut Reader) -> Option<Self> {
        r.read::<[u8; 1]>().map(Self::from_be_bytes)
    }
}
impl Writeable for u8 {
    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 1]>(self.to_be_bytes());
    }
}

impl Readable<'_> for u16 {
    const SIZE: usize = 2;

    fn read(r: &mut Reader) -> Option<Self> {
        r.read::<[u8; 2]>().map(Self::from_be_bytes)
    }
}
impl Writeable for u16 {
    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 2]>(self.to_be_bytes());
    }
}

impl Readable<'_> for i16 {
    const SIZE: usize = 2;

    fn read(r: &mut Reader) -> Option<Self> {
        r.read::<[u8; 2]>().map(Self::from_be_bytes)
    }
}
impl Writeable for i16 {
    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 2]>(self.to_be_bytes());
    }
}

impl Readable<'_> for u32 {
    const SIZE: usize = 4;

    fn read(r: &mut Reader) -> Option<Self> {
        r.read::<[u8; 4]>().map(Self::from_be_bytes)
    }
}

impl Writeable for u32 {
    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 4]>(self.to_be_bytes());
    }
}

impl Readable<'_> for i32 {
    const SIZE: usize = 4;

    fn read(r: &mut Reader) -> Option<Self> {
        r.read::<[u8; 4]>().map(Self::from_be_bytes)
    }
}
impl Writeable for i32 {
    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 4]>(self.to_be_bytes());
    }
}

#[derive(Clone, Copy, Debug)]
pub struct U24(pub u32);

impl Readable<'_> for U24 {
    const SIZE: usize = 3;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        let data = r.read::<[u8; 3]>()?;
        Some(U24(u32::from_be_bytes([0, data[0], data[1], data[2]])))
    }
}

impl Writeable for U24 {
    fn write(&self, w: &mut Writer) {
        let data = self.0.to_be_bytes();
        w.write::<[u8; 3]>([data[0], data[1], data[2]]);
    }
}

/// A type-safe wrapper for string ID.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Debug, Hash)]
pub struct StringId(pub u16);

impl Readable<'_> for StringId {
    const SIZE: usize = u16::SIZE;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Self(r.read::<u16>()?))
    }
}

impl Writeable for StringId {
    fn write(&self, w: &mut Writer) {
        w.write::<u16>(self.0)
    }
}

impl From<u16> for StringId {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

/// A 32-bit signed fixed-point number (16.16).
#[derive(Clone, Copy, Debug)]
pub struct Fixed(pub f32);

impl Readable<'_> for Fixed {
    const SIZE: usize = 4;

    #[inline]
    fn read(r: &mut Reader<'_>) -> Option<Self> {
        // TODO: is it safe to cast?
        i32::read(r).map(|n| Fixed(n as f32 / 65536.0))
    }
}
