//! The `glyf` table contains the main description of the glyphs. In order to
//! subset it, there are 5 things we need to do:
//! 1. We need to form the glyph closure. Glyphs can reference other glyphs, meaning that
//!    if a user for example requests the glyph 1, and this glyph references the glyph 2, then
//!    we need to include both of them in our subset.
//! 2. We need to remove glyph descriptions that are not needed for the subset, and reorder
//!    the existing glyph descriptions to match the order defined by the remapper.
//! 3. For component glyphs, we need to rewrite their description so that they reference
//!    the new glyph ID of the glyphs they reference.
//! 4. We need to calculate which format to use in the `loca` table.
//! 5. We need to update the `loca` table itself with the new offsets.

use super::*;
use write_fonts::tables::glyf::SimpleGlyph;
use write_fonts::FontWrite;
use write_fonts::{dump_table, TableWriter};

pub fn subset(ctx: &mut Context) -> Result<()> {
    let subsetted_entries = subset_glyf_entries(ctx)?;

    let mut sub_glyf = Writer::new();
    let mut sub_loca = Writer::new();

    let mut write_offset = |offset: usize| {
        if ctx.long_loca {
            sub_loca.write::<u32>(offset as u32);
        } else {
            sub_loca.write::<u16>((offset / 2) as u16);
        }
    };

    for entry in &subsetted_entries {
        write_offset(sub_glyf.len());
        sub_glyf.extend(entry);

        if !ctx.long_loca {
            sub_glyf.align(2);
        }
    }

    // Write the final offset.
    write_offset(sub_glyf.len());

    ctx.push(Tag::LOCA, sub_loca.finish());
    ctx.push(Tag::GLYF, sub_glyf.finish());

    Ok(())
}

/// A glyf + loca table.
struct Table<'a> {
    loca: &'a [u8],
    glyf: &'a [u8],
    long: bool,
}

fn subset_glyf_entries(ctx: &mut Context) -> Result<Vec<Vec<u8>>> {
    let mut size = 0;
    let mut glyf_entries = vec![];
    let mut maxp_data = MaxpData::default();

    for glyph_data in &ctx.font_data.glyph_data {
        let written_glyph = {
            let simple_glyph =
                SimpleGlyph::from_bezpath(&glyph_data.path).map_err(|_| MalformedFont)?;

            maxp_data.max_points = maxp_data
                .max_points
                .max(simple_glyph.contours.iter().map(|c| c.len() as u16).sum());
            maxp_data.max_contours =
                maxp_data.max_contours.max(simple_glyph.contours.len() as u16);

            let mut writer = TableWriter::default();
            simple_glyph.write_into(&mut writer);

            dump_table(&simple_glyph).map_err(|_| MalformedFont)?
        };

        let mut len = written_glyph.len();
        len += (len % 2 != 0) as usize;
        size += len;

        glyf_entries.push(written_glyph);
    }

    ctx.long_loca = size > 2 * (u16::MAX as usize);
    ctx.custom_maxp_data = Some(maxp_data);

    Ok(glyf_entries)
}
