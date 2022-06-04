use super::{Error, Result};

/// A readable stream of binary data.
pub struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    /// Create a new readable stream of binary data.
    pub fn new(data: &'a [u8]) -> Self {
        Self(data)
    }

    /// Try to read `T` from the data.
    pub fn read<T: Structure>(&mut self) -> Result<T> {
        T::read(self)
    }

    /// Take the first `n` bytes from the stream.
    pub fn take(&mut self, n: usize) -> Result<&[u8]> {
        if n <= self.0.len() {
            let head = &self.0[.. n];
            self.0 = &self.0[n ..];
            Ok(head)
        } else {
            Err(Error::MissingData)
        }
    }
}

/// A writable stream of binary data.
pub struct Writer(Vec<u8>);

impl Writer {
    /// Create a new writable stream of binary data.
    pub fn new() -> Self {
        Self(Vec::with_capacity(1024))
    }

    /// Try to read `T` from the data.
    pub fn write<T: Structure>(&mut self, data: T) {
        data.write(self);
    }

    /// Give bytes into the writer.
    pub fn give(&mut self, bytes: &[u8]) {
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
pub trait Structure: Sized {
    /// The memory size of the structure.
    const SIZE: usize;

    /// Try to read `Self` from the reader.
    fn read(r: &mut Reader) -> Result<Self>;

    /// Write `Self` into the writer.
    fn write(self, w: &mut Writer);

    /// Read self at the given offset in the binary data.
    fn read_at(data: &[u8], offset: usize) -> Result<Self> {
        let mut r = Reader::new(data);
        r.take(offset)?;
        Self::read(&mut r)
    }
}

impl<const N: usize> Structure for [u8; N] {
    const SIZE: usize = N;

    fn read(r: &mut Reader) -> Result<Self> {
        Ok(r.take(N)?.try_into().unwrap_or([0; N]))
    }

    fn write(self, w: &mut Writer) {
        w.give(&self)
    }
}

impl Structure for u8 {
    const SIZE: usize = 1;

    fn read(r: &mut Reader) -> Result<Self> {
        r.read::<[u8; 1]>().map(Self::from_be_bytes)
    }

    fn write(self, w: &mut Writer) {
        w.write::<[u8; 1]>(self.to_be_bytes());
    }
}

impl Structure for u16 {
    const SIZE: usize = 2;

    fn read(r: &mut Reader) -> Result<Self> {
        r.read::<[u8; 2]>().map(Self::from_be_bytes)
    }

    fn write(self, w: &mut Writer) {
        w.write::<[u8; 2]>(self.to_be_bytes());
    }
}

impl Structure for i16 {
    const SIZE: usize = 2;

    fn read(r: &mut Reader) -> Result<Self> {
        r.read::<[u8; 2]>().map(Self::from_be_bytes)
    }

    fn write(self, w: &mut Writer) {
        w.write::<[u8; 2]>(self.to_be_bytes());
    }
}

impl Structure for u32 {
    const SIZE: usize = 4;

    fn read(r: &mut Reader) -> Result<Self> {
        r.read::<[u8; 4]>().map(Self::from_be_bytes)
    }

    fn write(self, w: &mut Writer) {
        w.write::<[u8; 4]>(self.to_be_bytes());
    }
}
