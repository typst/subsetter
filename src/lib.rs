/*!
Reduces the size and coverage of OpenType fonts with TrueType or CFF outlines.

# Example
In the example below, we remove all glyphs except the ones with IDs 68, 69, 70.
Those correspond to the letters 'a', 'b' and 'c'.

```
// use subsetter::{subset, Profile};
//
// # fn main() -> Result<(), Box<dyn std::error::Error>> {
// // Read the raw font data.
// let data = std::fs::read("fonts/NotoSans-Regular.ttf")?;
//
// // Keep only three glyphs and the OpenType tables
// // required for embedding the font in a PDF file.
// let glyphs = &[68, 69, 70];
// let profile = Profile::pdf(glyphs);
// let sub = subset(&data, 0, profile)?;
//
// // Write the resulting file.
// std::fs::write("target/Noto-Small.ttf", sub)?;
// # Ok(())
// # }
```

Notably, this subsetter does not really remove glyphs, just their outlines. This
means that you don't have to worry about changed glyphs IDs. However, it also
means that the resulting font won't always be as small as possible. To somewhat
remedy this, this crate sometimes at least zeroes out unused data that it cannot
fully remove. This helps if the font gets compressed, for example when embedding
it in a PDF file.

In the above example, the original font was 375 KB (188 KB zipped) while the
resulting font is 36 KB (5 KB zipped).
*/
// TODO: Update code examples, README and look at documentation again.

#![deny(unsafe_code)]
#![deny(missing_docs)]

mod cmap;
mod glyf;
mod head;
mod hhea;
mod hmtx;
mod maxp;
mod name;
mod post;
mod stream;

use crate::stream::{Reader, Structure, Writer};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Debug, Display, Formatter};

/// Subset a font face to include less glyphs and tables.
///
/// - The `data` must be in the OpenType font format.
/// - The `index` is only relevant if the data contains a font collection
///   (`.ttc` or `.otc` file). Otherwise, it should be 0.
pub fn subset(
    data: &[u8],
    index: u32,
    profile: &[u16],
) -> Result<(Vec<u8>, HashMap<u16, u16>)> {
    let face = parse(data, index)?;
    let kind = match face.table(Tag::CFF).or(face.table(Tag::CFF2)) {
        Some(_) => FontKind::Cff,
        None => FontKind::TrueType,
    };

    let maxp = face.table(Tag::MAXP).ok_or(Error::MissingTable(Tag::MAXP))?;
    let num_glyphs = u16::read_at(maxp, 4)?;

    let mut requested_glyphs = HashSet::from_iter(profile.iter().copied());
    // We always include the .notdef glyph.
    requested_glyphs.insert(0);

    let mut ctx = Context {
        face,
        num_glyphs,
        subset: HashSet::new(),
        requested_glyphs,
        gid_map: HashMap::new(),
        reverse_gid_map: vec![],
        kind,
        tables: vec![],
        long_loca: true,
    };

    // See here for the required tables:
    // https://learn.microsoft.com/en-us/typography/opentype/spec/otff#required-tables
    // some of those are not strictly needed according to the PDF specification,
    // but it's still better to include them.

    if ctx.kind == FontKind::TrueType {
        glyf::discover(&mut ctx)?;
    }

    // if ctx.kind == FontKind::Cff {
    //     cff::discover(&mut ctx);
    // }

    ctx.initialize_gid_map();

    if ctx.kind == FontKind::TrueType {
        ctx.process(Tag::GLYF)?;
        // LOCA will be handled by GLYF
        ctx.process(Tag::CVT)?; // won't be subsetted.
        ctx.process(Tag::FPGM)?; // won't be subsetted.
        ctx.process(Tag::PREP)?; // won't be subsetted.
        ctx.process(Tag::GASP)?; // won't be subsetted.
    }

    // if ctx.kind == FontKind::Cff {
    //     ctx.process(Tag::CFF)?;
    //     ctx.process(Tag::CFF2)?;
    //     ctx.process(Tag::VORG)?;
    // }

    // Required tables.
    ctx.process(Tag::CMAP)?;
    ctx.process(Tag::HEAD)?;
    ctx.process(Tag::HHEA)?;
    ctx.process(Tag::HMTX)?;
    ctx.process(Tag::MAXP)?;
    ctx.process(Tag::NAME)?;
    ctx.process(Tag::OS2)?;
    ctx.process(Tag::POST)?;

    Ok(construct(ctx))
}

/// Parse a font face from OpenType data.
fn parse(data: &[u8], index: u32) -> Result<Face<'_>> {
    let mut r = Reader::new(data);
    let mut kind = r.read::<FontKind>()?;

    // Parse font collection header if necessary.
    if kind == FontKind::Collection {
        let offset = u32::read_at(data, 12 + 4 * (index as usize))?;
        let subdata = data.get(offset as usize..).ok_or(Error::InvalidOffset)?;
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
    for _ in 0..count {
        records.push(r.read::<TableRecord>()?);
    }

    Ok(Face { data, records })
}

/// Construct a brand new font.
fn construct(mut ctx: Context) -> (Vec<u8>, HashMap<u16, u16>) {
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
            data.to_mut()[8..12].fill(0);
            checksum_adjustment_offset = Some(offset + 8);
        }

        let len = data.len();
        w.write(TableRecord {
            tag: *tag,
            checksum: checksum(data),
            offset: offset as u32,
            length: len as u32,
        });

        #[cfg(test)]
        eprintln!("{}: {}", tag, len);

        // Increase offset, plus padding zeros to align to 4 bytes.
        offset += len;
        while offset % 4 != 0 {
            offset += 1;
        }
    }

    // Write tables.
    for (_, data) in &ctx.tables {
        // Write data plus padding zeros to align to 4 bytes.
        w.extend(data);
        w.align(4);
    }

    // Write checksum adjustment field in head table.
    let mut data = w.finish();
    if let Some(i) = checksum_adjustment_offset {
        let sum = checksum(&data);
        let val = 0xB1B0AFBA_u32.wrapping_sub(sum);
        data[i..i + 4].copy_from_slice(&val.to_be_bytes());
    }

    (data, ctx.gid_map)
}

/// Calculate a checksum over the sliced data as a sum of u32s. If the data
/// length is not a multiple of four, it is treated as if padded with zero to a
/// length that is a multiple of four.
fn checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    for chunk in data.chunks(4) {
        let mut bytes = [0; 4];
        bytes[..chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_be_bytes(bytes));
    }
    sum
}

/// Subsetting context.
struct Context<'a> {
    /// Original face.
    face: Face<'a>,
    /// The number of glyphs in the original face.
    num_glyphs: u16,
    /// Requested glyphs to subset
    requested_glyphs: HashSet<u16>,
    /// Actual glyphs that are needed to subset the font correctly,
    /// including glyphs referenced indirectly through components.
    subset: HashSet<u16>,
    // A map from old gids to new gids
    gid_map: HashMap<u16, u16>,
    // A map from new gids to old gids. The index represents the
    // new gid, and the value at that index the old gid.
    reverse_gid_map: Vec<u16>,
    /// The kind of face.
    kind: FontKind,
    /// Subsetted tables.
    tables: Vec<(Tag, Cow<'a, [u8]>)>,
    /// Whether the long loca format was chosen.
    long_loca: bool,
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
            // Tag::CFF => cff::subset(self)?,
            Tag::HEAD => head::subset(self)?,
            Tag::HHEA => hhea::subset(self)?,
            Tag::HMTX => hmtx::subset(self)?,
            Tag::POST => post::subset(self)?,
            Tag::CMAP => cmap::subset(self)?,
            Tag::MAXP => maxp::subset(self)?,
            Tag::NAME => name::subset(self)?,
            _ => self.push(tag, data),
        }

        Ok(())
    }

    fn initialize_gid_map(&mut self) {
        let mut original_gids = self.subset.iter().collect::<Vec<_>>();
        original_gids.sort();

        for (counter, gid) in original_gids.into_iter().enumerate() {
            self.gid_map.insert(*gid, counter as u16);
            self.reverse_gid_map.push(*gid);
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

/// A font face with OpenType tables.
struct Face<'a> {
    data: &'a [u8],
    records: Vec<TableRecord>,
}

impl<'a> Face<'a> {
    fn table(&self, tag: Tag) -> Option<&'a [u8]> {
        let i = self.records.binary_search_by(|record| record.tag.cmp(&tag)).ok()?;
        let record = self.records.get(i)?;
        let start = record.offset as usize;
        let end = start + (record.length as usize);
        self.data.get(start..end)
    }
}

/// What kind of contents the font has.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum FontKind {
    /// TrueType outlines.
    TrueType,
    /// CFF outlines
    Cff,
    /// A font collection.
    Collection,
}

impl Structure<'_> for FontKind {
    fn read(r: &mut Reader) -> Result<Self> {
        match r.read::<u32>()? {
            0x00010000 | 0x74727565 => Ok(FontKind::TrueType),
            0x4F54544F => Ok(FontKind::Cff),
            0x74746366 => Ok(FontKind::Collection),
            _ => Err(Error::UnknownKind),
        }
    }

    fn write(&self, w: &mut Writer) {
        w.write::<u32>(match self {
            FontKind::TrueType => 0x00010000,
            FontKind::Cff => 0x4F54544F,
            FontKind::Collection => 0x74746366,
        })
    }
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
    /// A table is missing.
    ///
    /// Mostly, the subsetter just ignores (i.e. not subsets) tables if they are
    /// missing (even the required ones). This error only occurs if a table
    /// depends on another table and that one is missing, e.g., `glyf` is
    /// present but `loca` is missing.
    MissingTable(Tag),
    /// The font relies on some unimplemented feature, and thus we cannot guarantee
    /// that the subsetted font would be correct.
    Unimplemented,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::UnknownKind => f.pad("unknown font kind"),
            Self::InvalidOffset => f.pad("invalid offset"),
            Self::MissingData => f.pad("missing more data"),
            Self::InvalidData => f.pad("invalid data"),
            Self::MissingTable(tag) => write!(f, "missing {tag} table"),
            Self::Unimplemented => f.pad("unimplemented feature in font"),
        }
    }
}

impl std::error::Error for Error {}
