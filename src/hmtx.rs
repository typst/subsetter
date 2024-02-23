use super::*;

/// Subset the htmx table.
///
/// We can't change anything about its size, but we can zero out all metrics
/// for unused glyphs so that it compresses better when embedded into a PDF.
pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let num_h_metrics = {
        let hhea = ctx.expect_table(Tag::HHEA)?;
        let mut r = Reader::new(hhea);
        r.skip(34)?;
        r.read::<u16>()?
    };

    let hmtx = ctx.expect_table(Tag::HMTX)?;
    let mut sub_htmx = Writer::new();

    for i in 0..ctx.subset.len() {
        let original_gid = ctx.reverse_gid_map[i];

        if original_gid < num_h_metrics {
            let mut r = Reader::new(&hmtx[(original_gid as usize * 4)..]);
            let advance_width = r.read::<u16>()?;
            let lsb = r.read::<u16>()?;
            sub_htmx.write::<u16>(advance_width);
            sub_htmx.write::<u16>(lsb);
        } else {
            return Err(Error::Unimplemented);
        }
    }

    ctx.push(Tag::HMTX, sub_htmx.finish());

    Ok(())
}
