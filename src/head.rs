use super::*;

/// Subset the head table.
///
/// Updates the loca format.
pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let mut head = ctx.expect_table(Tag::HEAD)?.to_vec();
    let index_to_loc = head.get_mut(50..52).ok_or(Error::InvalidOffset)?;
    index_to_loc[0] = 0;
    index_to_loc[1] = ctx.long_loca as u8;
    ctx.push(Tag::HEAD, head);
    Ok(())
}
