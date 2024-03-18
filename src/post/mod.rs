mod read;
mod subset;

use super::*;
use crate::post::read::Version2Table;
use crate::Error::{MalformedFont, SubsetError};

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let post = ctx.expect_table(Tag::POST).ok_or(MalformedFont)?;
    let mut r = Reader::new(post);

    // Version 2 is the only one worth subsetting.
    let version = r.read::<u32>().ok_or(MalformedFont)?;
    if version != 0x00020000 {
        ctx.push(Tag::POST, post);
        return Ok(());
    }

    let table = Version2Table::parse(post).ok_or(MalformedFont)?;
    let subsetted_table = subset::subset(ctx, &table).ok_or(SubsetError)?;

    let mut w = Writer::new();
    w.write(subsetted_table);

    ctx.push(Tag::POST, w.finish());
    Ok(())
}
