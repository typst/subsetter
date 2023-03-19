use super::*;

/// A glyf + loca table.
struct Table<'a> {
    loca: &'a [u8],
    glyf: &'a [u8],
    long: bool,
}

impl<'a> Table<'a> {
    fn new(ctx: &Context<'a>) -> Result<Self> {
        let loca = ctx.expect_table(Tag::LOCA)?;
        let glyf = ctx.expect_table(Tag::GLYF)?;
        let head = ctx.expect_table(Tag::HEAD)?;
        let long = i16::read_at(head, 50)? != 0;
        Ok(Self { loca, glyf, long })
    }

    fn glyph_data(&self, id: u16) -> Result<&'a [u8]> {
        let read_offset = |n| {
            Ok(if self.long {
                u32::read_at(self.loca, 4 * n)? as usize
            } else {
                u16::read_at(self.loca, 2 * n)? as usize * 2
            })
        };

        let from = read_offset(id as usize)?;
        let to = read_offset(id as usize + 1)?;
        self.glyf.get(from..to).ok_or(Error::InvalidOffset)
    }
}

/// Find all glyphs referenced through components.
pub(crate) fn discover(ctx: &mut Context) -> Result<()> {
    let table = Table::new(ctx)?;

    // Because glyphs may depend on other glyphs as components (also with
    // multiple layers of nesting), we have to process all glyphs to find
    // their components.
    let mut iter = ctx.profile.glyphs.iter().copied();
    let mut work = vec![0];

    // Find composite glyph descriptions.
    while let Some(id) = work.pop().or_else(|| iter.next()) {
        if ctx.subset.insert(id) {
            let mut r = Reader::new(table.glyph_data(id)?);
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

    // Compute combined size of all glyphs to select loca format.
    let mut size = 0;
    for &id in &ctx.subset {
        let mut len = table.glyph_data(id)?.len();
        len += (len % 2 != 0) as usize;
        size += len;
    }

    ctx.long_loca = size > 2 * (u16::MAX as usize);

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

/// Subset the glyf and loca tables by removing glyph data for unused glyphs.
pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let table = Table::new(ctx)?;

    let mut sub_glyf = Writer::new();
    let mut sub_loca = Writer::new();
    let mut write_offset = |offset: usize| {
        if ctx.long_loca {
            sub_loca.write::<u32>(offset as u32);
        } else {
            sub_loca.write::<u16>((offset / 2) as u16);
        }
    };

    for id in 0..ctx.num_glyphs {
        // If the glyph shouldn't be contained in the subset, it will
        // still get a loca entry, but the glyf data is simply empty.
        write_offset(sub_glyf.len());
        if ctx.subset.contains(&id) {
            let data = table.glyph_data(id)?;
            sub_glyf.give(data);
            if !ctx.long_loca {
                sub_glyf.align(2);
            }
        }
    }

    write_offset(sub_glyf.len());

    ctx.push(Tag::LOCA, sub_loca.finish());
    ctx.push(Tag::GLYF, sub_glyf.finish());

    Ok(())
}
