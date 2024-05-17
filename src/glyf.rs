use super::*;
use crate::Error::{MalformedFont, SubsetError};
use std::collections::HashSet;

pub(crate) fn glyph_closure(face: &Face, gid_mapper: &mut GidMapper) -> Result<()> {
    let table = Table::new(face).ok_or(MalformedFont)?;

    let mut all_glyphs = HashSet::new();
    let mut process_glyphs = gid_mapper.old_gids().collect::<Vec<_>>();

    while let Some(glyph) = process_glyphs.pop() {
        all_glyphs.insert(glyph);
        let glyph_data = match table.glyph_data(glyph) {
            Some(glph_data) => glph_data,
            None => unimplemented!(),
        };

        if glyph_data.is_empty() {
            continue;
        }

        let mut r = Reader::new(glyph_data);
        let num_contours = r.read::<i16>().ok_or(MalformedFont)?;

        if num_contours < 0 {
            for component in component_glyphs(glyph_data).ok_or(MalformedFont)? {
                if !all_glyphs.contains(&component) {
                    process_glyphs.push(component);
                }
            }
        }
    }

    for glyph in all_glyphs {
        gid_mapper.remap(glyph);
    }

    Ok(())
}

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

/// A glyf + loca table.
struct Table<'a> {
    loca: &'a [u8],
    glyf: &'a [u8],
    long: bool,
}

impl<'a> Table<'a> {
    fn new(face: &Face<'a>) -> Option<Self> {
        let loca = face.table(Tag::LOCA)?;
        let glyf = face.table(Tag::GLYF)?;
        let head = face.table(Tag::HEAD)?;

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

fn subset_glyf_entries<'a>(ctx: &mut Context<'a>) -> Result<Vec<Cow<'a, [u8]>>> {
    let table = Table::new(&ctx.face).ok_or(MalformedFont)?;

    let mut size = 0;
    let mut glyf_entries = vec![];

    for old_gid in ctx.mapper.old_gids() {
        let glyph_data = table.glyph_data(old_gid).ok_or(MalformedFont)?;

        // Empty glyph.
        if glyph_data.is_empty() {
            glyf_entries.push(Cow::Borrowed(glyph_data));
            continue;
        }

        let mut r = Reader::new(glyph_data);
        let num_contours = r.read::<i16>().ok_or(MalformedFont)?;

        let glyph_data = if num_contours < 0 {
            Cow::Owned(remap_component_glyph(&ctx.mapper, &glyph_data)?)
        } else {
            // Simple glyphs don't need any subsetting.
            Cow::Borrowed(glyph_data)
        };

        let mut len = glyph_data.len();
        len += (len % 2 != 0) as usize;
        size += len;

        glyf_entries.push(glyph_data);
    }

    ctx.long_loca = size > 2 * (u16::MAX as usize);

    Ok(glyf_entries)
}

fn remap_component_glyph(mapper: &GidMapper, data: &[u8]) -> Result<Vec<u8>> {
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
        let new_component = mapper.get(old_component).ok_or(SubsetError)?;
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

fn component_glyphs(glyph_data: &[u8]) -> Option<impl Iterator<Item = u16> + '_> {
    let mut r = Reader::new(glyph_data);

    // Number of contours
    r.read::<i16>()?;

    // xMin, yMin, xMax, yMax
    r.read::<i16>()?;
    r.read::<i16>()?;
    r.read::<i16>()?;
    r.read::<i16>()?;

    const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
    const WE_HAVE_A_SCALE: u16 = 0x0008;
    const MORE_COMPONENTS: u16 = 0x0020;
    const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 0x0040;
    const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;

    let mut done = false;
    Some(std::iter::from_fn(move || {
        if done {
            return None;
        }

        let flags = r.read::<u16>()?;
        let component = r.read::<u16>()?;

        if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            r.read::<i16>();
            r.read::<i16>();
        } else {
            r.read::<u16>();
        }

        if flags & WE_HAVE_A_SCALE != 0 {
            r.read::<F2Dot14>();
        } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
            r.read::<F2Dot14>();
            r.read::<F2Dot14>();
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            r.read::<F2Dot14>();
            r.read::<F2Dot14>();
            r.read::<F2Dot14>();
            r.read::<F2Dot14>();
        }

        done = flags & MORE_COMPONENTS == 0;
        Some(component)
    }))
}
