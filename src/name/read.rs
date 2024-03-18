use crate::stream::{Readable, Reader};
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub struct Version0Table<'a> {
    pub names: Vec<NameRecord>,
    pub storage: Cow<'a, [u8]>,
}

impl<'a> Version0Table<'a> {
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        let mut r = Reader::new(data);

        let version = r.read::<u16>()?;

        if version != 0 {
            return None;
        }

        let count = r.read::<u16>()?;
        r.read::<u16>()?; // storage offset

        let mut names = Vec::with_capacity(count as usize);

        for _ in 0..count {
            names.push(r.read::<NameRecord>()?);
        }

        for record in &names {
            println!(
                "{:?}",
                &r.tail()?[record.string_offset as usize
                    ..record.string_offset as usize + record.length as usize]
            )
        }

        let storage = Cow::Borrowed(r.tail()?);

        Some(Self { names, storage })
    }
}

impl Readable<'_> for NameRecord {
    const SIZE: usize = u16::SIZE * 6;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        let platform_id = r.read::<u16>()?;
        let encoding_id = r.read::<u16>()?;
        let language_id = r.read::<u16>()?;
        let name_id = r.read::<u16>()?;
        let length = r.read::<u16>()?;
        let string_offset = r.read::<u16>()?;

        Some(Self {
            platform_id,
            encoding_id,
            language_id,
            name_id,
            length,
            string_offset,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NameRecord {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub language_id: u16,
    pub name_id: u16,
    pub length: u16,
    pub string_offset: u16,
}

impl NameRecord {
    pub fn is_unicode(&self) -> bool {
        self.platform_id == 0
            || (self.platform_id == 3 && [0, 1, 10].contains(&self.encoding_id))
    }
}
