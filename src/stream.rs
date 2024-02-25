use super::{Error, Result};

/// A readable stream of binary data.
pub struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    /// Create a new readable stream of binary data.
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    /// The remaining data.
    pub fn tail(&self) -> &'a [u8] {
        &self.data[self.offset..]
    }

    /// Whether there is no data remaining.
    pub fn eof(&self) -> bool {
        self.offset >= self.data.len()
    }

    /// Try to read `T` from the data.
    pub fn read<T: Structure<'a>>(&mut self) -> Result<T> {
        T::read(self)
    }

    /// Take the first `n` bytes from the stream.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if n + self.offset <= self.data.len() {
            let slice = &self.data[self.offset..self.offset + n];
            self.offset += n;
            Ok(slice)
        } else {
            Err(Error::MissingData)
        }
    }

    /// Skip the first `n` bytes from the stream.
    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }
}

/// A writable stream of binary data.
pub struct Writer(Vec<u8>);

impl Writer {
    /// Create a new writable stream of binary data.
    pub fn new() -> Self {
        Self(Vec::with_capacity(1024))
    }

    /// Write `T` into the data.
    pub fn write<'a, T: Structure<'a>>(&mut self, data: T) {
        data.write(self);
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

/// Decode structures from a stream of binary data.
pub trait Structure<'a>: Sized {
    /// Try to read `Self` from the reader.
    fn read(r: &mut Reader<'a>) -> Result<Self>;

    /// Write `Self` into the writer.
    fn write(&self, w: &mut Writer);

    /// Read self at the given offset in the binary data.
    fn read_at(data: &'a [u8], offset: usize) -> Result<Self> {
        if let Some(sub) = data.get(offset..) {
            Self::read(&mut Reader::new(sub))
        } else {
            Err(Error::InvalidOffset)
        }
    }
}

impl<const N: usize> Structure<'_> for [u8; N] {
    fn read(r: &mut Reader) -> Result<Self> {
        Ok(r.take(N)?.try_into().unwrap_or([0; N]))
    }

    fn write(&self, w: &mut Writer) {
        w.extend(self)
    }
}

impl Structure<'_> for u8 {
    fn read(r: &mut Reader) -> Result<Self> {
        r.read::<[u8; 1]>().map(Self::from_be_bytes)
    }

    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 1]>(self.to_be_bytes());
    }
}

impl Structure<'_> for u16 {
    fn read(r: &mut Reader) -> Result<Self> {
        r.read::<[u8; 2]>().map(Self::from_be_bytes)
    }

    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 2]>(self.to_be_bytes());
    }
}

impl Structure<'_> for i16 {
    fn read(r: &mut Reader) -> Result<Self> {
        r.read::<[u8; 2]>().map(Self::from_be_bytes)
    }

    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 2]>(self.to_be_bytes());
    }
}

impl Structure<'_> for u32 {
    fn read(r: &mut Reader) -> Result<Self> {
        r.read::<[u8; 4]>().map(Self::from_be_bytes)
    }

    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 4]>(self.to_be_bytes());
    }
}

impl Structure<'_> for i32 {
    fn read(r: &mut Reader) -> Result<Self> {
        r.read::<[u8; 4]>().map(Self::from_be_bytes)
    }

    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 4]>(self.to_be_bytes());
    }
}
