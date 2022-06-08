//! Reduce the size and coverage of OpenType fonts.

#![deny(unsafe_code)]
#![deny(missing_docs)]

mod cff;
mod glyf;
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
        let offset = u32::read_at(data, 12 + 4 * (index as usize))?;
        let subdata = data.get(offset as usize ..).ok_or(Error::InvalidOffset)?;
        r = Reader::new(subdata);
        kind = r.read::<FontKind>()?;
        if kind == FontKind::Collection {
            return Err(Error::UnknownKind);
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
    const SVG: Self = Self(*b"SVG ");
}

impl Structure<'_> for Tag {
    fn read(r: &mut Reader) -> Result<Self> {
        r.read::<[u8; 4]>().map(Self)
    }

    fn write(&self, w: &mut Writer) {
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
/// #### Possible Future Work
/// - A setter for variation coordinates which would make the subsetter create a
///   static instance of a variable font.
/// - A profile which keeps and subsets bitmap, color and SVG tables.
/// - A profile which takes a char set instead of a glyph set and subsets the
///   layout tables.
pub struct Profile<'a> {
    glyphs: &'a [u16],
}

impl<'a> Profile<'a> {
    /// Reduces the font to the subset needed for PDF embedding.
    ///
    /// Keeps only the basic required tables plus either the TrueType-related or
    /// CFF-related tables.
    ///
    /// In particular, this also removes all non-outline glyph descriptions like
    /// bitmaps, layered color glyphs and SVGs as these are not mentioned in the
    /// PDF standard and not supported by most PDF readers.
    ///
    /// The subsetted font can be embedded in a PDF as a `FontFile3` with
    /// Subtype `OpenType`. Alternatively:
    /// - For TrueType outlines: You can embed it as a `FontFile2`.
    /// - For CFF outlines: You can extract the CFF table and embed just the
    ///   table as a `FontFile3` with Subtype `Type1C`
    pub fn pdf(glyphs: &'a [u16]) -> Self {
        Self { glyphs }
    }
}

/// Subsetting context.
struct Context<'a> {
    /// Original face.
    face: &'a dyn Face,
    /// The number of glyphs in the original and subsetted face.
    ///
    /// Subsetting doesn't actually delete glyphs, just their outlines.
    num_glyphs: u16,
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

    /// Process a table.
    fn process(&mut self, tag: Tag) -> Result<()> {
        let data = match self.face.table(tag) {
            Some(data) => data,
            None => return Ok(()),
        };

        match tag {
            Tag::GLYF => glyf::subset(self)?,
            Tag::LOCA => panic!("handled by glyf"),
            Tag::CFF => cff::subset(self)?,
            _ => self.push(tag, data),
        }

        Ok(())
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
    let maxp = face.table(Tag::MAXP).ok_or(Error::MissingTable(Tag::MAXP))?;
    let mut ctx = Context {
        face,
        num_glyphs: u16::read_at(maxp, 4)?,
        profile,
        kind: match face.table(Tag::CFF).or(face.table(Tag::CFF2)) {
            Some(_) => FontKind::CFF,
            None => FontKind::TrueType,
        },
        tables: vec![],
    };

    // Required tables.
    ctx.process(Tag::CMAP)?;
    ctx.process(Tag::HEAD)?;
    ctx.process(Tag::HHEA)?;
    ctx.process(Tag::HMTX)?;
    ctx.process(Tag::MAXP)?;
    ctx.process(Tag::NAME)?;
    ctx.process(Tag::OS2)?;
    ctx.process(Tag::POST)?;

    if ctx.kind == FontKind::TrueType {
        // Writes glyf and loca table.
        ctx.process(Tag::GLYF)?;
        ctx.process(Tag::CVT)?;
        ctx.process(Tag::FPGM)?;
        ctx.process(Tag::PREP)?;
        ctx.process(Tag::GASP)?;
    }

    if ctx.kind == FontKind::CFF {
        ctx.process(Tag::CFF)?;
        ctx.process(Tag::CFF2)?;
        ctx.process(Tag::VORG)?;
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
    let mut offset = 12 + ctx.tables.len() * 16;
    for (tag, data) in &mut ctx.tables {
        if *tag == Tag::HEAD {
            // Zero out checksum field in head table.
            data.to_mut()[8 .. 12].fill(0);
            checksum_adjustment_offset = Some(offset + 8);
        }

        let len = data.len();
        println!("{}: {}", tag, len);
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

impl Structure<'_> for FontKind {
    fn read(r: &mut Reader) -> Result<Self> {
        match r.read::<u32>()? {
            0x00010000 | 0x74727565 => Ok(FontKind::TrueType),
            0x4F54544F => Ok(FontKind::CFF),
            0x74746366 => Ok(FontKind::Collection),
            _ => Err(Error::UnknownKind),
        }
    }

    fn write(&self, w: &mut Writer) {
        w.write::<u32>(match self {
            FontKind::TrueType => 0x00010000,
            FontKind::CFF => 0x4F54544F,
            FontKind::Collection => 0x74746366,
        })
    }
}

/// A signed 16-bit fixed-point number.
struct F2Dot14(u16);

impl Structure<'_> for F2Dot14 {
    fn read(r: &mut Reader) -> Result<Self> {
        r.read::<u16>().map(Self)
    }

    fn write(&self, w: &mut Writer) {
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

impl Structure<'_> for TableRecord {
    fn read(r: &mut Reader) -> Result<Self> {
        Ok(TableRecord {
            tag: r.read::<Tag>()?,
            checksum: r.read::<u32>()?,
            offset: r.read::<u32>()?,
            length: r.read::<u32>()?,
        })
    }

    fn write(&self, w: &mut Writer) {
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
    /// The file contains an unknown kind of font.
    UnknownKind,
    /// An offset pointed outside of the data.
    InvalidOffset,
    /// Parsing expected more data.
    MissingData,
    /// Parsed data was invalid.
    InvalidData,
    /// The read data does not conform.
    /// A table is missing.
    ///
    /// Mostly, the subsetter just ignores (i.e. not subsets) tables if they are
    /// missing (even the required ones). This error only occurs if a table
    /// depends on another table and that one is missing, e.g., `glyf` is
    /// present but `loca` is missing.
    MissingTable(Tag),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::UnknownKind => f.pad("unknown font kind"),
            Self::InvalidOffset => f.pad("invalid offset"),
            Self::MissingData => f.pad("missing more data"),
            Self::InvalidData => f.pad("invalid data"),
            Self::MissingTable(tag) => write!(f, "missing {tag} table"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{parse, subset, Profile};

    #[test]
    fn test_subset_truetype() {
        test("NotoSans-Regular.ttf", "Hello<>.!πﬁ");
    }

    #[test]
    fn test_subset_cff() {
        test("NewCMMath-Regular.otf", "1+2=π?");
        test("NotoSansCJKsc-Regular.otf", "ABC你好");
    }

    fn test(path: &str, text: &str) {
        eprintln!("==============================================");
        eprintln!("Testing {path}");

        let data = std::fs::read(Path::new("fonts").join(path)).unwrap();
        let ttf = ttf_parser::Face::from_slice(&data, 0).unwrap();
        let glyphs: Vec<_> =
            text.chars().filter_map(|c| Some(ttf.glyph_index(c)?.0)).collect();

        let face = parse(&data, 0).unwrap();
        let profile = Profile::pdf(&glyphs);
        let subs = subset(&face, profile).unwrap();
        let stem = Path::new(path).file_stem().unwrap().to_str().unwrap();
        let out = Path::new("target").join(Path::new(stem)).with_extension("ttf");
        std::fs::write(out, &subs).unwrap();

        let ttfs = ttf_parser::Face::from_slice(&subs, 0).unwrap();
        let cff = ttfs.tables().cff;
        for c in text.chars() {
            let id = ttf.glyph_index(c).unwrap();
            if let Some(cff) = &cff {
                cff.outline(id, &mut Sink).unwrap();
            }

            macro_rules! same {
                ($method:ident, $($args:tt)*) => {
                    assert_eq!(
                        ttf.$method($($args)*),
                        ttfs.$method($($args)*),
                    );
                };
            }

            same!(glyph_index, c);
            same!(glyph_hor_advance, id);
            same!(glyph_hor_side_bearing, id);
            same!(glyph_bounding_box, id);
        }
    }

    struct Sink;
    impl ttf_parser::OutlineBuilder for Sink {
        fn move_to(&mut self, _: f32, _: f32) {}
        fn line_to(&mut self, _: f32, _: f32) {}
        fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {}
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {}
        fn close(&mut self) {}
    }
}
