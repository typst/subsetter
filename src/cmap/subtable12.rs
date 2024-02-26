use crate::stream::{Reader, Structure, Writer};
use crate::Context;

struct SequentialMapGroupRecord {
    start_char_code: u32,
    end_char_code: u32,
    start_glyph_id: u32,
}

impl Structure<'_> for SequentialMapGroupRecord {
    fn read(r: &mut Reader<'_>) -> crate::Result<Self> {
        let start_char_code = r.read::<u32>()?;
        let end_char_code = r.read::<u32>()?;
        let start_glyph_id = r.read::<u32>()?;

        Ok(Self { start_char_code, end_char_code, start_glyph_id })
    }

    fn write(&self, w: &mut Writer) {
        w.write::<u32>(self.start_char_code);
        w.write::<u32>(self.end_char_code);
        w.write::<u32>(self.start_glyph_id);
    }
}

/// A format 12 subtable.
pub(crate) struct Subtable12 {
    language: u32,
    groups: Vec<SequentialMapGroupRecord>,
}

impl Subtable12 {
    /// Returns a glyph index for a code point.
    fn glyph_index(&self, code_point: u32) -> Option<u16> {
        let index = self
            .groups
            .binary_search_by(|range| {
                use core::cmp::Ordering;

                if range.start_char_code > code_point {
                    Ordering::Greater
                } else if range.end_char_code < code_point {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            })
            .ok()?;

        let group = &self.groups[index];

        let id = group
            .start_glyph_id
            .checked_add(code_point)?
            .checked_sub(group.start_char_code)?;
        u16::try_from(id).ok()
    }

    /// Calls `f` for each codepoint defined in this table.
    fn codepoints(&self, mut f: impl FnMut(u32)) {
        for group in &self.groups {
            for code_point in group.start_char_code..=group.end_char_code {
                f(code_point);
            }
        }
    }
}

impl Structure<'_> for Subtable12 {
    fn read(r: &mut Reader<'_>) -> crate::Result<Self> {
        r.read::<u16>()?; // format
        r.read::<u16>()?; // reserved
        r.read::<u32>()?; // length
        let language = r.read::<u32>()?;
        let num_groups = r.read::<u32>()?;

        let groups = r.read_vector::<SequentialMapGroupRecord>(num_groups as usize)?;
        Ok(Self { language, groups })
    }

    fn write(&self, w: &mut Writer) {
        // format + reserved + length + language + num_groups + num_groups * (start_char,
        // end_char, start_glyph_id)
        let length = 2 + 2 + 4 + 4 + 4 + (4 + 4 + 4) * self.groups.len();

        w.write::<u16>(12);
        w.write::<u16>(0);
        w.write::<u32>(length as u32);
        w.write::<u32>(self.language);
        w.write::<u32>(self.groups.len() as u32);

        for group in &self.groups {
            group.write(w);
        }
    }
}

pub(crate) fn subset_subtable12(ctx: &Context, data: &[u8]) -> crate::Result<Vec<u8>> {
    let subtable = Subtable12::read_at(data, 0)?;
    let mut all_codepoints = vec![];
    subtable.codepoints(|c| all_codepoints.push(c));

    let mut new_mappings = all_codepoints
        .into_iter()
        .filter_map(|c| {
            if let Some(g) = subtable.glyph_index(c) {
                if ctx.requested_glyphs.contains(&g) {
                    if let Some(new_g) = ctx.mapper.forward.get(&g) {
                        return Some((c, *new_g));
                    }
                }
            }

            None
        })
        .collect::<Vec<_>>();

    new_mappings.sort();
    let mut map_iter = new_mappings.into_iter();

    let mut new_groups = vec![];

    if let Some(first) = map_iter.next() {
        let mut cur_start = first.0;
        let mut cur_gid = first.1;
        let mut cur_range = 0;

        for next in map_iter {
            if next.0 == cur_start + cur_range + 1
                && next.1 as u32 == cur_gid as u32 + cur_range + 1
            {
                cur_range += 1;
                continue;
            }

            new_groups.push(SequentialMapGroupRecord {
                start_char_code: cur_start,
                end_char_code: cur_start + cur_range,
                start_glyph_id: cur_gid as u32,
            });

            cur_start = next.0;
            cur_gid = next.1;
            cur_range = 0;
        }

        new_groups.push(SequentialMapGroupRecord {
            start_char_code: cur_start,
            end_char_code: cur_start + cur_range,
            start_glyph_id: cur_gid as u32,
        });
    }

    let new_subtable = Subtable12 { language: subtable.language, groups: new_groups };

    let mut w = Writer::new();
    w.write::<Subtable12>(new_subtable);

    Ok(w.finish())
}
