use crate::cmap::subtable12::subset_subtable12;
use crate::cmap::subtable4::subset_subtable4;
use crate::stream::{Reader, Structure, Writer};
use crate::Error::InvalidOffset;
use crate::Result;
use crate::{Context, Tag};

mod subtable12;
mod subtable4;

#[derive(Debug)]
struct EncodingRecord {
    platform_id: u16,
    encoding_id: u16,
    subtable_offset: u32,
}

impl EncodingRecord {
    fn is_unicode(&self) -> bool {
        self.platform_id == 0
            || (self.platform_id == 3 && [0, 1, 10].contains(&self.encoding_id))
    }
}

impl Structure<'_> for EncodingRecord {
    fn read(r: &mut Reader) -> crate::Result<Self> {
        let platform_id = r.read::<u16>()?;
        let encoding_id = r.read::<u16>()?;
        let subtable_offset = r.read::<u32>()?;

        Ok(EncodingRecord { platform_id, encoding_id, subtable_offset })
    }

    fn write(&self, w: &mut Writer) {
        w.write::<u16>(self.platform_id);
        w.write::<u16>(self.encoding_id);
        w.write::<u32>(self.subtable_offset);
    }
}

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let new_subtables = subset_subtables(ctx)?;
    let num_tables = new_subtables.len() as u16;

    let mut sub_cmap = Writer::new();
    let mut subtables = Writer::new();

    // 2 * (version + num_tables) + num_tables * (platform_id + encoding_id + subtable_offset)
    let mut subtable_offset = 2 * 2 + num_tables as u32 * 8;

    sub_cmap.write::<u16>(0); // version
    sub_cmap.write::<u16>(new_subtables.len() as u16);

    for (mut record, data) in new_subtables {
        record.subtable_offset = subtable_offset;
        sub_cmap.write::<EncodingRecord>(record);
        subtables.extend(&data);
        subtable_offset += data.len() as u32;
    }

    sub_cmap.extend(&subtables.finish());

    ctx.push(Tag::CMAP, sub_cmap.finish());
    Ok(())
}

fn subset_subtables(
    ctx: &mut Context,
) -> Result<Vec<(EncodingRecord, Vec<u8>)>> {
    let cmap = ctx.expect_table(Tag::CMAP)?;
    let mut reader = Reader::new(cmap);

    reader.read::<u16>()?; // version
    let num_tables = reader.read::<u16>()?;
    let mut new_subtables = vec![];

    for _ in 0..num_tables {
        let record = reader.read::<EncodingRecord>()?;
        // We follow fonttools approach here, were by default we discard non-Unicode
        // cmap records.
        if record.is_unicode() {
            let subtable_data =
                cmap.get((record.subtable_offset as usize)..).ok_or(InvalidOffset)?;

            match u16::read_at(subtable_data, 0) {
                Ok(4) => {
                    new_subtables.push((record, subset_subtable4(ctx, subtable_data)?));
                }
                // TODO: Optimization: if an entry already exists in a 4 table we subsetted, we don't
                // need to add it to the 12 one again. It seems like fonttools does this.
                Ok(12) => {
                    new_subtables.push((record, subset_subtable12(ctx, subtable_data)?));
                }
                // TODO: Implement subtable 14 and add tests for it.
                _ => {}
            }
        }
    }

    Ok(new_subtables)
}
