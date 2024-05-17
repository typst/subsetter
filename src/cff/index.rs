use crate::cff::types::U24;
use crate::read::{Readable, Reader};

pub trait IndexSize: for<'a> Readable<'a> {
    fn to_u32(self) -> u32;
}

impl IndexSize for u16 {
    fn to_u32(self) -> u32 {
        u32::from(self)
    }
}

impl IndexSize for u32 {
    fn to_u32(self) -> u32 {
        self
    }
}

#[inline]
pub fn parse_index<'a, T: IndexSize>(r: &mut Reader<'a>) -> Option<Index<'a>> {
    let count = r.read::<T>()?;
    parse_index_impl(count.to_u32(), r)
}

#[inline(never)]
fn parse_index_impl<'a>(count: u32, r: &mut Reader<'a>) -> Option<Index<'a>> {
    if count == 0 || count == core::u32::MAX {
        return Some(Index::default());
    }

    let offset_size = r.read::<OffsetSize>()?;
    let offsets_len = (count + 1).checked_mul(offset_size.to_u32())?;
    let offsets = VarOffsets {
        data: r.read_bytes(offsets_len as usize)?,
        offset_size,
    };

    // Last offset indicates a Data Index size.
    match offsets.last() {
        Some(last_offset) => {
            let data = r.read_bytes(last_offset as usize)?;
            Some(Index { data, offsets })
        }
        None => Some(Index::default()),
    }
}

#[inline]
pub fn skip_index<T: IndexSize>(r: &mut Reader) -> Option<()> {
    let count = r.read::<T>()?;
    skip_index_impl(count.to_u32(), r)
}

#[inline(never)]
fn skip_index_impl(count: u32, r: &mut Reader) -> Option<()> {
    if count == 0 || count == core::u32::MAX {
        return Some(());
    }

    let offset_size = r.read::<OffsetSize>()?;
    let offsets_len = (count + 1).checked_mul(offset_size.to_u32())?;
    let offsets = VarOffsets {
        data: r.read_bytes(offsets_len as usize)?,
        offset_size,
    };

    if let Some(last_offset) = offsets.last() {
        r.skip_bytes(last_offset as usize);
    }

    Some(())
}

#[derive(Clone, Copy, Debug)]
pub struct VarOffsets<'a> {
    pub data: &'a [u8],
    pub offset_size: OffsetSize,
}

impl<'a> VarOffsets<'a> {
    pub fn get(&self, index: u32) -> Option<u32> {
        if index >= self.len() {
            return None;
        }

        let start = index as usize * self.offset_size.to_usize();
        let mut r = Reader::new_at(self.data, start);
        let n: u32 = match self.offset_size {
            OffsetSize::Size1 => u32::from(r.read::<u8>()?),
            OffsetSize::Size2 => u32::from(r.read::<u16>()?),
            OffsetSize::Size3 => r.read::<U24>()?.0,
            OffsetSize::Size4 => r.read::<u32>()?,
        };

        // Offsets are offset by one byte in the font,
        // so we have to shift them back.
        n.checked_sub(1)
    }

    #[inline]
    pub fn last(&self) -> Option<u32> {
        if !self.is_empty() {
            self.get(self.len() - 1)
        } else {
            None
        }
    }

    #[inline]
    pub fn len(&self) -> u32 {
        self.data.len() as u32 / self.offset_size as u32
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Index<'a> {
    pub data: &'a [u8],
    pub offsets: VarOffsets<'a>,
}

impl<'a> Default for Index<'a> {
    #[inline]
    fn default() -> Self {
        Index {
            data: b"",
            offsets: VarOffsets { data: b"", offset_size: OffsetSize::Size1 },
        }
    }
}

impl<'a> IntoIterator for Index<'a> {
    type Item = &'a [u8];
    type IntoIter = IndexIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IndexIter { data: self, offset_index: 0 }
    }
}

impl<'a> Index<'a> {
    #[inline]
    pub fn len(&self) -> u32 {
        // Last offset points to the byte after the `Object data`. We should skip it.
        self.offsets.len().saturating_sub(1)
    }

    pub fn get(&self, index: u32) -> Option<&'a [u8]> {
        let next_index = index.checked_add(1)?; // make sure we do not overflow
        let start = self.offsets.get(index)? as usize;
        let end = self.offsets.get(next_index)? as usize;
        self.data.get(start..end)
    }
}

pub struct IndexIter<'a> {
    data: Index<'a>,
    offset_index: u32,
}

impl<'a> Iterator for IndexIter<'a> {
    type Item = &'a [u8];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset_index == self.data.len() {
            return None;
        }

        let index = self.offset_index;
        self.offset_index += 1;
        self.data.get(index)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OffsetSize {
    Size1 = 1,
    Size2 = 2,
    Size3 = 3,
    Size4 = 4,
}

impl OffsetSize {
    #[inline]
    pub fn to_u32(self) -> u32 {
        self as u32
    }
    #[inline]
    pub fn to_usize(self) -> usize {
        self as usize
    }
}

impl Readable<'_> for OffsetSize {
    const SIZE: usize = 1;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        match r.read::<u8>()? {
            1 => Some(OffsetSize::Size1),
            2 => Some(OffsetSize::Size2),
            3 => Some(OffsetSize::Size3),
            4 => Some(OffsetSize::Size4),
            _ => None,
        }
    }
}
