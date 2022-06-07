mod dict;
mod index;

use std::collections::HashSet;
use std::fmt::{self, Debug, Formatter};
use std::ops::Range;

use self::dict::*;
use self::index::*;
use super::*;

/// A CFF table.
struct Table<'a> {
    name: Index<Opaque<'a>>,
    top_dict: Dict<'a>,
    strings: Index<Opaque<'a>>,
    global_subrs: Index<Opaque<'a>>,
    char_strings: Index<Opaque<'a>>,
    charset: Option<Opaque<'a>>,
    private_dict: Option<Dict<'a>>,
    local_subrs: Option<Index<Opaque<'a>>>,
}

/// Recorded offsets that will be written into DICTs.
struct Offsets {
    char_strings: usize,
    charset: Option<usize>,
    private_dict: Option<Range<usize>>,
    local_subrs: Option<usize>,
}

/// Subset the CFF table by removing glyph data for unused glyphs.
pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let cff = ctx.expect_table(Tag::CFF)?;
    let mut r = Reader::new(cff);

    // Read version.
    let (major, _) = (r.read::<u8>()?, r.read::<u8>());
    if major != 1 {
        ctx.push(Tag::CFF, cff);
        return Ok(());
    }

    let header_size = r.read::<u8>()? as usize;
    r = Reader::new(cff.get(header_size ..).ok_or(Error::InvalidOffset)?);

    // Read four indices at fixed positions.
    let name = r.read::<Index<Opaque>>()?;
    let top_dicts = r.read::<Index<Dict>>()?;
    let strings = r.read::<Index<Opaque>>()?;
    let global_subrs = r.read::<Index<Opaque>>()?;

    // Extract only Top DICT.
    let top_dict = top_dicts.into_one().ok_or(Error::MissingData)?;

    // Bail out for CID fonts.
    if top_dict.get(top::ROS).is_some() {
        ctx.push(Tag::CFF, cff);
        return Ok(());
    }

    // CFF Encoding shouldn't really happen in OpenType fonts since there's
    // cmap, but if it does, we bail out.
    if let Some(1 ..) = top_dict.get_offset(top::ENCODING) {
        ctx.push(Tag::CFF, cff);
        return Ok(());
    }

    // These are the glyph descriptions.
    let char_strings = {
        let offset = top_dict.get_offset(top::CHAR_STRINGS).ok_or(Error::MissingData)?;
        Index::read_at(cff, offset)?
    };

    // Copy over charset.
    let mut charset = None;
    if let Some(offset @ 1 ..) = top_dict.get_offset(top::CHARSET) {
        let sub = cff.get(offset ..).ok_or(Error::InvalidOffset)?;
        charset = Some(read_charset(sub, ctx.num_glyphs)?);
    }

    // Read Private DICT with local subroutines.
    let mut private_dict = None;
    let mut local_subrs = None;
    if let Some(range) = top_dict.get_range(top::PRIVATE) {
        let start = range.start;
        let sub = cff.get(range).ok_or(Error::InvalidOffset)?;
        let dict = Dict::read_at(sub, 0)?;

        if let Some(offset) = dict.get_offset(private::SUBRS) {
            local_subrs = Some(Index::read_at(cff, start + offset)?);
        }

        private_dict = Some(dict);
    }

    // Construct a new CFF table.
    let sub_cff = construct(Table {
        name,
        top_dict: subset_top(&top_dict),
        strings,
        global_subrs,
        charset,
        char_strings: subset_char_strings(ctx, char_strings)?,
        private_dict: private_dict.as_ref().map(subset_private),
        local_subrs,
    });

    ctx.push(Tag::CFF, sub_cff);

    Ok(())
}

/// Construct a new CFF table.
fn construct(mut table: Table) -> Vec<u8> {
    let mut data = vec![];
    let mut offsets = Offsets {
        char_strings: 0,
        charset: table.charset.is_some().then(|| 0),
        private_dict: table.private_dict.is_some().then(|| 0 .. 0),
        local_subrs: table.local_subrs.is_some().then(|| 0),
    };

    for run in 0 .. 2 {
        let mut last = 0;
        let mut inspect = |w: &Writer, _name: &str| {
            if run > 0 {
                #[cfg(test)]
                eprintln!("{_name} took {} bytes", w.len() - last);
                last = w.len();
            }
        };

        set_offets(&mut table, &offsets);

        // Write header.
        let mut w = Writer::new();
        w.write::<u8>(1);
        w.write::<u8>(0);
        w.write::<u8>(4);
        w.write::<u8>(4);
        inspect(&w, "Header");

        // Write the four fixed indices.
        w.write_ref(&table.name);
        inspect(&w, "Name INDEX");

        w.write(Index::from_one(table.top_dict.clone()));
        inspect(&w, "Top DICT INDEX");

        w.write_ref(&table.strings);
        inspect(&w, "String INDEX");

        w.write_ref(&table.global_subrs);
        inspect(&w, "Global Subroutine INDEX");

        // Write charset.
        if let Some(charset) = &table.charset {
            offsets.charset = Some(w.len());
            w.write_ref(charset);
            inspect(&w, "Charset");
        }

        // Write char strings.
        offsets.char_strings = w.len();
        w.write_ref(&table.char_strings);
        inspect(&w, "Charstring INDEX");

        // Write private dict.
        let private_start = w.len();
        if let Some(private_dict) = &table.private_dict {
            w.write_ref(private_dict);
            let end = w.len();
            offsets.private_dict = Some(private_start .. end);
            inspect(&w, "Private DICT");
        }

        // Write local subroutines.
        if let Some(local_subrs) = &table.local_subrs {
            offsets.local_subrs = Some(w.len() - private_start);
            w.write_ref(local_subrs);
            inspect(&w, "Local Subroutine INDEX");
        }

        data = w.finish();
    }

    data
}

/// Insert the offsets of various parts of the font into the relevant dictionaries.
fn set_offets(table: &mut Table, offsets: &Offsets) {
    if let Some(offset) = offsets.charset {
        table.top_dict.set_offset(top::CHARSET, offset);
    }

    table.top_dict.set_offset(top::CHAR_STRINGS, offsets.char_strings);

    if let Some(range) = &offsets.private_dict {
        table.top_dict.set_range(top::PRIVATE, range);
    }

    if let (Some(private), Some(offset)) = (&mut table.private_dict, offsets.local_subrs)
    {
        private.set_offset(private::SUBRS, offset);
    }
}

/// Subset a Top DICT.
///
/// Keeps only relevant non-offset entries. Offset entries are inserted later.
fn subset_top<'a>(top_dict: &Dict<'a>) -> Dict<'a> {
    let mut sub = Dict::new();
    sub.copy(&top_dict, top::ROS);
    sub.copy(&top_dict, top::CID_FONT_VERSION);
    sub.copy(&top_dict, top::CID_FONT_REVISION);
    sub.copy(&top_dict, top::CID_FONT_TYPE);
    sub.copy(&top_dict, top::CID_COUNT);
    sub.copy(&top_dict, top::FD_ARRAY);
    sub.copy(&top_dict, top::FD_SELECT);
    sub.copy(&top_dict, top::FONT_NAME);
    sub.copy(&top_dict, top::VERSION);
    sub.copy(&top_dict, top::NOTICE);
    sub.copy(&top_dict, top::COPYRIGHT);
    sub.copy(&top_dict, top::FULL_NAME);
    sub.copy(&top_dict, top::FAMILY_NAME);
    sub.copy(&top_dict, top::WEIGHT);
    sub.copy(&top_dict, top::IS_FIXED_PITCH);
    sub.copy(&top_dict, top::ITALIC_ANGLE);
    sub.copy(&top_dict, top::UNDERLINE_POSITION);
    sub.copy(&top_dict, top::UNDERLINE_THICKNESS);
    sub.copy(&top_dict, top::PAINT_TYPE);
    sub.copy(&top_dict, top::CHARSTRING_TYPE);
    sub.copy(&top_dict, top::FONT_MATRIX);
    sub.copy(&top_dict, top::FONT_BBOX);
    sub.copy(&top_dict, top::STROKE_WIDTH);
    sub.copy(&top_dict, top::POST_SCRIPT);
    sub
}

/// Subset a Private DICT.
///
/// Keeps only relevant non-offset entries. Offset entries are inserted later.
fn subset_private<'a>(private_dict: &Dict<'a>) -> Dict<'a> {
    let mut sub = Dict::new();
    sub.copy(&private_dict, private::BLUE_VALUES);
    sub.copy(&private_dict, private::OTHER_BLUES);
    sub.copy(&private_dict, private::FAMILY_BLUES);
    sub.copy(&private_dict, private::FAMILY_OTHER_BLUES);
    sub.copy(&private_dict, private::BLUE_SCALE);
    sub.copy(&private_dict, private::BLUE_SHIFT);
    sub.copy(&private_dict, private::BLUE_FUZZ);
    sub.copy(&private_dict, private::STD_HW);
    sub.copy(&private_dict, private::STD_VW);
    sub.copy(&private_dict, private::STEM_SNAP_H);
    sub.copy(&private_dict, private::STEM_SNAP_V);
    sub.copy(&private_dict, private::FORCE_BOLD);
    sub.copy(&private_dict, private::LANGUAGE_GROUP);
    sub.copy(&private_dict, private::EXPANSION_FACTOR);
    sub.copy(&private_dict, private::INITIAL_RANDOM_SEED);
    sub.copy(&private_dict, private::DEFAULT_WIDTH_X);
    sub.copy(&private_dict, private::NOMINAL_WIDTH_X);
    sub
}

/// Extract the charset bytes.
fn read_charset(data: &[u8], num_glyphs: u16) -> Result<Opaque<'_>> {
    let mut r = Reader::new(data);
    let mut len = 1;

    let format = r.read::<u8>()?;
    match format {
        0 => {
            len += 2 * num_glyphs.saturating_sub(1) as usize;
        }
        1 => {
            let mut seen = 1;
            while seen < num_glyphs {
                r.read::<u16>()?;
                seen = seen.saturating_add(1);
                seen = seen.saturating_add(r.read::<u8>()? as u16);
                len += 3;
            }
        }
        2 => {
            let mut seen = 1;
            while seen < num_glyphs {
                r.read::<u16>()?;
                seen = seen.saturating_add(1);
                seen = seen.saturating_add(1 + r.read::<u16>()?);
                len += 4;
            }
        }
        _ => return Err(Error::InvalidData),
    }

    Ok(Opaque(data.get(.. len).ok_or(Error::InvalidOffset)?))
}

/// Subset the glyph descriptions.
fn subset_char_strings<'a>(
    ctx: &Context,
    mut strings: Index<Opaque<'a>>,
) -> Result<Index<Opaque<'a>>> {
    // The set of all glyphs we will include in the subset.
    let subset: HashSet<u16> = ctx.profile.glyphs.iter().copied().collect();

    for glyph in 0 .. ctx.num_glyphs {
        if !subset.contains(&glyph) {
            // The byte sequence [14] is the minimal valid charstring consisting
            // of just a single `endchar` operator.
            *strings.get_mut(glyph as usize).ok_or(Error::InvalidOffset)? = Opaque(&[14]);
        }
    }

    Ok(strings)
}

/// An opaque binary data structure.
struct Opaque<'a>(&'a [u8]);

impl<'a> Structure<'a> for Opaque<'a> {
    fn read(r: &mut Reader<'a>) -> Result<Self> {
        let data = r.data();
        r.skip(data.len())?;
        Ok(Self(data))
    }

    fn write(&self, w: &mut Writer) {
        w.give(&self.0);
    }
}

impl Debug for Opaque<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.pad("Opaque { .. }")
    }
}
