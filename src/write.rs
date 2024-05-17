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

pub trait Writeable: Sized {
    fn write(&self, w: &mut Writer);
}

impl<const N: usize> Writeable for [u8; N] {
    fn write(&self, w: &mut Writer) {
        w.extend(self)
    }
}

impl Writeable for u8 {
    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 1]>(self.to_be_bytes());
    }
}

impl Writeable for &[u8] {
    fn write(&self, w: &mut Writer) {
        w.extend(self);
    }
}

impl Writeable for u16 {
    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 2]>(self.to_be_bytes());
    }
}

impl Writeable for i16 {
    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 2]>(self.to_be_bytes());
    }
}

impl Writeable for u32 {
    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 4]>(self.to_be_bytes());
    }
}

impl Writeable for i32 {
    fn write(&self, w: &mut Writer) {
        w.write::<[u8; 4]>(self.to_be_bytes());
    }
}
