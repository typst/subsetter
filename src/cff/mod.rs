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
    top: Dict<'a>,
    strings: Index<Opaque<'a>>,
    global_subrs: Index<Opaque<'a>>,
    encoding: Option<Opaque<'a>>,
    charset: Option<Opaque<'a>>,
    char_strings: Index<Opaque<'a>>,
    private: Option<PrivateData<'a>>,
    cid: Option<CidData<'a>>,
}

/// Data specific to Private DICTs.
struct PrivateData<'a> {
    dict: Dict<'a>,
    subrs: Option<Index<Opaque<'a>>>,
}

/// Data specific to CID-keyed fonts.
struct CidData<'a> {
    array: Index<Dict<'a>>,
    select: FdSelect<'a>,
    private: Vec<PrivateData<'a>>,
}

/// An FD Select dat structure.
struct FdSelect<'a>(Cow<'a, [u8]>);

/// Recorded offsets that will be written into DICTs.
struct Offsets {
    char_strings: usize,
    encoding: Option<usize>,
    charset: Option<usize>,
    private: Option<PrivateOffsets>,
    cid: Option<CidOffsets>,
}

/// Offsets specific to Private DICTs.
struct PrivateOffsets {
    dict: Range<usize>,
    subrs: Option<usize>,
}

/// Offsets specific to CID-keyed fonts.
struct CidOffsets {
    array: usize,
    select: usize,
    private: Vec<PrivateOffsets>,
}

/// Find all glyphs referenced through components.
/// CFF doesn't used component glyphs, so it's just the profile's set.
///
/// TODO: What about seac?
pub(crate) fn discover(ctx: &mut Context) {
    ctx.subset = ctx.profile.glyphs.iter().copied().collect();
}

/// Subset the CFF table by removing glyph data for unused glyphs.
pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let cff = ctx.expect_table(Tag::CFF)?;

    // Check version.
    let mut r = Reader::new(cff);
    let major = r.read::<u8>()?;
    if major != 1 {
        ctx.push(Tag::CFF, cff);
        return Ok(());
    }

    // Parse CFF table.
    let mut table = read_cff_table(ctx, cff)?;

    // Subset the char strings.
    subset_char_strings(ctx, &mut table.char_strings)?;

    // Subset Top and Private DICT.
    table.top.retain(top::KEEP);
    if let Some(private) = &mut table.private {
        private.dict.retain(private::KEEP);
    }

    // Subset data specific to CID-keyed fonts.
    if let Some(cid) = &mut table.cid {
        subset_font_dicts(ctx, cid)?;

        for dict in cid.array.iter_mut() {
            dict.retain(top::KEEP);
        }

        for private in &mut cid.private {
            private.dict.retain(private::KEEP);
        }
    }

    // Construct a new CFF table.
    let mut sub_cff = vec![];
    let mut offsets = create_offsets(&table);

    // Write twice because we first need to find out the offsets of various data
    // structures.
    for _ in 0..2 {
        let mut w = Writer::new();
        insert_offsets(&mut table, &offsets);
        write_cff_table(&mut w, &table, &mut offsets);
        sub_cff = w.finish();
    }

    ctx.push(Tag::CFF, sub_cff);

    Ok(())
}

/// Subset the glyph descriptions.
fn subset_char_strings<'a>(ctx: &Context, strings: &mut Index<Opaque<'a>>) -> Result<()> {
    for glyph in 0..ctx.num_glyphs {
        if !ctx.subset.contains(&glyph) {
            // The byte sequence [14] is the minimal valid charstring consisting
            // of just a single `endchar` operator.
            *strings.get_mut(glyph as usize).ok_or(Error::InvalidOffset)? = Opaque(&[14]);
        }
    }

    Ok(())
}

/// Subset CID-related data.
fn subset_font_dicts(ctx: &Context, cid: &mut CidData) -> Result<()> {
    // Determine which subroutine indices to keep.
    let mut kept_subrs = HashSet::new();
    for &glyph in ctx.profile.glyphs {
        kept_subrs
            .insert(*cid.select.0.get(usize::from(glyph)).ok_or(Error::MissingData)?);
    }

    // Remove subroutines for unused Private DICTs.
    for (i, dict) in cid.private.iter_mut().enumerate() {
        if !kept_subrs.contains(&(i as u8)) {
            dict.subrs = None;
        }
    }

    Ok(())
}

/// Parse a CFF table.
fn read_cff_table<'a>(ctx: &Context, cff: &'a [u8]) -> Result<Table<'a>> {
    // Skip header.
    let mut r = Reader::new(cff);
    r.read::<u8>()?;
    r.read::<u8>()?;
    let header_size = r.read::<u8>()? as usize;
    r = Reader::new(cff.get(header_size..).ok_or(Error::InvalidOffset)?);

    // Read four indices at fixed positions.
    let name = r.read::<Index<Opaque>>()?;
    let tops = r.read::<Index<Dict>>()?;
    let strings = r.read::<Index<Opaque>>()?;
    let global_subrs = r.read::<Index<Opaque>>()?;

    // Extract only Top DICT.
    let top = tops.into_one().ok_or(Error::MissingData)?;

    // Read encoding if it exists.
    let mut encoding = None;
    if let Some(offset) = top.get_offset(top::ENCODING) {
        let data = cff.get(offset..).ok_or(Error::InvalidOffset)?;
        encoding = Some(read_encoding(data)?);
    }

    // Read the glyph descriptions.
    let char_strings = {
        let offset = top.get_offset(top::CHAR_STRINGS).ok_or(Error::MissingData)?;
        Index::read_at(cff, offset)?
    };

    // Read the charset.
    let mut charset = None;
    if let Some(offset @ 1..) = top.get_offset(top::CHARSET) {
        let sub = cff.get(offset..).ok_or(Error::InvalidOffset)?;
        charset = Some(read_charset(sub, ctx.num_glyphs)?);
    }

    // Read Private DICT with local subroutines.
    let mut private = None;
    if let Some(range) = top.get_range(top::PRIVATE) {
        private = Some(read_private_dict(cff, range)?);
    }

    // Read data specific to CID-keyed fonts.
    let mut cid = None;
    if top.get(top::ROS).is_some() {
        cid = Some(read_cid_data(ctx, cff, &top)?);
    }

    Ok(Table {
        name,
        top,
        strings,
        global_subrs,
        encoding,
        charset,
        char_strings,
        private,
        cid,
    })
}

/// Write the a new CFF table.
fn write_cff_table(w: &mut Writer, table: &Table, offsets: &mut Offsets) {
    // Write header.
    w.write::<u8>(1);
    w.write::<u8>(0);
    w.write::<u8>(4);
    w.write::<u8>(4);
    w.inspect("Header");

    // Write the four fixed indices.
    w.write_ref(&table.name);
    w.inspect("Name INDEX");

    w.write(Index::from_one(table.top.clone()));
    w.inspect("Top DICT INDEX");

    w.write_ref(&table.strings);
    w.inspect("String INDEX");

    w.write_ref(&table.global_subrs);
    w.inspect("Global Subroutine INDEX");

    // Write encoding.
    if let Some(encoding) = &table.encoding {
        offsets.encoding = Some(w.len());
        write_encoding(w, encoding);
        w.inspect("Encoding");
    }

    // Write charset.
    if let Some(charset) = &table.charset {
        offsets.charset = Some(w.len());
        write_charset(w, charset);
        w.inspect("Charset");
    }

    // Write char strings.
    offsets.char_strings = w.len();
    w.write_ref(&table.char_strings);
    w.inspect("Charstring INDEX");

    // Write private dict.
    if let (Some(private), Some(offsets)) = (&table.private, &mut offsets.private) {
        write_private_data(w, private, offsets);
    }

    // Write data specific to CID-keyed fonts.
    if let (Some(cid), Some(offsets)) = (&table.cid, &mut offsets.cid) {
        write_cid_data(w, cid, offsets);
    }
}

/// Read data specific to CID-keyed fonts.
fn read_cid_data<'a>(
    ctx: &Context,
    cff: &'a [u8],
    top: &Dict<'a>,
) -> Result<CidData<'a>> {
    // Read FD Array.
    let array = {
        let offset = top.get_offset(top::FD_ARRAY).ok_or(Error::MissingData)?;
        Index::<Dict<'a>>::read_at(cff, offset)?
    };

    // Read FD Select data structure.
    let select = {
        let offset = top.get_offset(top::FD_SELECT).ok_or(Error::MissingData)?;
        let sub = cff.get(offset..).ok_or(Error::InvalidOffset)?;
        read_fd_select(sub, ctx.num_glyphs)?
    };

    // Read Private DICTs.
    let mut private = vec![];
    for dict in array.iter() {
        let range = dict.get_range(top::PRIVATE).ok_or(Error::MissingData)?;
        private.push(read_private_dict(cff, range)?);
    }

    Ok(CidData { array, select, private })
}

/// Write data specific to CID-keyed fonts.
fn write_cid_data(w: &mut Writer, cid: &CidData, offsets: &mut CidOffsets) {
    // Write FD Array.
    offsets.array = w.len();
    w.write_ref(&cid.array);
    w.inspect("FD Array");

    // Write FD Select.
    offsets.select = w.len();
    write_fd_select(w, &cid.select);
    w.inspect("FD Select");

    // Write Private DICTs.
    for (private, offsets) in cid.private.iter().zip(&mut offsets.private) {
        write_private_data(w, private, offsets);
    }
}

/// Read a Private DICT and optionally local subroutines.
fn read_private_dict<'a>(cff: &'a [u8], range: Range<usize>) -> Result<PrivateData<'a>> {
    let start = range.start;
    let sub = cff.get(range).ok_or(Error::InvalidOffset)?;
    let dict = Dict::read_at(sub, 0)?;

    let mut subrs = None;
    if let Some(offset) = dict.get_offset(private::SUBRS) {
        subrs = Some(Index::read_at(cff, start + offset)?);
    }

    Ok(PrivateData { dict, subrs })
}

/// Write a Private DICT and optionally local subroutines.
fn write_private_data(
    w: &mut Writer,
    private: &PrivateData,
    offsets: &mut PrivateOffsets,
) {
    offsets.dict.start = w.len();
    w.write_ref(&private.dict);
    offsets.dict.end = w.len();
    w.inspect("Private DICT");

    // Write local subroutines.
    if let Some(subrs) = &private.subrs {
        offsets.subrs = Some(w.len() - offsets.dict.start);
        w.write_ref(subrs);
        w.inspect("Local Subroutine INDEX");
    }
}

/// Read an encoding.
fn read_encoding(data: &[u8]) -> Result<Opaque<'_>> {
    let mut r = Reader::new(data);
    let mut len = 1;

    let format = r.read::<u8>()?;
    match format {
        0 => {
            let n_codes = r.read::<u8>()? as usize;
            len += 1 + n_codes;
        }
        1 => {
            let n_ranges = r.read::<u8>()? as usize;
            len += 1 + 2 * n_ranges;
        }
        _ => return Err(Error::InvalidData),
    }

    Ok(Opaque(data.get(..len).ok_or(Error::InvalidOffset)?))
}

/// Write an encoding.
fn write_encoding(w: &mut Writer, encoding: &Opaque<'_>) {
    w.write_ref(encoding);
}

/// Read a charset.
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
                seen = seen.saturating_add(r.read::<u16>()?);
                len += 4;
            }
        }
        _ => return Err(Error::InvalidData),
    }

    Ok(Opaque(data.get(..len).ok_or(Error::InvalidOffset)?))
}

/// Write a charset.
fn write_charset(w: &mut Writer, charset: &Opaque<'_>) {
    w.write_ref(charset);
}

/// Read the FD Select data structure.
fn read_fd_select(data: &[u8], num_glyphs: u16) -> Result<FdSelect<'_>> {
    let mut r = Reader::new(data);
    let format = r.read::<u8>()?;
    Ok(FdSelect(match format {
        0 => Cow::Borrowed(r.take(num_glyphs as usize)?),
        3 => {
            let count = r.read::<u16>()?;
            let mut fds = vec![];
            let mut first = r.read::<u16>()?;
            for _ in 0..count {
                let fd = r.read::<u8>()?;
                let end = r.read::<u16>()?;
                for _ in first..end {
                    fds.push(fd);
                }
                first = end;
            }
            Cow::Owned(fds)
        }
        _ => return Err(Error::InvalidData),
    }))
}

/// Write an FD Select data structure.
fn write_fd_select(w: &mut Writer, select: &FdSelect) {
    w.write::<u8>(0);
    w.give(&select.0);
}

/// Create initial zero offsets for all data structures.
fn create_offsets(table: &Table) -> Offsets {
    Offsets {
        char_strings: 0,
        charset: table.charset.as_ref().map(|_| 0),
        encoding: table.encoding.as_ref().map(|_| 0),
        private: table.private.as_ref().map(create_private_offsets),
        cid: table.cid.as_ref().map(create_cid_offsets),
    }
}

/// Create initial zero offsets for all CID-related data structures.
fn create_cid_offsets(cid: &CidData) -> CidOffsets {
    CidOffsets {
        array: 0,
        select: 0,
        private: cid.private.iter().map(create_private_offsets).collect(),
    }
}

/// Create initial zero offsets for a Private DICT.
fn create_private_offsets(private: &PrivateData) -> PrivateOffsets {
    PrivateOffsets {
        dict: 0..0,
        subrs: private.subrs.as_ref().map(|_| 0),
    }
}

/// Insert the offsets of various parts of the font into the relevant DICTs.
fn insert_offsets(table: &mut Table, offsets: &Offsets) {
    if let Some(offset) = offsets.encoding {
        table.top.set_offset(top::ENCODING, offset);
    }

    if let Some(offset) = offsets.charset {
        table.top.set_offset(top::CHARSET, offset);
    }

    table.top.set_offset(top::CHAR_STRINGS, offsets.char_strings);

    if let (Some(private), Some(offsets)) = (&mut table.private, &offsets.private) {
        table.top.set_range(top::PRIVATE, &offsets.dict);

        if let Some(offset) = offsets.subrs {
            private.dict.set_offset(private::SUBRS, offset);
        }
    }

    if let (Some(cid), Some(offsets)) = (&mut table.cid, &offsets.cid) {
        table.top.set_offset(top::FD_ARRAY, offsets.array);
        table.top.set_offset(top::FD_SELECT, offsets.select);

        for (dict, offsets) in cid.array.iter_mut().zip(&offsets.private) {
            dict.set_range(top::PRIVATE, &offsets.dict);
        }

        for (private, offsets) in cid.private.iter_mut().zip(&offsets.private) {
            if let Some(offset) = offsets.subrs {
                private.dict.set_offset(private::SUBRS, offset);
            }
        }
    }
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
