mod index;

use super::*;
use crate::cff::index::{parse_index, Index};

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let cff = ctx.expect_table(Tag::CFF)?;

    let mut r = Reader::new(cff);

    // Parse Header.
    let major = r.read::<u8>()?;
    r.skip::<u8>()?; // minor
    let header_size = r.read::<u8>()?;
    r.skip::<u8>()?; // Absolute offset

    if major != 1 {
        return Err(Error::Unimplemented);
    }

    // Jump to Name INDEX. It's not necessarily right after the header.
    if header_size > 4 {
        r.advance(usize::from(header_size) - 4)?;
    }

    let name_index = parse_index::<u16>(&mut r)?;
    Ok(())
}

pub(crate) fn discover(ctx: &mut Context) -> Result<()> {
    ctx.subset.insert(0);
    ctx.subset
        .extend(ctx.requested_glyphs.iter().filter(|g| **g < ctx.num_glyphs));
    Ok(())
}
