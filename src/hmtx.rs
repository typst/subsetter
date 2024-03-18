use super::*;
use crate::Error::{MalformedFont, SubsetError};

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let num_h_metrics = {
        let hhea = ctx.expect_table(Tag::HHEA).ok_or(MalformedFont)?;
        let mut r = Reader::new(hhea);
        r.skip_bytes(34);
        r.read::<u16>().ok_or(MalformedFont)?
    };

    let hmtx = ctx.expect_table(Tag::HMTX).ok_or(MalformedFont)?;

    let mut hmtx_map = HashMap::with_capacity(ctx.num_glyphs as usize);

    let mut last_advance_width = 0;
    for gid in 0..ctx.num_glyphs {
        if gid < num_h_metrics {
            let offset = gid as usize * 4;
            let mut r = Reader::new(&hmtx[offset..]);
            let advance_width = r.read::<u16>().ok_or(MalformedFont)?;
            let lsb = r.read::<u16>().ok_or(MalformedFont)?;

            hmtx_map.insert(gid, (advance_width, lsb));

            last_advance_width = advance_width;
        } else {
            let metrics_end = num_h_metrics as usize * 4;
            let offset = metrics_end + (gid - num_h_metrics) as usize * 2;
            let mut r = Reader::new(&hmtx[offset..]);

            let lsb = r.read::<u16>().ok_or(MalformedFont)?;

            hmtx_map.insert(gid, (last_advance_width, lsb));
        }
    }

    let mut advanced_widths = Vec::new();
    let mut left_side_bearings = Vec::new();

    for gid in 0..ctx.subset.len() as u16 {
        let original_gid = ctx.mapper.get_reverse(gid).ok_or(SubsetError)?;
        let entry = hmtx_map.get(&original_gid).ok_or(SubsetError)?;
        advanced_widths.push(entry.0);
        left_side_bearings.push(entry.1);
    }

    let mut actual_length = None;

    if let Some(last) = advanced_widths.last() {
        for (i, lsb) in advanced_widths.iter().rev().enumerate() {
            if i == 1 && last != lsb {
                break;
            }

            if last != lsb {
                actual_length = Some(advanced_widths.len() - i + 1);
                break;
            }

            if i == advanced_widths.len() - 1 {
                // All glyphs have the same width
                actual_length = Some(1);
            }
        }
    }

    if let Some(actual_length) = actual_length {
        advanced_widths.truncate(actual_length);
    }

    let mut sub_hmtx = Writer::new();

    for (i, lsb) in left_side_bearings.iter().enumerate() {
        if let Some(advance_width) = advanced_widths.get(i) {
            sub_hmtx.write::<u16>(*advance_width);
        }

        sub_hmtx.write::<u16>(*lsb);
    }

    ctx.push(Tag::HMTX, sub_hmtx.finish());

    let hhea = ctx.expect_table(Tag::HHEA).ok_or(MalformedFont)?;
    let mut sub_hhea = Writer::new();
    sub_hhea.extend(&hhea[0..hhea.len() - 2]);
    sub_hhea.write::<u16>(advanced_widths.len() as u16);

    ctx.push(Tag::HHEA, sub_hhea.finish());

    Ok(())
}
