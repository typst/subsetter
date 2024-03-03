use super::*;
use crate::Error::MalformedFont;

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let hhea = ctx.expect_table(Tag::HHEA).ok_or(MalformedFont)?;
    let mut sub_hhea = Writer::new();
    sub_hhea.extend(&hhea[0..hhea.len() - 2]);
    sub_hhea.write::<u16>(ctx.subset.len() as u16);

    ctx.push(Tag::HHEA, sub_hhea.finish());
    Ok(())
}
