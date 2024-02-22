use super::*;
use crate::Error::{InvalidOffset, MissingTable};

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
    fn read(r: &mut Reader) -> Result<Self> {
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

#[derive(Debug)]
struct Subtable4<'a> {
    language: u16,
    seg_count: u16,
    end_codes: Vec<u16>,
    start_codes: Vec<u16>,
    id_deltas: Vec<i16>,
    id_range_offsets: Vec<u16>,
    glyph_id_array: &'a [u8],
}

impl Subtable4<'_> {
    /// Returns a glyph index for a code point.
    ///
    /// Returns `None` when `code_point` is larger than `u16`.
    pub fn glyph_index(&self, code_point: u32) -> Option<u16> {
        // This subtable supports code points only in a u16 range.
        let code_point = u16::try_from(code_point).ok()?;

        // A custom binary search.
        let mut start = 0;
        let mut end = self.start_codes.len();
        while end > start {
            let index = (start + end) / 2;
            let end_value = *self.end_codes.get(index)?;
            if end_value >= code_point {
                let start_value = *self.start_codes.get(index)?;
                if start_value > code_point {
                    end = index;
                } else {
                    let id_range_offset = *self.id_range_offsets.get(index)?;
                    let id_delta = *self.id_deltas.get(index)?;
                    if id_range_offset == 0 {
                        return Some(code_point.wrapping_add(id_delta as u16));
                    } else if id_range_offset == 0xFFFF {
                        // Some malformed fonts have 0xFFFF as the last offset,
                        // which is invalid and should be ignored.
                        return None;
                    }

                    let delta = (u32::from(code_point) - u32::from(start_value)) * 2;
                    let delta = u16::try_from(delta).ok()?;

                    let id_range_offset_pos = (index * 2) as u16;
                    let pos = id_range_offset_pos.wrapping_add(delta);
                    let pos = pos.wrapping_add(id_range_offset);

                    let glyph_array_value: u16 =
                        u16::read_at(self.glyph_id_array, usize::from(pos)).ok()?;

                    // 0 indicates missing glyph.
                    if glyph_array_value == 0 {
                        return None;
                    }

                    let glyph_id = (glyph_array_value as i16).wrapping_add(id_delta);
                    return u16::try_from(glyph_id).ok();
                }
            } else {
                start = index + 1;
            }
        }

        None
    }

    /// Calls `f` for each codepoint defined in this table.
    pub fn codepoints(&self, mut f: impl FnMut(u32)) {
        for (start, end) in self.start_codes.iter().zip(&self.end_codes) {
            // OxFFFF value is special and indicates codes end.
            if *start == *end && *start == 0xFFFF {
                break;
            }

            for code_point in *start..=*end {
                f(u32::from(code_point));
            }
        }
    }
}

impl<'a> Structure<'a> for Subtable4<'a> {
    fn read(r: &mut Reader<'a>) -> Result<Self> {
        let data = r.data();
        r.skip(4)?; // format + length
        let language = r.read::<u16>()?;
        let seg_count_x2 = r.read::<u16>()?;

        if seg_count_x2 < 2 {
            return Err(InvalidData);
        }

        let seg_count = seg_count_x2 / 2;

        r.skip(6)?; // search range + entry selector + range shift

        let mut end_codes = vec![];

        for _ in 0..seg_count {
            end_codes.push(r.read::<u16>()?);
        }

        r.skip(2)?; // reserved pad

        let mut start_codes = vec![];

        for _ in 0..seg_count {
            start_codes.push(r.read::<u16>()?);
        }

        let mut id_deltas = vec![];

        for _ in 0..seg_count {
            id_deltas.push(r.read::<i16>()?);
        }

        let glyph_id_array = r.data();
        let mut id_range_offsets = vec![];

        for _ in 0..seg_count {
            id_range_offsets.push(r.read::<u16>()?);
        }

        println!("{:?}", start_codes);
        println!("{:?}", end_codes);
        println!("{:?}", id_deltas);
        println!("{:?}", id_range_offsets);

        Ok(Subtable4 {
            language,
            seg_count,
            end_codes,
            start_codes,
            id_deltas,
            id_range_offsets,
            glyph_id_array,
        })
    }

    fn write(&self, w: &mut Writer) {}
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
        let subtable_data =
            cmap.get((record.subtable_offset as usize)..).ok_or(InvalidOffset)?;
        match u16::read_at(subtable_data, 0) {
            Ok(4) => subset_subtable4(ctx, subtable_data)?,
            _ => {}
        }

        return Ok(());
    }

    Ok(())
}

fn subset_subtable4(ctx: &Context, data: &[u8]) -> Result<()> {
    let subtable = Subtable4::read_at(data, 0)?;
    let mut all_codepoints = vec![];
    subtable.codepoints(|c| all_codepoints.push(c as u16));

    let mut new_mappings = all_codepoints
        .into_iter()
        .filter_map(|c| {
            if let Some(g) = subtable.glyph_index(c as u32) {
                if ctx.subset.contains(&g) {
                    if let Some(new_g) = ctx.gid_map.get(&g) {
                        return Some((c, *new_g));
                    }
                }
            }

            return None;
        })
        .collect::<Vec<_>>();

    new_mappings.sort();

    let mut segments = vec![];

    let delta = |pair: (u16, u16)| {
        (pair.1 as i32 - pair.0 as i32) as i16
    };

    let mut map_iter = new_mappings.into_iter();
    let mut first = map_iter.next().ok_or(InvalidData)?;
    let mut cur_start = first.0;
    let mut cur_delta = delta(first);
    let mut cur_range = 0;

    while let Some(next) = map_iter.next() {
        if next.0 == cur_start + cur_range + 1 {
            if delta(next) == cur_delta {
                cur_range += 1;
                continue;
            }
        }

        segments.push((cur_start, cur_start + cur_range, cur_delta));
        cur_start = next.0;
        cur_delta = delta(next);
        cur_range = 0;
    }

    segments.push((cur_start, cur_start + cur_range, cur_delta));
    segments.push((0xFFFF, 0xFFFF, 1));

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
