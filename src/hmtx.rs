//! The `hmtx` table contains the horizontal metrics for each glyph.
//! All we need to do is to rewrite the table so that it matches the
//! sequence of the new glyphs. A minor pain point is that the table
//! allows omitting the advance width for the last few glyphs if it is
//! the same. In order to keep the code simple, we do not keep this optimization
//! when rewriting the table.
//! While doing so, we also rewrite the `hhea` table, which contains
//! the number of glyphs that contain both, advance width and
//! left side bearing metrics.

// The parsing logic was taken from ttf-parser.

use super::*;
use crate::Error::OverflowError;

pub fn subset(ctx: &mut Context) -> Result<()> {
    let num_glyphs = ctx.font_data.glyph_data.len();
    let mut sub_hmtx = Writer::new();

    for glyph_data in &ctx.font_data.glyph_data {
        sub_hmtx.write::<u16>(glyph_data.advance_width);
        sub_hmtx.write::<i16>(glyph_data.lsb);
    }

    ctx.push(Tag::HMTX, sub_hmtx.finish());

    let hhea = ctx.expect_table(Tag::HHEA).ok_or(MalformedFont)?;
    let mut sub_hhea = Writer::new();
    sub_hhea.extend(&hhea[0..hhea.len() - 2]);
    sub_hhea.write::<u16>(num_glyphs as u16);

    ctx.push(Tag::HHEA, sub_hhea.finish());

    Ok(())
}
