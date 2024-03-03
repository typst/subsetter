use super::*;
use crate::Error::MalformedFont;

/// Subset the head table.
///
/// Updates the loca format.
pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let mut head = ctx.expect_table(Tag::HEAD).ok_or(MalformedFont)?.to_vec();
    let index_to_loc = head.get_mut(50..52).ok_or(MalformedFont)?;
    index_to_loc[0] = 0;
    index_to_loc[1] = ctx.long_loca as u8;
    ctx.push(Tag::HEAD, head);
    Ok(())
}
