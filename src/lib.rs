/*!
Reduces the size and coverage of OpenType fonts with TrueType or CFF outlines for embedding
in PDFs. You can in general expect very good results in terms of font size, as most of the things
that can be subsetted are also subsetted.

# Scope
**Note that the resulting font subsets will most likely be unusable in any other contexts than PDF writing,
since a lot of information will be removed from the font which is not necessary in PDFs, but is
necessary in other contexts.** This is on purpose, and for now, there are no plans to expand the
scope of this crate to become a general purpose subsetter, as this is a massive undertaking and
will make the already complex codebase even more complex.

In the future,
[klippa](https://github.com/googlefonts/fontations/tree/main/klippa) will hopefully fill the gap
of a general-purpose subsetter in the Rust ecosystem.

# Notes
A couple of important notes if you want to use this crate in combination with your own pdf writer:

- You must write your fonts as a CID font. This is because we remove the `cmap` table from the font,
so you must provide your own cmap table in the PDF.
- Copyright information in the font will be retained.
- When writing a CID font in PDF, CIDs must be used to address glyphs. This can be pretty tricky,
because the meaning of CID depends on the type of font you are embedding (see the PDF specification
for more information). The subsetter will convert SID-keyed fonts to CID-keyed ones and an identity
mapping from GID to CID for all fonts, regardless of the previous mapping. Because of this, you can
always use the remapped GID as the CID for a glyph, and do not need to worry about the type of font
you are embedding.

# Example
In the example below, we remove all glyphs except the ones with IDs 68, 69, 70 from Noto Sans.
Those correspond to the letters 'a', 'b' and 'c'. We then save the resulting font to disk.

```
use subsetter::{subset, GlyphRemapper};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
// Read the raw font data.
let data = std::fs::read("fonts/NotoSans-Regular.ttf")?;

// These are the glyphs we want to keep.
let glyphs = &[68, 69, 70];

// Create a new glyph remapper that remaps our glyphs to new glyph IDs. This is necessary because
// glyph IDs in fonts must be consecutive. So if we only include the glyphs 68, 69, 70 and remove all
// other glyph IDs, in the new font they will have the glyph IDs 1, 2, 3.
let mut remapper = GlyphRemapper::new();
for glyph in glyphs {
    remapper.remap(*glyph);
}

// Create the subset.
let sub = subset(&data, 0, &remapper)?;

// This is how you can access the new glyph ID of a glyph in the old font.
for glyph in glyphs {
    println!("Glyph {} has the ID {} in the new font", *glyph, remapper.get(*glyph).unwrap());
}

// Write the resulting file.
std::fs::write("target/Noto-Small.ttf", sub)?;
# Ok(())
# }
```

In the above example, the original font was 556 KB, while the
resulting font is 1 KB.
*/

// TODO: Add example that shows how to use it in combination with PDF.

#![deny(unsafe_code)]
#![deny(missing_docs)]

mod cff;
mod glyf;
mod head;
mod hmtx;
mod maxp;
mod name;
mod post;
mod read;
mod remapper;
mod write;

use crate::read::{Readable, Reader};
pub use crate::remapper::GlyphRemapper;
use crate::write::{Writeable, Writer};
use crate::Error::{MalformedFont, UnknownKind};
use std::borrow::Cow;
use std::fmt::{self, Debug, Display, Formatter};

/// Subset the font face to include only the necessary glyphs and tables.
///
/// - The `data` must be in the OpenType font format.
/// - The `index` is only relevant if the data contains a font collection
///   (`.ttc` or `.otc` file). Otherwise, it should be 0.
pub fn subset(data: &[u8], index: u32, mapper: &GlyphRemapper) -> Result<Vec<u8>> {
    let mapper = mapper.clone();
    let context = prepare_context(data, index, mapper)?;
    _subset(context)
}

fn prepare_context(
    data: &[u8],
    index: u32,
    mut gid_remapper: GlyphRemapper,
) -> Result<Context> {
    let face = parse(data, index)?;
    let kind = match (face.table(Tag::GLYF), face.table(Tag::CFF)) {
        (Some(_), _) => FontKind::TrueType,
        (_, Some(_)) => FontKind::Cff,
        _ => return Err(UnknownKind),
    };

    if kind == FontKind::TrueType {
        glyf::closure(&face, &mut gid_remapper)?;
    }

    Ok(Context {
        face,
        mapper: gid_remapper,
        kind,
        tables: vec![],
        long_loca: false,
    })
}

fn _subset(mut ctx: Context) -> Result<Vec<u8>> {
    // See here for the required tables:
    // https://learn.microsoft.com/en-us/typography/opentype/spec/otff#required-tables
    // but some of those are not strictly needed according to the PDF specification.

    // Of the above tables, we are not including the following ones:
    // - CFF2: Since we don't support CFF2
    // - VORG: PDF doesn't use that table.
    // - CMAP: CID fonts in PDF define their own cmaps, so we don't need to include them in the font.
    // - GASP: Not mandated by PDF specification, and ghostscript also seems to exclude them.
    // - OS2: Not mandated by PDF specification, and ghostscript also seems to exclude them.

    if ctx.kind == FontKind::TrueType {
        // LOCA will be handled by GLYF
        ctx.process(Tag::GLYF)?;
        ctx.process(Tag::CVT)?; // won't be subsetted.
        ctx.process(Tag::FPGM)?; // won't be subsetted.
        ctx.process(Tag::PREP)?; // won't be subsetted.
    }

    if ctx.kind == FontKind::Cff {
        ctx.process(Tag::CFF)?;
    }

    // Required tables.
    ctx.process(Tag::HEAD)?;
    ctx.process(Tag::HMTX)?;
    ctx.process(Tag::MAXP)?;
    // NAME is also not strictly needed, and ghostscript removes it when subsetting.
    // However, it contains copyright information which probably should not be removed...
    // Even though it can free up a lot of space for some fonts.
    ctx.process(Tag::NAME)?;
    ctx.process(Tag::POST)?;

    Ok(construct(ctx))
}

/// Parse a font face from OpenType data.
fn parse(data: &[u8], index: u32) -> Result<Face<'_>> {
    let mut r = Reader::new(data);
    let mut kind = r.read::<FontKind>().ok_or(UnknownKind)?;

    // Parse font collection header if necessary.
    if kind == FontKind::Collection {
        r = Reader::new_at(data, 12 + 4 * (index as usize));
        let offset = r.read::<u32>().ok_or(MalformedFont)?;
        let subdata = data.get(offset as usize..).ok_or(MalformedFont)?;
        r = Reader::new(subdata);
        kind = r.read::<FontKind>().ok_or(MalformedFont)?;

        // Cannot have nested collection
        if kind == FontKind::Collection {
            return Err(MalformedFont);
        }
    }

    // Read number of table records.
    let count = r.read::<u16>().ok_or(MalformedFont)?;
    r.read::<u16>().ok_or(MalformedFont)?;
    r.read::<u16>().ok_or(MalformedFont)?;
    r.read::<u16>().ok_or(MalformedFont)?;

    // Read table records.
    let mut records = vec![];
    for _ in 0..count {
        records.push(r.read::<TableRecord>().ok_or(MalformedFont)?);
    }

    Ok(Face { data, records })
}

/// Construct a brand-new font.
fn construct(mut ctx: Context) -> Vec<u8> {
    ctx.tables.sort_by_key(|&(tag, _)| tag);

    let mut w = Writer::new();
    w.write::<FontKind>(ctx.kind);

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

    data
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
    /// A map from old gids to new gids, and the reverse
    mapper: GlyphRemapper,
    /// The kind of face.
    kind: FontKind,
    /// Subsetted tables.
    tables: Vec<(Tag, Cow<'a, [u8]>)>,
    /// Whether the long loca format was chosen.
    long_loca: bool,
}

impl<'a> Context<'a> {
    /// Expect a table.
    fn expect_table(&self, tag: Tag) -> Option<&'a [u8]> {
        self.face.table(tag)
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
            Tag::HEAD => head::subset(self)?,
            Tag::HHEA => panic!("handled by hmtx"),
            Tag::HMTX => hmtx::subset(self)?,
            Tag::POST => post::subset(self)?,
            Tag::MAXP => maxp::subset(self)?,
            Tag::NAME => name::subset(self)?,
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

impl Readable<'_> for FontKind {
    const SIZE: usize = u32::SIZE;

    fn read(r: &mut Reader) -> Option<Self> {
        match r.read::<u32>()? {
            0x00010000 | 0x74727565 => Some(FontKind::TrueType),
            0x4F54544F => Some(FontKind::Cff),
            0x74746366 => Some(FontKind::Collection),
            _ => None,
        }
    }
}

impl Writeable for FontKind {
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

impl Readable<'_> for Tag {
    const SIZE: usize = u8::SIZE * 4;

    fn read(r: &mut Reader) -> Option<Self> {
        r.read::<[u8; 4]>().map(Self)
    }
}

impl Writeable for Tag {
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

impl Readable<'_> for TableRecord {
    const SIZE: usize = Tag::SIZE + u32::SIZE + u32::SIZE + u32::SIZE;

    fn read(r: &mut Reader) -> Option<Self> {
        Some(TableRecord {
            tag: r.read::<Tag>()?,
            checksum: r.read::<u32>()?,
            offset: r.read::<u32>()?,
            length: r.read::<u32>()?,
        })
    }
}

impl Writeable for TableRecord {
    fn write(&self, w: &mut Writer) {
        w.write::<Tag>(self.tag);
        w.write::<u32>(self.checksum);
        w.write::<u32>(self.offset);
        w.write::<u32>(self.length);
    }
}

/// A signed 16-bit fixed-point number.
struct F2Dot14(u16);

impl Readable<'_> for F2Dot14 {
    const SIZE: usize = u16::SIZE;

    fn read(r: &mut Reader) -> Option<Self> {
        r.read::<u16>().map(Self)
    }
}

impl Writeable for F2Dot14 {
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
    /// The font is malformed (or there is a bug in the font parsing logic).
    MalformedFont,
    /// The font relies on an unimplemented feature, and thus the subsetting
    /// process couldn't be completed.
    Unimplemented,
    /// An unexpected error occurred when subsetting the font. Indicates that there
    /// is a logical bug in the subsetter.
    SubsetError,
    /// An overflow occurred during the computation. Could be either an issue
    /// with the font itself, or a bug in the subsetter logic.
    OverflowError,
    /// An error occurred while processing the CFF table.
    CFFError,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::UnknownKind => f.write_str("unknown font kind"),
            Self::MalformedFont => f.write_str("malformed font"),
            Self::Unimplemented => f.write_str("unsupported feature in font"),
            Self::SubsetError => f.write_str("subsetting of font failed"),
            Self::OverflowError => f.write_str("overflow occurred"),
            Self::CFFError => f.write_str("processing CFF table failed"),
        }
    }
}

impl std::error::Error for Error {}
