use super::*;
use crate::Error::MalformedFont;

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
pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
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

fn subset_glyf_entries<'a>(ctx: &mut Context<'a>) -> Result<Vec<Cow<'a, [u8]>>> {
    let table = Table::new(ctx).ok_or(MalformedFont)?;

    // Because glyphs may depend on other glyphs as components (also with
    // multiple layers of nesting), we have to process all glyphs to find
    // their components.
    let mut size = 0;
    let mut glyf_entries = vec![];

    let mut new_gid = 0;

    // This while loop works under the assumption that adding new GIDs
    // is monotonically increasing.
    while new_gid < ctx.mapper.num_gids() {
        let old_gid = ctx.mapper.get_reverse(new_gid).unwrap();
        let glyph_data = table.glyph_data(old_gid).ok_or(MalformedFont)?;

        // Empty glyph.
        if glyph_data.is_empty() {
            glyf_entries.push(Cow::Borrowed(glyph_data));
            new_gid += 1;
            continue;
        }

        let mut r = Reader::new(glyph_data);
        let num_contours = r.read::<i16>().ok_or(MalformedFont)?;

        let glyph_data = if num_contours < 0 {
            Cow::Owned(remap_component_glyph(ctx, &glyph_data)?)
        } else {
            // Simple glyphs don't need any subsetting.
            Cow::Borrowed(glyph_data)
        };

        let mut len = glyph_data.len();
        len += (len % 2 != 0) as usize;
        size += len;

        glyf_entries.push(glyph_data);

        new_gid += 1;
    }

    ctx.long_loca = size > 2 * (u16::MAX as usize);

    Ok(glyf_entries)
}

fn remap_component_glyph(ctx: &mut Context, data: &[u8]) -> Result<Vec<u8>> {
    let mut r = Reader::new(data);
    let mut w = Writer::with_capacity(data.len());

    // number of contours
    w.write(r.read::<i16>().ok_or(MalformedFont)?);

    // xMin, yMin, xMax, yMax
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
        let old_component = r.read::<u16>().ok_or(MalformedFont)?;
        let new_component = ctx.mapper.remap(old_component);
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
