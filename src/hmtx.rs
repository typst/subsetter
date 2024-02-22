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

    let mut hmtx = ctx.expect_table(Tag::HMTX)?.to_vec();

    let mut offset = 0;
    for i in 0..num_h_metrics {
        if !ctx.subset.contains(&i) {
            hmtx.get_mut(offset..offset + 4).ok_or(Error::MissingData)?.fill(0);
        }
        offset += 4;
    }

    for i in num_h_metrics..ctx.num_glyphs {
        if !ctx.subset.contains(&i) {
            hmtx.get_mut(offset..offset + 2).ok_or(Error::MissingData)?.fill(0);
        }
        offset += 2;
    }

    ctx.push(Tag::HMTX, hmtx);

    Ok(())
}
