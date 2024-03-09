use crate::name::read::NameRecord;
use crate::name::read::Version0Table;
use crate::stream::{Readable, Writeable, Writer};
use std::borrow::Cow;

type SubsettedVersion0Table<'a> = Version0Table<'a>;

pub fn subset<'a>(table: &Version0Table<'a>) -> Option<SubsettedVersion0Table<'a>> {
    let mut names = table
        .names
        .iter()
        .copied()
        .filter(|record| {
            record.is_unicode() && [0, 1, 2, 3, 4, 5, 6].contains(&record.name_id)
        })
        .collect::<Vec<_>>();

    let mut storage = Vec::new();
    let mut cur_storage_offset = 0;

    for record in &mut names {
        storage.extend(
            &table.storage[(record.string_offset as usize)
                ..((record.string_offset + record.length) as usize)],
        );
        record.string_offset = cur_storage_offset;
        cur_storage_offset += record.length;
    }

    Some(SubsettedVersion0Table { names, storage: Cow::Owned(storage) })
}

impl Writeable for SubsettedVersion0Table<'_> {
    fn write(&self, w: &mut Writer) {
        let count = u16::try_from(self.names.len()).unwrap();

        // version
        w.write::<u16>(0);
        // count
        w.write::<u16>(count);
        // storage offset
        w.write::<u16>(u16::SIZE as u16 * 3 + count * NameRecord::SIZE as u16);
        w.write_vector::<NameRecord>(&self.names);
        w.extend(&self.storage);
    }
}

impl Writeable for NameRecord {
    fn write(&self, w: &mut Writer) {
        w.write::<u16>(self.platform_id);
        w.write::<u16>(self.encoding_id);
        w.write::<u16>(self.language_id);
        w.write::<u16>(self.name_id);
        w.write::<u16>(self.length);
        w.write::<u16>(self.string_offset);
    }
}