//! Reduce the size and coverage of OpenType fonts.

#![deny(unsafe_code)]
#![deny(missing_docs)]

mod stream;

use std::borrow::Cow;
use std::fmt::{self, Debug, Display, Formatter};

use crate::stream::{Reader, Structure, Writer};

/// Parse a font face from OpenType data.
///
/// The `index` is only relevant if the data contains a font collection (`.ttc`
/// or `.otc` file). Otherwise, it should be 0.
///
/// Supports only raw OpenType fonts and collections. If you have a WOFF file or
/// get the tables from somewhere else, you can implement [`Face`] yourself.
pub fn parse(data: &[u8], index: u32) -> Result<impl Face + '_> {
    struct Parsed<'a> {
        data: &'a [u8],
        records: Vec<TableRecord>,
    }

    impl Face for Parsed<'_> {
        fn table(&self, tag: Tag) -> Option<&[u8]> {
            let i = self.records.binary_search_by(|record| record.tag.cmp(&tag)).ok()?;
            let record = self.records.get(i)?;
            let start = record.offset as usize;
            let end = start + (record.length as usize);
            self.data.get(start .. end)
        }
    }

    let mut r = Reader::new(data);
    let mut kind = r.read::<FontKind>()?;

    // Parse font collection header if necessary.
    if kind == FontKind::Collection {
        let offset = u32::read_at(data, 12 + u32::SIZE * (index as usize))?;
        let subdata = data.get(offset as usize ..).ok_or(Error::InvalidOffset)?;
        r = Reader::new(subdata);
        kind = r.read::<FontKind>()?;
        if kind == FontKind::Collection {
            return Err(Error::NestedCollection);
        }
    }

    // Read number of table records.
    let count = r.read::<u16>()?;
    r.read::<u16>()?;
    r.read::<u16>()?;
    r.read::<u16>()?;

    // Read table records.
    let mut records = vec![];
    for _ in 0 .. count {
        records.push(r.read::<TableRecord>()?);
    }

    Ok(Parsed { data, records })
}

/// A font face with OpenType tables.
pub trait Face {
    /// Retrieve the data for the given table.
    fn table(&self, tag: Tag) -> Option<&[u8]>;
}

/// A 4-byte OpenType tag.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Tag(pub [u8; 4]);

#[allow(unused)]
impl Tag {
    // General tables.
    const CMAP: Self = Self(*b"cmap");
    const HEAD: Self = Self(*b"head");
    const HHEA: Self = Self(*b"hhea");
    const HMTX: Self = Self(*b"hmtx");
    const MAXP: Self = Self(*b"maxp");
    const NAME: Self = Self(*b"name");
    const OS2: Self = Self(*b"OS/2");
    const POST: Self = Self(*b"post");

    // TrueType.
    const GLYF: Self = Self(*b"glyf");
    const LOCA: Self = Self(*b"loca");
    const PREP: Self = Self(*b"prep");
    const FPGM: Self = Self(*b"fpgm");
    const CVT: Self = Self(*b"cvt ");
    const GASP: Self = Self(*b"gasp");

    // CFF.
    const CFF: Self = Self(*b"CFF ");
    const CFF2: Self = Self(*b"CFF2");
    const VORG: Self = Self(*b"VORG");

    // Bitmap and color fonts.
    const EBDT: Self = Self(*b"EBDT");
    const EBLC: Self = Self(*b"EBLC");
    const EBSC: Self = Self(*b"EBSC");
    const COLR: Self = Self(*b"COLR");
    const CPAL: Self = Self(*b"CPAL");
    const CBDT: Self = Self(*b"CBDT");
    const CBLC: Self = Self(*b"CBLC");
    const SBIX: Self = Self(*b"sbix");
    const SVG_: Self = Self(*b"SVG ");
}

impl Structure for Tag {
    const SIZE: usize = 4;

    fn read(r: &mut Reader) -> Result<Self> {
        r.read::<[u8; 4]>().map(Self)
    }

    fn write(self, w: &mut Writer) {
        w.write::<[u8; 4]>(self.0)
    }
}

impl Debug for Tag {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Tag({self})")
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.pad(std::str::from_utf8(&self.0).unwrap_or("..."))
    }
}

/// Defines which things to keep in the font.
///
/// #### In the future
/// - All currently defined profiles drop all layout tables. Profiles that
///   subset layout tables like GSUB might be added later.
/// - Profiles might gain support for setting variation coordinates to create a
///   non-variadic instance of a font.
pub struct Profile<'a> {
    glyphs: &'a [u16],
    keep_graphic: bool,
}

impl<'a> Profile<'a> {
    /// Reduces the font to the subset needed for drawing.
    ///
    /// Drops all layout tables. This is the correct profile if text was already
    /// mapped to glyph indices and the consumer of the subsetted font is only
    /// interested in rendering those glyphs.
    pub fn rendering(glyphs: &'a [u16]) -> Self {
        Self { glyphs, keep_graphic: true }
    }

    /// Reduces the font to the subset needed for PDF embedding.
    ///
    /// This is based on the rendering profile but removes all non-outline glyph
    /// descriptions like bitmaps, layered color glyphs and SVGs as these are
    /// not mentioned in the PDF standard and not supported by PDF readers.
    ///
    /// The subsetted font can be embedded in a PDF as a `FontFile3` with
    /// Subtype `OpenType`. Alternatively, it can also be embedded as:
    /// - a `FontFile2` if it contains TrueType outlines
    /// - you can extract the CFF table and embed it as a `FontFile3` with
    ///   Subtype `Type1C` if it contains CFF outlines
    pub fn pdf(glyphs: &'a [u16]) -> Self {
        Self { glyphs, keep_graphic: false }
    }
}

/// Susbetting context.
struct Context<'a> {
    /// Original face.
    face: &'a dyn Face,
    /// The subsetting profile.
    profile: Profile<'a>,
    /// The kind of face.
    kind: FontKind,
    /// Subsetted tables.
    tables: Vec<(Tag, Cow<'a, [u8]>)>,
}

impl<'a> Context<'a> {
    /// Expect a table.
    fn expect_table(&self, tag: Tag) -> Result<&'a [u8]> {
        self.face.table(tag).ok_or(Error::MissingTable(tag))
    }

    /// Copy a table.
    fn copy(&mut self, tag: Tag) {
        if let Some(data) = self.face.table(tag) {
            self.push(tag, data);
        }
    }

    /// Push a subsetted table.
    fn push(&mut self, tag: Tag, table: impl Into<Cow<'a, [u8]>>) {
        debug_assert!(
            !self.tables.iter().any(|&(prev, _)| prev == tag),
            "duplicate {tag} table"
        );
        self.tables.push((tag, table.into()));
    }
}

/// Subset a font face to include less glyphs and tables.
pub fn subset(face: &dyn Face, profile: Profile) -> Result<Vec<u8>> {
    let mut ctx = Context {
        face,
        profile,
        kind: match face.table(Tag::CFF).or(face.table(Tag::CFF2)) {
            Some(_) => FontKind::CFF,
            None => FontKind::TrueType,
        },
        tables: vec![],
    };

    // Required tables.
    ctx.copy(Tag::CMAP);
    ctx.copy(Tag::HEAD);
    ctx.copy(Tag::HHEA);
    ctx.copy(Tag::HMTX);
    ctx.copy(Tag::MAXP);
    ctx.copy(Tag::NAME);
    ctx.copy(Tag::OS2);
    ctx.copy(Tag::POST);

    if ctx.kind == FontKind::TrueType {
        ctx.copy(Tag::GLYF);
        ctx.copy(Tag::LOCA);
        ctx.copy(Tag::CVT);
        ctx.copy(Tag::FPGM);
        ctx.copy(Tag::PREP);
        ctx.copy(Tag::GASP);
    }

    if ctx.kind == FontKind::CFF {
        ctx.copy(Tag::CFF);
        ctx.copy(Tag::CFF2);
        ctx.copy(Tag::VORG)
    }

    if ctx.profile.keep_graphic {
        ctx.copy(Tag::EBDT);
        ctx.copy(Tag::EBLC);
        ctx.copy(Tag::EBSC);
        ctx.copy(Tag::CBDT);
        ctx.copy(Tag::CBLC);
        ctx.copy(Tag::SBIX);
        ctx.copy(Tag::COLR);
        ctx.copy(Tag::CPAL);
        ctx.copy(Tag::SVG_);
    }

    Ok(construct(ctx))
}

/// Construct a brand new font.
fn construct(mut ctx: Context) -> Vec<u8> {
    let mut w = Writer::new();
    w.write::<FontKind>(ctx.kind);

    // Tables shall be sorted by tag.
    ctx.tables.sort_by_key(|&(tag, _)| tag);

    // Write table directory.
    let count = ctx.tables.len() as u16;
    let entry_selector = (count as f32).log2().floor() as u16;
    let search_range = 2u16.pow(u32::from(entry_selector)) * 16;
    let range_shift = count * 16 - search_range;
    w.write(count);
    w.write(search_range);
    w.write(entry_selector);
    w.write(range_shift);

    // This variable will hold the offset to the checksum adjustment field
    // in the head table, which we'll have to write in the end (after
    // checksumming the whole font).
    let mut checksum_adjustment_offset = None;

    // Write table records.
    let mut offset = 12 + ctx.tables.len() * TableRecord::SIZE;
    for (tag, data) in &mut ctx.tables {
        if *tag == Tag::HEAD {
            // Zero out checksum field in head table.
            data.to_mut()[8 .. 12].fill(0);
            checksum_adjustment_offset = Some(offset + 8);
        }

        let len = data.len();
        w.write(TableRecord {
            tag: *tag,
            checksum: checksum(data),
            offset: offset as u32,
            length: len as u32,
        });

        // Increase offset, plus padding zeros to align to 4 bytes.
        offset += len;
        while offset % 4 != 0 {
            offset += 1;
        }
    }

    // Write tables.
    for (_, data) in &ctx.tables {
        // Write data plus padding zeros to align to 4 bytes.
        w.give(data);
        w.align(4);
    }

    // Write checksum adjustment field in head table.
    let mut data = w.finish();
    if let Some(i) = checksum_adjustment_offset {
        let sum = checksum(&data);
        let val = 0xB1B0AFBA_u32.wrapping_sub(sum);
        data[i .. i + 4].copy_from_slice(&val.to_be_bytes());
    }

    data
}

/// Calculate a checksum over the sliced data as a sum of u32s. If the data
/// length is not a multiple of four, it is treated as if padded with zero to a
/// length that is a multiple of four.
fn checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    for chunk in data.chunks(4) {
        let mut bytes = [0; 4];
        bytes[.. chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_be_bytes(bytes));
    }
    sum
}

/// What kind of contents the font has.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum FontKind {
    /// TrueType outlines.
    TrueType,
    /// CFF outlines
    CFF,
    /// A font collection.
    Collection,
}

impl Structure for FontKind {
    const SIZE: usize = 4;

    fn read(r: &mut Reader) -> Result<Self> {
        match r.read::<u32>()? {
            0x00010000 | 0x74727565 => Ok(FontKind::TrueType),
            0x4F54544F => Ok(FontKind::CFF),
            0x74746366 => Ok(FontKind::Collection),
            _ => Err(Error::UnknownKind),
        }
    }

    fn write(self, w: &mut Writer) {
        w.write::<u32>(match self {
            FontKind::TrueType => 0x00010000,
            FontKind::CFF => 0x4F54544F,
            FontKind::Collection => 0x74746366,
        })
    }
}

/// A signed 16-bit fixed-point number.
struct F2Dot14(u16);

impl Structure for F2Dot14 {
    const SIZE: usize = 2;

    fn read(r: &mut Reader) -> Result<Self> {
        r.read::<u16>().map(Self)
    }

    fn write(self, w: &mut Writer) {
        w.write::<u16>(self.0)
    }
}

/// Locates a table in the font file.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct TableRecord {
    tag: Tag,
    checksum: u32,
    offset: u32,
    length: u32,
}

impl Structure for TableRecord {
    const SIZE: usize = 16;

    fn read(r: &mut Reader) -> Result<Self> {
        Ok(TableRecord {
            tag: r.read::<Tag>()?,
            checksum: r.read::<u32>()?,
            offset: r.read::<u32>()?,
            length: r.read::<u32>()?,
        })
    }

    fn write(self, w: &mut Writer) {
        w.write::<Tag>(self.tag);
        w.write::<u32>(self.checksum);
        w.write::<u32>(self.offset);
        w.write::<u32>(self.length);
    }
}

/// The result type for everything.
type Result<T> = std::result::Result<T, Error>;

/// Parsing failed because the font face is malformed.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Error {
    /// The font file starts with an unknown magic number.
    UnknownKind,
    /// The file contained nested font collections.
    NestedCollection,
    /// An offset pointed outside of the data.
    InvalidOffset,
    /// Parsing expected more data.
    MissingData,
    /// A table is missing.
    MissingTable(Tag),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::UnknownKind => f.pad("unknown font kind"),
            Self::NestedCollection => f.pad("nested font collection"),
            Self::InvalidOffset => f.pad("invalid offset"),
            Self::MissingData => f.pad("missing more data"),
            Self::MissingTable(tag) => write!(f, "missing {tag} table"),
        }
    }
}

impl std::error::Error for Error {}
