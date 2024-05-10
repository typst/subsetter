use super::*;
use crate::Error::MalformedFont;

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let maxp = ctx.expect_table(Tag::MAXP).ok_or(MalformedFont)?;
    let mut r = Reader::new(maxp);
    let version = r.read::<u32>().ok_or(MalformedFont)?;
    r.read::<u16>().ok_or(MalformedFont)?; // num glyphs

    let mut sub_maxp = Writer::new();
    sub_maxp.write::<u32>(version);
    sub_maxp.write::<u16>(ctx.mapper.num_gids());

    if version == 0x00010000 {
        sub_maxp.extend(r.tail().ok_or(MalformedFont)?);
    }

    ctx.push(Tag::MAXP, sub_maxp.finish());
    Ok(())
}
