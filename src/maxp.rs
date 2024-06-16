//! The `maxp` table contains the number of glyphs (and some additional information
//! depending on the version). All we need to do is rewrite the number of glyphs, the rest
//! can be copied from the old table.

use super::*;

pub fn subset(ctx: &mut Context) -> Result<()> {
    let maxp = ctx.expect_table(Tag::MAXP).ok_or(MalformedFont)?;
    let mut r = Reader::new(maxp);
    let version = r.read::<u32>().ok_or(MalformedFont)?;
    // number of glyphs
    r.read::<u16>().ok_or(MalformedFont)?;

    let mut sub_maxp = Writer::new();
    sub_maxp.write::<u32>(version);
    sub_maxp.write::<u16>(ctx.mapper.num_gids());

    if version == 0x00010000 {
        sub_maxp.extend(r.tail().ok_or(MalformedFont)?);
    }

    ctx.push(Tag::MAXP, sub_maxp.finish());
    Ok(())
}
