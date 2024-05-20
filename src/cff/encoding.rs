use crate::cff::charset::Charset;
use crate::cff::types::StringId;
use crate::Error::Unimplemented;
use crate::Result;
use crate::read::LazyArray16;
use crate::read::{Readable, Reader};

#[rustfmt::skip]
pub const STANDARD_ENCODING: [u8; 256] = [
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
    1,   2,   3,   4,   5,   6,   7,   8,   9,  10,  11,  12,  13,  14,  15,  16,
    17,  18,  19,  20,  21,  22,  23,  24,  25,  26,  27,  28,  29,  30,  31,  32,
    33,  34,  35,  36,  37,  38,  39,  40,  41,  42,  43,  44,  45,  46,  47,  48,
    49,  50,  51,  52,  53,  54,  55,  56,  57,  58,  59,  60,  61,  62,  63,  64,
    65,  66,  67,  68,  69,  70,  71,  72,  73,  74,  75,  76,  77,  78,  79,  80,
    81,  82,  83,  84,  85,  86,  87,  88,  89,  90,  91,  92,  93,  94,  95,   0,
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
    0,  96,  97,  98,  99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110,
    0, 111, 112, 113, 114,   0, 115, 116, 117, 118, 119, 120, 121, 122,   0, 123,
    0, 124, 125, 126, 127, 128, 129, 130, 131,   0, 132, 133,   0, 134, 135, 136,
    137,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
    0, 138,   0, 139,   0,   0,   0,   0, 140, 141, 142, 143,   0,   0,   0,   0,
    0, 144,   0,   0,   0, 145,   0,   0, 146, 147, 148, 149,   0,   0,   0,   0,
];

#[derive(Clone, Copy, Debug)]
pub(crate) struct Format1Range {
    first: u8,
    left: u8,
}

impl Readable<'_> for Format1Range {
    const SIZE: usize = 2;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Format1Range { first: r.read::<u8>()?, left: r.read::<u8>()? })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Supplement {
    code: u8,
    name: StringId,
}

impl Readable<'_> for Supplement {
    const SIZE: usize = 3;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Supplement { code: r.read::<u8>()?, name: r.read::<StringId>()? })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum EncodingKind<'a> {
    Standard,
    Expert,
    Format0(LazyArray16<'a, u8>),
    Format1(LazyArray16<'a, Format1Range>),
}

impl EncodingKind {
    pub fn is_custom(&self) {
        matches!(self, EncodingKind::Format0(_) | EncodingKind::Format1(_))
    }

    pub(crate) fn gid_to_code(&self, charset: &Charset, gid: u16) -> Option<u8> {
        match self {
            EncodingKind::Standard | EncodingKind::Expert => panic!("gid_to_code should only be called with custom encodings."),
            EncodingKind::Format0(ref table) => {
                if gid == 0 {

                }
                // +1 because .notdef is implicit.
                table
                    .into_iter()
                    .position(|c| c == code)
                    .map(|i| (i + 1) as u16)
                    .map(GlyphId)
            }
            EncodingKind::Format1(ref table) => {
                // Starts from 1 because .notdef is implicit.
                let mut gid: u16 = 1;
                for range in table.into_iter() {
                    let end = range.first.saturating_add(range.left);
                    if (range.first..=end).contains(&code) {
                        gid += u16::from(code - range.first);
                        return Some(GlyphId(gid));
                    } else {
                        gid += u16::from(range.left) + 1;
                    }
                }

                None
            }
        }
    }
}

impl Default for EncodingKind<'_> {
    fn default() -> Self {
        Self::Standard
    }
}


pub(crate) fn parse_encoding<'a>(r: &mut Reader<'a>) -> Result<Option<Encoding<'a>>> {
    let format = r.read::<u8>()?;
    // The first high-bit in format indicates that a Supplemental encoding is present.
    // Check it and clear.
    let has_supplemental = format & 0x80 != 0;
    let format = format & 0x7f;

    // TODO: Find a test font with supplemental encoding so we can implement and
    // test it.
    if has_supplemental {
        return Err(Unimplemented);
    }

    let count = u16::from(r.read::<u8>()?);
    let kind = match format {
        // TODO: read_array8?
        0 => r.read_array16::<u8>(count).map(EncodingKind::Format0)?,
        1 => r.read_array16::<Format1Range>(count).map(EncodingKind::Format1)?,
        _ => return Ok(None),
    };

    let supplemental = if has_supplemental {
        let count = u16::from(r.read::<u8>()?);
        r.read_array16::<Supplement>(count)?
    } else {
        LazyArray16::default()
    };

    Ok(Some(Encoding { kind, supplemental }))
}

/// Enumerates Charset IDs defined in the Adobe Technical Note #5176, Table 16
mod encoding_id {
    pub const STANDARD: usize = 0;
    pub const EXPERT: usize = 1;
}
