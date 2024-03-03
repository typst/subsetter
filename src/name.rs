use super::*;
use crate::stream::{Readable, Writeable};
use crate::Error::MalformedFont;

struct NameRecord {
    platform_id: u16,
    encoding_id: u16,
    language_id: u16,
    name_id: u16,
    length: u16,
    string_offset: u16,
}

impl NameRecord {
    fn is_unicode(&self) -> bool {
        self.platform_id == 0
            || (self.platform_id == 3 && [0, 1, 10].contains(&self.encoding_id))
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

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let name = ctx.expect_table(Tag::NAME).ok_or(MalformedFont)?;
    let mut r = Reader::new(name);

    // From the over 3k font (variations) I had locally, none had version 1.
    // So we only focus on subsetting version 0 and on the off-chance
    // that a font has version 1, we just add it as is.
    let version = r.read::<u16>().ok_or(MalformedFont)?;

    if version != 0 {
        ctx.push(Tag::NAME, name);
        return Ok(());
    }

    let count = r.read::<u16>().ok_or(MalformedFont)?;
    r.read::<u16>().ok_or(MalformedFont)?; // storage offset

    let mut name_records = vec![];

    for _ in 0..count {
        name_records.push(r.read::<NameRecord>().ok_or(MalformedFont)?);
    }

    let storage = r.tail().ok_or(MalformedFont)?;

    let mut pruned = prune_name_records(name_records);

    if pruned.is_empty() && count != 0 {
        // Only contains non-Unicode records, so we don't subset.
        ctx.push(Tag::NAME, name);
        return Ok(());
    }

    let mut sub_name = Writer::new();
    let mut new_storage = Writer::new();
    let mut cur_storage_offset = 0;

    let count = pruned.len() as u16;

    // version
    sub_name.write::<u16>(0);
    // count
    sub_name.write::<u16>(count);
    // storage offset
    sub_name.write::<u16>(2 * 3 + count * 12);

    for record in &mut pruned {
        new_storage.extend(
            &storage[(record.string_offset as usize)
                ..((record.string_offset + record.length) as usize)],
        );
        record.string_offset = cur_storage_offset;
        record.write(&mut sub_name);
        cur_storage_offset += record.length;
    }

    sub_name.extend(&new_storage.finish());

    ctx.push(Tag::NAME, sub_name.finish());
    Ok(())
}

fn prune_name_records(name_records: Vec<NameRecord>) -> Vec<NameRecord> {
    let mut pruned = vec![];

    for record in name_records {
        if record.is_unicode() {
            // TODO: Determine which exact records we need. But the PDF reference
            // doesn't seem to indicate anything about this.
            if [0, 1, 2, 3, 4, 5, 6].contains(&record.name_id) {
                pruned.push(record);
            }
        }
    }

    pruned
}
