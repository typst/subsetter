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

    let mut last_advance_width = 0;
    for i in 0..ctx.subset.len() {
        let original_gid = ctx.reverse_gid_map[i];

        if original_gid < num_h_metrics {
            let offset = original_gid as usize * 4;
            let mut r = Reader::new(&hmtx[offset..]);
            let advance_width = r.read::<u16>()?;
            let lsb = r.read::<u16>()?;
            sub_htmx.write::<u16>(advance_width);
            sub_htmx.write::<u16>(lsb);

            last_advance_width = advance_width;
        } else {
            let metrics_end = num_h_metrics as usize * 4;
            let offset = metrics_end + (original_gid - num_h_metrics) as usize * 2;
            let mut r = Reader::new(&hmtx[offset..]);

            let lsb = r.read::<u16>()?;
            sub_htmx.write::<u16>(last_advance_width);
            sub_htmx.write::<u16>(lsb);
        }
    }

    ctx.push(Tag::HMTX, sub_htmx.finish());

    Ok(())
}
