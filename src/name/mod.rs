mod read;
mod subset;

use super::*;
use crate::name::read::Version0Table;
use crate::Error::{MalformedFont, SubsetError};

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let name = ctx.expect_table(Tag::NAME).ok_or(MalformedFont)?;
    // println!("{:?}", name);
    let mut r = Reader::new(name);

    let version = r.read::<u16>().ok_or(MalformedFont)?;

    // From my personal experiments, version 1 is isn't used at all, so we
    // don't bother subsetting it.
    if version != 0 {
        ctx.push(Tag::NAME, name);
        return Ok(());
    }

    let table = Version0Table::parse(name).ok_or(MalformedFont)?;
    let subsetted_table = subset::subset(&table).ok_or(SubsetError)?;

    let mut w = Writer::new();
    w.write(subsetted_table);

    ctx.push(Tag::NAME, w.finish());
    Ok(())
}
