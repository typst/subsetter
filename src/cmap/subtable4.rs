use crate::stream::{Reader, Structure, Writer};
use crate::Context;
use crate::Error::InvalidData;
use std::borrow::Cow;

/// A Format 4 subtable.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Subtable4<'a> {
    language: u16,
    seg_count: u16,
    end_codes: Vec<u16>,
    start_codes: Vec<u16>,
    id_deltas: Vec<i16>,
    id_range_offsets: Vec<u16>,
    glyph_id_array: Cow<'a, [u8]>,
}

// TODO: Add attribution to ttf-parser (also in other code locations)
impl Subtable4<'_> {
    /// Returns a glyph index for a code point.
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
                        u16::read_at(self.glyph_id_array.as_ref(), usize::from(pos))
                            .ok()?;

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
    fn read(r: &mut Reader<'a>) -> crate::Result<Self> {
        r.skip(4)?; // format + length
        let language = r.read::<u16>()?;
        let seg_count_x2 = r.read::<u16>()?;

        if seg_count_x2 < 2 {
            return Err(InvalidData);
        }

        let seg_count = seg_count_x2 / 2;
        r.skip(6)?; // search range + entry selector + range shift
        let end_codes = r.read_vector::<u16>(seg_count as usize)?;
        r.skip(2)?; // reserved pad
        let start_codes = r.read_vector::<u16>(seg_count as usize)?;
        let id_deltas = r.read_vector::<i16>(seg_count as usize)?;

        let glyph_id_array = Cow::Borrowed(r.tail());
        let id_range_offsets = r.read_vector::<u16>(seg_count as usize)?;

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

    fn write(&self, w: &mut Writer) {
        w.write::<u16>(4); // version

        // (format + length + language + seg_count_x2 + search_range +
        // entry_selector + range_shift + reserved_pad) + seg_count *
        // (end_code + start_code + id_delta + id_range_offsets)
        let length = 2 * 8 + 2 * self.seg_count * 4;
        w.write::<u16>(length);
        w.write::<u16>(self.language);

        let seg_count_x2 = 2 * self.seg_count;
        let floor_log_2 = (u16::BITS - self.seg_count.leading_zeros()) - 1;
        let search_range = 2 * 2u16.pow(floor_log_2);
        let entry_selector = floor_log_2 as u16;
        let range_shift = seg_count_x2 - search_range;

        w.write::<u16>(seg_count_x2);
        w.write::<u16>(search_range);
        w.write::<u16>(entry_selector);
        w.write::<u16>(range_shift);

        w.write_vector(&self.end_codes);
        w.write::<u16>(0); // reserved pad
        w.write_vector(&self.start_codes);
        w.write_vector(&self.id_deltas);
        w.write_vector(&self.id_range_offsets);
    }
}

/// Create a subsetter version of a Subtable4.
pub(crate) fn subset_subtable4(ctx: &Context, data: &[u8]) -> crate::Result<Vec<u8>> {
    let subtable = Reader::new(data).read::<Subtable4>()?;

    let mut all_codepoints = vec![];
    subtable.codepoints(|c| all_codepoints.push(c as u16));

    // Create vector of pairs (c, g), where c is the codepoint and
    // g is the new gid.
    let mut char_to_new_gid_mappings = all_codepoints
        .into_iter()
        .filter_map(|c| {
            if let Some(g) = subtable.glyph_index(c as u32) {
                if ctx.requested_glyphs.contains(&g) {
                    if let Some(new_g) = ctx.mapper.forward.get(&g) {
                        return Some((c, *new_g));
                    }
                }
            }

            None
        })
        .collect::<Vec<_>>();

    char_to_new_gid_mappings.sort();

    let delta = |pair: (u16, u16)| (pair.1 as i32 - pair.0 as i32) as i16;

    let mut segments = vec![];
    let mut map_iter = char_to_new_gid_mappings.into_iter();

    if let Some(first) = map_iter.next() {
        let mut cur_start = first.0;
        let mut cur_delta = delta(first);
        let mut cur_range = 0;

        for next in map_iter {
            if next.0 == cur_start + cur_range + 1 && delta(next) == cur_delta {
                cur_range += 1;
                continue;
            }

            segments.push((cur_start, cur_start + cur_range, cur_delta));
            cur_start = next.0;
            cur_delta = delta(next);
            cur_range = 0;
        }

        // Don't forget the last range!
        segments.push((cur_start, cur_start + cur_range, cur_delta));
    }

    // "For the search to terminate, the final start code and endCode values must
    // be 0xFFFF. This segment need not contain any valid mappings. (It can just map the
    // single character code 0xFFFF to missingGlyph). However, the segment must be present."
    segments.push((0xFFFF, 0xFFFF, 1));

    let language = subtable.language;
    let seg_count = segments.len() as u16;
    let end_codes = segments.iter().map(|e| e.1).collect();
    let start_codes = segments.iter().map(|e| e.0).collect();
    let id_deltas = segments.iter().map(|e| e.2).collect();
    let id_range_offsets =
        [0].into_iter().cycle().take(seg_count as usize).collect::<Vec<_>>();
    let glyph_id_array = Cow::Owned(vec![]);

    let new_subtable = Subtable4 {
        language,
        seg_count,
        end_codes,
        start_codes,
        id_deltas,
        id_range_offsets,
        glyph_id_array,
    };

    let mut w = Writer::new();
    w.write::<Subtable4>(new_subtable);

    Ok(w.finish())
}
