use super::*;
use crate::Error::MissingTable;

#[derive(Debug)]
struct EncodingRecord {
    platform_id: u16,
    encoding_id: u16,
    subtable_offset: u32
}

impl EncodingRecord {
    fn is_unicode(&self) -> bool {
        self.platform_id == 0
            || (self.platform_id == 3
                && [0, 1, 10].contains(&self.encoding_id))
    }
}

impl Structure<'_> for EncodingRecord {
    fn read(r: &mut Reader) -> Result<Self> {
        let platform_id = r.read::<u16>()?;
        let encoding_id = r.read::<u16>()?;
        let subtable_offset = r.read::<u32>()?;

        Ok(EncodingRecord {
            platform_id,
            encoding_id,
            subtable_offset
        })
    }

    fn write(&self, w: &mut Writer) {
        w.write::<u16>(self.platform_id);
        w.write::<u16>(self.encoding_id);
        w.write::<u32>(self.subtable_offset);
    }
}

// This function is heavily inspired by how fonttools does the subsetting of that
// table.
pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let cmap = ctx.expect_table(Tag::CMAP)?;
    let mut reader = Reader::new(cmap);

    let version = reader.read::<u16>()?;
    let num_tables = reader.read::<u16>()?;

    for _ in 0..num_tables {
        let record = reader.read::<EncodingRecord>()?;
        println!("{:?}", record);
    }

    Ok(())
}

// fn subset_subtable4(
//     ctx: &mut Context,
//     writer: &mut Writer,
//     subtable: &Subtable4,
// ) -> Result<Vec<u8>> {
//     let mut writer = Writer::new();
//     let mut all_codepoints = vec![];
//     subtable.codepoints(|c| all_codepoints.push(c));
//
//     let new_mappings = all_codepoints
//         .into_iter()
//         .filter_map(|c| {
//             if let Some(g) = subtable.glyph_index(c) {
//                 if ctx.subset.contains(&g.0) {
//                     if let Some(new_g) = ctx.gid_map.get(&g.0) {
//                         return Some((c, new_g));
//                     }
//                 }
//             }
//
//             return None;
//         })
//         .collect::<Vec<_>>();
//
//     println!("{:?}", new_mappings);
//
//     Ok(vec![])
// }
