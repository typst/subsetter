use crate::stream::{Reader, StringId};
use std::collections::HashMap;

fn parse_format0(r: &mut Reader, num_codes: u8) -> Option<HashMap<u8, u16>> {
    Some(
        r.read_vector::<u8>(num_codes as usize)
            .ok()?
            .into_iter()
            .enumerate()
            .map(|(code, gid)| (code as u8, u16::from(gid) + 1))
            .collect(),
    )
}

fn parse_format1(r: &mut Reader, num_ranges: u8) -> Option<HashMap<u8, u16>> {
    let mut map = HashMap::new();

    let mut gid = 1;
    for _ in 0..num_ranges {
        let first = r.read::<u8>().ok()?;
        let n_left = r.read::<u8>().ok()?;

        for i in first..=first + n_left {
            map.insert(i, gid);
            gid += 1;
        }
    }

    Some(map)
}

fn parse_supplemental(r: &mut Reader, n_sups: u8) -> Option<Vec<(u8, StringId)>> {
    let mut entries = vec![];

    for _ in 0..n_sups {
        let code = r.read::<u8>().ok()?;
        let sid = r.read::<StringId>().ok()?;

        entries.push((code, sid));
    }

    Some(entries)
}

pub(crate) fn parse_encoding(
    r: &mut Reader,
) -> Option<(HashMap<u8, u16>, Vec<(u8, StringId)>)> {
    let format = r.read::<u8>().ok()?;
    // The first high-bit in format indicates that a Supplemental encoding is present.
    // Check it and clear.
    let has_supplemental = format & 0x80 != 0;
    let format = format & 0x7f;

    let count = r.read::<u8>().ok()?;
    // println!("count: {}", count);
    let encoding = match format {
        // TODO: read_array8?
        0 => parse_format0(r, count),
        1 => parse_format1(r, count),
        _ => unreachable!(),
    }?;

    let supplemental = if has_supplemental {
        let n_sups = r.read::<u8>().ok()?;
        parse_supplemental(r, n_sups)
    } else {
        Some(vec![])
    }?;

    Some((encoding, supplemental))
}
