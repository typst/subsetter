use std::fmt::{self, Debug, Formatter};
use std::ops::{Deref, DerefMut};

use crate::{Error, Reader, Result, Structure, Writer};

/// An INDEX data structure.
#[derive(Clone)]
pub struct Index<T>(pub Vec<T>);

impl<T> Index<T> {
    /// Create a new index with a single entry.
    pub fn from_one(item: T) -> Self {
        Self(vec![item])
    }

    /// Extract the index's first entry.
    pub fn into_one(self) -> Option<T> {
        self.0.into_iter().next()
    }
}

impl<'a, T> Structure<'a> for Index<T>
where
    T: Structure<'a>,
{
    fn read(r: &mut Reader<'a>) -> Result<Self> {
        let data = r.data();
        let count = r.read::<u16>()? as usize;
        if count == 0 {
            return Ok(Self(vec![]));
        }

        let offsize = r.read::<Offsize>()? as usize;
        let base = 3 + offsize * (count + 1) - 1;
        let mut read_offset = || {
            let mut bytes: [u8; 4] = [0; 4];
            bytes[4 - offsize..4].copy_from_slice(r.take(offsize)?);
            Ok(base + u32::from_be_bytes(bytes) as usize)
        };

        let mut objects = Vec::with_capacity(count);
        let mut last = read_offset()?;
        let mut skip = 0;
        for _ in 0..count {
            let offset = read_offset()?;
            let slice = data.get(last..offset).ok_or(Error::InvalidOffset)?;
            objects.push(T::read_at(slice, 0)?);
            skip += slice.len();
            last = offset;
        }

        r.skip(skip)?;
        Ok(Self(objects))
    }

    fn write(&self, w: &mut Writer) {
        w.write::<u16>(self.0.len() as u16);
        if self.0.is_empty() {
            return;
        }

        let mut buffer = Writer::new();
        let mut offsets = vec![];
        for object in &self.0 {
            offsets.push(1 + buffer.len() as u32);
            buffer.write_ref::<T>(object);
        }

        let end = 1 + buffer.len() as u32;
        offsets.push(end);

        let offsize = Offsize::select(end);
        w.write::<Offsize>(offsize);

        let offsize = offsize as usize;
        for offset in offsets {
            let bytes = u32::to_be_bytes(offset);
            w.give(&bytes[4 - offsize..4]);
        }

        w.give(&buffer.finish());
    }
}

impl<T: Debug> Debug for Index<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_list().entries(&self.0).finish()
    }
}

impl<T> Deref for Index<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Index<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The number of bytes an offset is encoded with.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
enum Offsize {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
}

impl Offsize {
    fn select(max: u32) -> Self {
        if max < (1 << 8) {
            Self::One
        } else if max < (1 << 16) {
            Self::Two
        } else if max < (1 << 24) {
            Self::Three
        } else {
            Self::Four
        }
    }
}

impl Structure<'_> for Offsize {
    fn read(r: &mut Reader) -> Result<Self> {
        match r.read::<u8>()? {
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            3 => Ok(Self::Three),
            4 => Ok(Self::Four),
            _ => Err(Error::InvalidOffset),
        }
    }

    fn write(&self, w: &mut Writer) {
        w.write::<u8>(*self as u8);
    }
}
