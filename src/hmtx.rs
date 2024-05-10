use super::*;
use crate::Error::{MalformedFont, SubsetError};

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let hmtx = ctx.expect_table(Tag::HMTX).ok_or(MalformedFont)?;

    let get_metrics = |new_gid: u16| {
        let num_h_metrics = {
            let hhea = ctx.expect_table(Tag::HHEA).ok_or(MalformedFont)?;
            let mut r = Reader::new(hhea);
            r.skip_bytes(34);
            r.read::<u16>().ok_or(MalformedFont)?
        };

        let old_gid = ctx.mapper.get_reverse(new_gid).ok_or(SubsetError)?;
        let last_advance_width = {
            let index = 4 * num_h_metrics.checked_sub(1).ok_or(MalformedFont)? as usize;
            let mut r = Reader::new(hmtx.get(index..).ok_or(MalformedFont)?);
            r.read::<u16>().ok_or(MalformedFont)?
        };

        let has_advance_width = old_gid < num_h_metrics;
        let offset = if has_advance_width {
            old_gid as usize * 4
        } else {
            (num_h_metrics * 4 + (old_gid - num_h_metrics) * 2) as usize
        };

        let mut r = Reader::new(hmtx.get(offset..).ok_or(MalformedFont)?);

        if has_advance_width {
            let adv = r.read::<u16>().ok_or(MalformedFont)?;
            let lsb = r.read::<u16>().ok_or(MalformedFont)?;
            Ok((adv, lsb))
        } else {
            Ok((last_advance_width, r.read::<u16>().ok_or(MalformedFont)?))
        }
    };

    let mut last_advance_width_index = ctx.mapper.num_gids() - 1;
    let last_advance_width = get_metrics(last_advance_width_index)?.0;

    for gid in (0..last_advance_width_index).rev() {
        if get_metrics(gid)?.0 == last_advance_width {
            last_advance_width_index = gid;
        } else {
            break;
        }
    }

    let mut sub_hmtx = Writer::new();

    for gid in 0..ctx.mapper.num_gids() {
        let metrics = get_metrics(gid)?;

        if gid <= last_advance_width_index {
            sub_hmtx.write::<u16>(metrics.0);
        }

        sub_hmtx.write::<u16>(metrics.1);
    }

    ctx.push(Tag::HMTX, sub_hmtx.finish());

    let hhea = ctx.expect_table(Tag::HHEA).ok_or(MalformedFont)?;
    let mut sub_hhea = Writer::new();
    sub_hhea.extend(&hhea[0..hhea.len() - 2]);
    sub_hhea.write::<u16>(last_advance_width_index + 1);

    ctx.push(Tag::HHEA, sub_hhea.finish());

    Ok(())
}
