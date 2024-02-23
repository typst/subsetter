use super::*;

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let maxp = ctx.expect_table(Tag::MAXP)?;
    let mut r = Reader::new(maxp);
    let version = r.read::<u32>()?;
    r.read::<u16>()?; // num glyphs

    let mut sub_maxp = Writer::new();
    sub_maxp.write::<u32>(version);
    sub_maxp.write::<u16>(ctx.subset.len() as u16);

    if version == 0x00010000 {
        sub_maxp.give(r.data());
    }

    ctx.push(Tag::MAXP, sub_maxp.finish());
    Ok(())
}
