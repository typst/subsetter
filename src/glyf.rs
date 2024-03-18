use super::*;
use crate::Error::{MalformedFont, SubsetError};

/// A glyf + loca table.
struct Table<'a> {
    loca: &'a [u8],
    glyf: &'a [u8],
    long: bool,
}

impl<'a> Table<'a> {
    fn new(ctx: &Context<'a>) -> Option<Self> {
        let loca = ctx.expect_table(Tag::LOCA)?;
        let glyf = ctx.expect_table(Tag::GLYF)?;
        let head = ctx.expect_table(Tag::HEAD)?;

        let mut r = Reader::new_at(head, 50);
        let long = r.read::<i16>()? != 0;
        Some(Self { loca, glyf, long })
    }

    fn glyph_data(&self, id: u16) -> Option<&'a [u8]> {
        let read_offset = |n| {
            Some(if self.long {
                let mut r = Reader::new_at(self.loca, 4 * n);
                r.read::<u32>()? as usize
            } else {
                let mut r = Reader::new_at(self.loca, 2 * n);
                2 * r.read::<u16>()? as usize
            })
        };

        let from = read_offset(id as usize)?;
        let to = read_offset(id as usize + 1)?;
        self.glyf.get(from..to)
    }
}

/// Find all glyphs referenced through components.
pub(crate) fn discover(ctx: &mut Context) -> Result<()> {
    let table = Table::new(ctx).ok_or(MalformedFont)?;

    // Because glyphs may depend on other glyphs as components (also with
    // multiple layers of nesting), we have to process all glyphs to find
    // their components.
    let mut glyph_vec = ctx.requested_glyphs.iter().copied().collect::<Vec<_>>();
    glyph_vec.sort();
    let mut iter = glyph_vec.into_iter();
    let mut work = vec![0];

    // Find composite glyph descriptions.
    while let Some(id) = work.pop().or_else(|| iter.next()) {
        if id < ctx.num_glyphs && ctx.subset.insert(id) {
            let mut r = Reader::new(table.glyph_data(id).ok_or(MalformedFont)?);
            if let Some(num_contours) = r.read::<i16>() {
                // Negative means this is a composite glyph.
                if num_contours < 0 {
                    // Skip min/max metrics.
                    r.read::<i16>().ok_or(MalformedFont)?;
                    r.read::<i16>().ok_or(MalformedFont)?;
                    r.read::<i16>().ok_or(MalformedFont)?;
                    r.read::<i16>().ok_or(MalformedFont)?;

                    let extended = component_glyphs(r).collect::<Vec<_>>();
                    work.extend(extended);
                }
            }
        }
    }

    // Compute combined size of all glyphs to select loca format.
    let mut size = 0;
    for &id in &ctx.subset {
        let mut len = table.glyph_data(id).ok_or(MalformedFont)?.len();
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

        let flags = r.read::<u16>()?;
        let component = r.read::<u16>()?;

        if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            r.read::<i16>()?;
            r.read::<i16>()?;
        } else {
            r.read::<u16>()?;
        }

        if flags & WE_HAVE_A_SCALE != 0 {
            r.read::<F2Dot14>()?;
        } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
            r.read::<F2Dot14>()?;
            r.read::<F2Dot14>()?;
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            r.read::<F2Dot14>()?;
            r.read::<F2Dot14>()?;
            r.read::<F2Dot14>()?;
            r.read::<F2Dot14>()?;
        }

        done = flags & MORE_COMPONENTS == 0;
        Some(component)
    })
}

/// Subset the glyf and loca tables by removing glyph data for unused glyphs.
pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let table = Table::new(ctx).ok_or(MalformedFont)?;

    let mut sub_glyf = Writer::new();
    let mut sub_loca = Writer::new();
    let mut write_offset = |offset: usize| {
        if ctx.long_loca {
            sub_loca.write::<u32>(offset as u32);
        } else {
            sub_loca.write::<u16>((offset / 2) as u16);
        }
    };

    for gid in 0..ctx.mapper.num_gids() {
        let old_gid = ctx.mapper.get_reverse(gid).ok_or(SubsetError)?;
        write_offset(sub_glyf.len());
        let data = table.glyph_data(old_gid).ok_or(MalformedFont)?;

        // Not contours
        if data.is_empty() {
            continue;
        }

        let mut r = Reader::new(data);

        if r.read::<i16>().ok_or(MalformedFont)? < 0 {
            sub_glyf.extend(&remap_component_glyphs(ctx, data)?);
        } else {
            sub_glyf.extend(data);
        }

        if !ctx.long_loca {
            sub_glyf.align(2);
        }
    }

    write_offset(sub_glyf.len());

    ctx.push(Tag::LOCA, sub_loca.finish());
    ctx.push(Tag::GLYF, sub_glyf.finish());

    Ok(())
}

fn remap_component_glyphs(ctx: &Context, data: &[u8]) -> Result<Vec<u8>> {
    let mut r = Reader::new(data);
    let mut w = Writer::with_capacity(data.len());

    // num_contours
    w.write(r.read::<i16>().ok_or(MalformedFont)?);

    //x,y min/max
    w.write(r.read::<i16>().ok_or(MalformedFont)?);
    w.write(r.read::<i16>().ok_or(MalformedFont)?);
    w.write(r.read::<i16>().ok_or(MalformedFont)?);
    w.write(r.read::<i16>().ok_or(MalformedFont)?);

    const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
    const WE_HAVE_A_SCALE: u16 = 0x0008;
    const MORE_COMPONENTS: u16 = 0x0020;
    const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 0x0040;
    const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;

    let mut done;

    loop {
        let flags = r.read::<u16>().ok_or(MalformedFont)?;
        w.write(flags);
        let component = r.read::<u16>().ok_or(MalformedFont)?;
        let new_component = ctx.mapper.get(component).ok_or(SubsetError)?;
        w.write(new_component);

        if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            w.write(r.read::<i16>().ok_or(MalformedFont)?);
            w.write(r.read::<i16>().ok_or(MalformedFont)?);
        } else {
            w.write(r.read::<u16>().ok_or(MalformedFont)?);
        }

        if flags & WE_HAVE_A_SCALE != 0 {
            w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
        } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
            w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
            w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
            w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
            w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
            w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
        }

        done = flags & MORE_COMPONENTS == 0;

        if done {
            break;
        }
    }

    Ok(w.finish())
}
