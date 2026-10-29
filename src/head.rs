//! The `head` table mostly contains information that can be reused from the
//! old table, except for the `loca` format, which depends on the size of the
//! glyph data, and the bounds which can change when instancing. The checksum
//! will be recalculated in the very end.

use super::*;

pub fn subset(ctx: &mut Context) -> Result<()> {
    let mut head = ctx.expect_table(Tag::HEAD).ok_or(MalformedFont)?.to_vec();
    let index_to_loc = head.get_mut(50..52).ok_or(MalformedFont)?;
    index_to_loc[0] = 0;
    index_to_loc[1] = ctx.long_loca as u8;

    // We are (potentially) instancing a variable font, recalculate the font bounds.
    if ctx.interjector.is_skrifa() {
        let [x_min, y_min, x_max, y_max] = ctx.font_bbox.unwrap_or_default();
        let bbox = head.get_mut(36..44).ok_or(MalformedFont)?;
        bbox[0..2].copy_from_slice(&x_min.to_be_bytes());
        bbox[2..4].copy_from_slice(&y_min.to_be_bytes());
        bbox[4..6].copy_from_slice(&x_max.to_be_bytes());
        bbox[6..8].copy_from_slice(&y_max.to_be_bytes());
    }

    ctx.push(Tag::HEAD, head);
    Ok(())
}
