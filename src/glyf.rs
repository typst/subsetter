use std::collections::HashSet;

use super::*;

/// Subset the glyf and loca tables by removing glyph data for unused glyphs.
pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let head = ctx.expect_table(Tag::HEAD)?;
    let short = i16::read_at(head, 50)? == 0;
    if short {
        subset_impl::<u16>(ctx)
    } else {
        subset_impl::<u32>(ctx)
    }
}

fn subset_impl<T>(ctx: &mut Context) -> Result<()>
where
    T: LocaOffset,
{
    let loca = ctx.expect_table(Tag::LOCA)?;
    let glyf = ctx.expect_table(Tag::GLYF)?;
    let maxp = ctx.expect_table(Tag::MAXP)?;

    // Read data for a single glyph.
    let glyph_data = |id: u16| {
        let from = T::read_at(loca, usize::from(id) * T::SIZE)?;
        let to = T::read_at(loca, (usize::from(id) + 1) * T::SIZE)?;
        glyf.get(from.loca_to_usize() .. to.loca_to_usize())
            .ok_or(Error::InvalidOffset)
    };

    // The set of all glyphs we will include in the subset.
    let mut subset = HashSet::new();

    // Because glyphs may depend on other glyphs as components (also with
    // multiple layers of nesting), we have to process all glyphs to find
    // their components.
    let mut iter = ctx.profile.glyphs.iter().copied();
    let mut work = vec![0];

    // Find composite glyph descriptions.
    while let Some(id) = work.pop().or_else(|| iter.next()) {
        if subset.insert(id) {
            let mut r = Reader::new(glyph_data(id)?);
            if let Ok(num_contours) = r.read::<i16>() {
                // Negative means this is a composite glyph.
                if num_contours < 0 {
                    // Skip min/max metrics.
                    r.read::<i16>()?;
                    r.read::<i16>()?;
                    r.read::<i16>()?;
                    r.read::<i16>()?;

                    // Read component glyphs.
                    work.extend(component_glyphs(r));
                }
            }
        }
    }

    let mut sub_loca = Writer::new();
    let mut sub_glyf = Writer::new();

    let num_glyphs = u16::read_at(maxp, 4)?;
    for id in 0 .. num_glyphs {
        // If the glyph shouldn't be contained in the subset, it will
        // still get a loca entry, but the glyf data is simply empty.
        sub_loca.write(T::usize_to_loca(sub_glyf.len()));
        if subset.contains(&id) {
            sub_glyf.give(glyph_data(id)?);
        }
    }

    sub_loca.write(T::usize_to_loca(sub_glyf.len()));

    ctx.push(Tag::LOCA, sub_loca.finish());
    ctx.push(Tag::GLYF, sub_glyf.finish());

    Ok(())
}

/// Returns an iterator over the component glyphs referenced by the given
/// `glyf` table composite glyph description.
fn component_glyphs(mut r: Reader) -> impl Iterator<Item = u16> + '_ {
    const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
    const WE_HAVE_A_SCALE: u16 = 0x0008;
    const MORE_COMPONENTS: u16 = 0x0020;
    const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 0x0040;
    const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;

    let mut done = false;
    std::iter::from_fn(move || {
        if done {
            return None;
        }

        let flags = r.read::<u16>().ok()?;
        let component = r.read::<u16>().ok()?;

        if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            r.read::<i16>().ok()?;
            r.read::<i16>().ok()?;
        } else {
            r.read::<u16>().ok()?;
        }

        if flags & WE_HAVE_A_SCALE != 0 {
            r.read::<F2Dot14>().ok()?;
        } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
            r.read::<F2Dot14>().ok()?;
            r.read::<F2Dot14>().ok()?;
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            r.read::<F2Dot14>().ok()?;
            r.read::<F2Dot14>().ok()?;
            r.read::<F2Dot14>().ok()?;
            r.read::<F2Dot14>().ok()?;
        }

        done = flags & MORE_COMPONENTS == 0;
        Some(component)
    })
}

/// A loca offset, either 16-bit or 32-bit.
trait LocaOffset: Structure {
    fn loca_to_usize(self) -> usize;
    fn usize_to_loca(offset: usize) -> Self;
}

impl LocaOffset for u16 {
    fn loca_to_usize(self) -> usize {
        2 * usize::from(self)
    }

    fn usize_to_loca(offset: usize) -> Self {
        // Shouldn't overflow since all offsets were u16 before and
        // we are only shortening the table.
        (offset / 2) as u16
    }
}

impl LocaOffset for u32 {
    fn loca_to_usize(self) -> usize {
        self as usize
    }

    fn usize_to_loca(offset: usize) -> Self {
        offset as u32
    }
}
