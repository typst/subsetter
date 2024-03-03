use std::collections::HashMap;
use crate::stream::{Reader, StringId};

/// Enumerates Charset IDs defined in the Adobe Technical Note #5176, Table 22
pub mod charset_id {
    pub const ISO_ADOBE: usize = 0;
    pub const EXPERT: usize = 1;
    pub const EXPERT_SUBSET: usize = 2;
}

#[derive(Clone, Debug)]
pub(crate) enum Charset {
    ISOAdobe,
    Expert,
    ExpertSubset,
    DoubleByte(HashMap<StringId, u16>)
}

fn count_format_2(r: &Reader, number_of_glyphs: u16) -> Option<u16> {
    let mut count = 0;

    {
        let mut r= r.clone();
        let mut total_left = number_of_glyphs - 1;
        while total_left > 0 {
            r.skip::<StringId>().ok()?; // first
            let left = r.read::<u16>().ok()?;
            total_left = total_left.checked_sub(u16::from(left) + 1)?;
            count += 1;
        }

        Some(count)
    }
}

fn parse_format_2(r: &mut Reader, num_ranges: u16) -> Option<HashMap<StringId, u16>> {
    let mut map = HashMap::new();

    let mut gid = 1;
    for _ in 0..num_ranges {
        let first = r.read::<StringId>().ok()?;
        let n_left = r.read::<u16>().ok()?;

        for i in first.0..=first.0 + n_left as u16 {
            map.insert(StringId(i), gid);
            gid += 1;
        }
    }

    Some(map)
}

pub(crate) fn parse_charset(r: &mut Reader, number_of_glyphs: u16) -> Option<Charset> {
    let format = r.read::<u8>().ok()?;

    match format {
        2 => {
            let count = count_format_2(r, number_of_glyphs)?;
            let result = Some(Charset::DoubleByte(parse_format_2(r, count)?));
            return result;
        },
        _ => {}
    }

    None
}