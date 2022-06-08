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
    cid: Option<CidData<'a>>,
}

/// Data specific to CID-keyed fonts.
struct CidData<'a> {
    array: Index<Dict<'a>>,
    select: Cow<'a, [u8]>,
    private_dicts: Vec<Dict<'a>>,
    local_subrs: Vec<Option<Index<Opaque<'a>>>>,
}

/// Recorded offsets that will be written into DICTs.
struct Offsets {
    char_strings: usize,
    charset: Option<usize>,
    private_dict: Option<Range<usize>>,
    local_subrs: Option<usize>,
    cid: Option<CidOffsets>,
}

/// Offsets specific to CID-keyed fonts.
struct CidOffsets {
    array: usize,
    select: usize,
    private_dicts: Vec<Range<usize>>,
    local_subrs: Vec<Option<usize>>,
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
    let mut top_dict = top_dicts.into_one().ok_or(Error::MissingData)?;

    // These are the glyph descriptions.
    let mut char_strings = {
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

    // Read data specific to CID-keyed fonts.
    let mut cid = None;
    if top_dict.get(top::ROS).is_some() {
        cid = Some(read_cid_data(ctx, cff, &top_dict)?);
    }

    // Subset things.
    subset_top(&mut top_dict);
    subset_char_strings(ctx, &mut char_strings)?;

    if let Some(dict) = &mut private_dict {
        subset_private(dict);
    }

    if let Some(cid) = &mut cid {
        for dict in cid.array.iter_mut() {
            subset_top(dict)
        }
        for dict in &mut cid.private_dicts {
            subset_private(dict);
        }
    }

    // Construct a new CFF table.
    let sub_cff = construct(Table {
        name,
        top_dict,
        strings,
        global_subrs,
        charset,
        char_strings,
        private_dict,
        local_subrs,
        cid,
    });

    ctx.push(Tag::CFF, sub_cff);

    Ok(())
}

/// Read data specific to CID-keyed fonts.
fn read_cid_data<'a>(
    ctx: &Context,
    cff: &'a [u8],
    top_dict: &Dict<'a>,
) -> Result<CidData<'a>> {
    // Read FD ARRAY.
    let array = {
        let offset = top_dict.get_offset(top::FD_ARRAY).ok_or(Error::MissingData)?;
        Index::<Dict<'a>>::read_at(cff, offset)?
    };

    // Read FD Select data structure.
    let select = {
        let offset = top_dict.get_offset(top::FD_SELECT).ok_or(Error::MissingData)?;
        let sub = cff.get(offset ..).ok_or(Error::InvalidOffset)?;
        parse_fd_select(sub, ctx.num_glyphs)?
    };

    let mut private_dicts = vec![];
    let mut local_subrs = vec![];

    // Read CID private dicts.
    for dict in array.iter() {
        let range = dict.get_range(top::PRIVATE).ok_or(Error::MissingData)?;
        let start = range.start;
        let sub = cff.get(range).ok_or(Error::InvalidOffset)?;
        let dict = Dict::read_at(sub, 0)?;

        let mut local_subr = None;
        if let Some(offset) = dict.get_offset(private::SUBRS) {
            local_subr = Some(Index::read_at(cff, start + offset)?);
        }

        private_dicts.push(dict);
        local_subrs.push(local_subr);
    }

    Ok(CidData {
        array,
        select,
        private_dicts,
        local_subrs,
    })
}

/// Construct a new CFF table.
fn construct(mut table: Table) -> Vec<u8> {
    let mut data = vec![];
    let mut offsets = setup_offsets(&table);

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
        if let (Some(private_dict), Some(range)) =
            (&table.private_dict, &mut offsets.private_dict)
        {
            range.start = w.len();
            w.write_ref(private_dict);
            range.end = w.len();
            inspect(&w, "Private DICT");
        }

        // Write local subroutines.
        if let Some(local_subrs) = &table.local_subrs {
            let base = offsets.private_dict.as_ref().unwrap().start;
            offsets.local_subrs = Some(w.len() - base);
            w.write_ref(local_subrs);
            inspect(&w, "Local Subroutine INDEX");
        }

        // Write data specific to CID-keyed fonts.
        if let (Some(data), Some(offsets)) = (&table.cid, &mut offsets.cid) {
            // Write FD Array.
            offsets.array = w.len();
            w.write_ref(&data.array);
            inspect(&w, "FD Array");

            // Write FD Select.
            offsets.select = w.len();
            write_fd_select(&mut w, &data.select);
            inspect(&w, "FD Select");

            // Write Private DICTS.
            for (dict, range) in data.private_dicts.iter().zip(&mut offsets.private_dicts)
            {
                range.start = w.len();
                w.write_ref(dict);
                range.end = w.len();
                inspect(&w, "Private DICT");
            }

            // Write local subroutines.
            for (i, subrs) in data.local_subrs.iter().enumerate() {
                if let Some(subrs) = subrs {
                    let base = offsets.private_dicts[i].start;
                    offsets.local_subrs[i] = Some(w.len() - base);
                    w.write_ref(subrs);
                    inspect(&w, "Local Subroutine INDEX");
                }
            }
        }

        data = w.finish();
    }

    data
}

/// Create initial zero offsets for all data structures.
fn setup_offsets(table: &Table) -> Offsets {
    Offsets {
        char_strings: 0,
        charset: table.charset.as_ref().map(|_| 0),
        private_dict: table.private_dict.as_ref().map(|_| 0 .. 0),
        local_subrs: table.local_subrs.as_ref().map(|_| 0),
        cid: table.cid.as_ref().map(|cid| CidOffsets {
            array: 0,
            select: 0,
            private_dicts: vec![0 .. 0; cid.array.len()],
            local_subrs: cid
                .local_subrs
                .iter()
                .map(|subr| subr.as_ref().map(|_| 0))
                .collect(),
        }),
    }
}

/// Insert the offsets of various parts of the font into the relevant
/// dictionaries.
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

    if let (Some(data), Some(offsets)) = (&mut table.cid, &offsets.cid) {
        table.top_dict.set_offset(top::FD_ARRAY, offsets.array);
        table.top_dict.set_offset(top::FD_SELECT, offsets.select);

        for (dict, range) in data.array.iter_mut().zip(&offsets.private_dicts) {
            dict.set_range(top::PRIVATE, range);
        }

        for (private, offset) in data.private_dicts.iter_mut().zip(&offsets.local_subrs) {
            if let &Some(offset) = offset {
                private.set_offset(private::SUBRS, offset);
            }
        }
    }
}

/// Subset a Top DICT.
///
/// Keeps only relevant non-offset entries. Offset entries are inserted later.
fn subset_top<'a>(top_dict: &mut Dict<'a>) {
    top_dict.keep(top::KEEP);
}

/// Subset a Private DICT.
///
/// Keeps only relevant non-offset entries. Offset entries are inserted later.
fn subset_private<'a>(private_dict: &mut Dict<'a>) {
    private_dict.keep(private::KEEP);
}

/// Subset the glyph descriptions.
fn subset_char_strings<'a>(ctx: &Context, strings: &mut Index<Opaque<'a>>) -> Result<()> {
    // The set of all glyphs we will include in the subset.
    let subset: HashSet<u16> = ctx.profile.glyphs.iter().copied().collect();

    for glyph in 0 .. ctx.num_glyphs {
        if !subset.contains(&glyph) {
            // The byte sequence [14] is the minimal valid charstring consisting
            // of just a single `endchar` operator.
            *strings.get_mut(glyph as usize).ok_or(Error::InvalidOffset)? = Opaque(&[14]);
        }
    }

    Ok(())
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

/// Returns the font dict index for each glyph.
fn parse_fd_select(data: &[u8], num_glyphs: u16) -> Result<Cow<'_, [u8]>> {
    let mut r = Reader::new(data);
    let format = r.read::<u8>()?;
    Ok(match format {
        0 => Cow::Borrowed(r.take(num_glyphs as usize)?),
        3 => {
            let count = r.read::<u16>()?;
            let mut fds = vec![];
            let mut first = r.read::<u16>()?;
            for _ in 0 .. count {
                let fd = r.read::<u8>()?;
                let end = r.read::<u16>()?;
                for _ in first .. end {
                    fds.push(fd);
                }
                first = end;
            }
            Cow::Owned(fds)
        }
        _ => return Err(Error::InvalidData),
    })
}

/// Write an FD Select data structure.
fn write_fd_select(w: &mut Writer, fd: &[u8]) {
    w.write::<u8>(0);
    w.give(fd);
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
