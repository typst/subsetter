use super::{Error, Result};
use crate::Error::MissingData;

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

    /// Create a new readable stream of binary data at a specific position.
    pub fn new_at(data: &'a [u8], offset: usize) -> Result<Self> {
        let mut reader = Self { data, offset: 0 };
        reader.advance(offset)?;
        Ok(reader)
    }

    /// The remaining data from the current offset.
    pub fn tail(&self) -> &'a [u8] {
        &self.data[self.offset..]
    }

    /// Whether the reader has reached the end.
    pub fn at_end(&self) -> bool {
        self.offset >= self.data.len()
    }

    /// Returns the current offset.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Whether there is no data remaining.
    pub fn eof(&self) -> bool {
        self.offset >= self.data.len()
    }

    /// Try to read `T` from the data.
    pub fn read<T: Structure<'a>>(&mut self) -> Result<T> {
        T::read(self)
    }

    /// Read a certain number of bytes
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        let v = self.data.get(self.offset..self.offset + len).ok_or(MissingData)?;
        self.advance(len)?;
        Ok(v)
    }

    /// Try to read `T` from the data.
    pub fn skip<T: Structure<'a>>(&mut self) -> Result<()> {
        let _ = self.read::<T>()?;
        Ok(())
    }

    /// Try to read a vector of `T` from the data.
    pub fn read_vector<T: Structure<'a>>(&mut self, count: usize) -> Result<Vec<T>> {
        let mut res = Vec::with_capacity(count);

        for _ in 0..count {
            res.push(self.read::<T>()?);
        }

        Ok(res)
    }

    /// Take the next `n` bytes from the stream.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if n + self.offset <= self.data.len() {
            let slice = &self.data[self.offset..self.offset + n];
            self.offset += n;
            Ok(slice)
        } else {
            Err(Error::MissingData)
        }
    }

    /// Skip the next `n` bytes from the stream.
    pub fn advance(&mut self, n: usize) -> Result<()> {
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

    pub fn write_vector<'a, T: Structure<'a>>(&mut self, data: &Vec<T>) {
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

#[derive(Clone, Copy, Debug)]
pub struct U24(pub u32);

impl Structure<'_> for U24 {
    fn read(r: &mut Reader<'_>) -> crate::Result<Self> {
        let data = r.read::<[u8; 3]>()?;
        Ok(U24(u32::from_be_bytes([0, data[0], data[1], data[2]])))
    }

    fn write(&self, w: &mut Writer) {
        let data = self.0.to_be_bytes();
        w.write::<[u8; 3]>([data[0], data[1], data[2]]);
    }
}
