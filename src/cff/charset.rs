use crate::cff::number::StringId;
use crate::cff::remapper::SidRemapper;
use crate::cff::FontKind;
use crate::read::LazyArray16;
use crate::read::{Readable, Reader};
use crate::write::Writer;
use crate::Error::{MalformedFont, SubsetError};
use crate::GlyphRemapper;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Charset<'a> {
    ISOAdobe,
    Expert,
    ExpertSubset,
    Format0(LazyArray16<'a, StringId>),
    Format1(LazyArray16<'a, Format1Range>),
    Format2(LazyArray16<'a, Format2Range>),
}

impl Charset<'_> {
    pub fn gid_to_sid(&self, gid: u16) -> Option<StringId> {
        match self {
            Charset::ISOAdobe => {
                if gid <= 228 {
                    Some(StringId(gid))
                } else {
                    None
                }
            }
            Charset::Expert => {
                EXPERT_ENCODING.get(usize::from(gid)).cloned().map(StringId)
            }
            Charset::ExpertSubset => {
                EXPERT_SUBSET_ENCODING.get(usize::from(gid)).cloned().map(StringId)
            }
            Charset::Format0(ref array) => {
                if gid == 0 {
                    Some(StringId(0))
                } else {
                    array.get(gid - 1)
                }
            }
            Charset::Format1(array) => {
                if gid == 0 {
                    Some(StringId(0))
                } else {
                    let mut sid = gid - 1;
                    for range in *array {
                        if sid <= u16::from(range.left) {
                            sid = sid.checked_add(range.first.0)?;
                            return Some(StringId(sid));
                        }

                        sid = sid.checked_sub(u16::from(range.left) + 1)?;
                    }

                    None
                }
            }
            Charset::Format2(array) => {
                if gid == 0 {
                    Some(StringId(0))
                } else {
                    let mut sid = gid - 1;
                    for range in *array {
                        if sid <= range.left {
                            sid = sid.checked_add(range.first.0)?;
                            return Some(StringId(sid));
                        }

                        sid = sid.checked_sub(range.left.checked_add(1)?)?;
                    }

                    None
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Format1Range {
    first: StringId,
    left: u8,
}

impl Readable<'_> for Format1Range {
    const SIZE: usize = 3;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Format1Range {
            first: r.read::<StringId>()?,
            left: r.read::<u8>()?,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Format2Range {
    first: StringId,
    left: u16,
}

impl Readable<'_> for Format2Range {
    const SIZE: usize = 4;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Format2Range {
            first: r.read::<StringId>()?,
            left: r.read::<u16>()?,
        })
    }
}

pub(crate) fn parse_charset<'a>(
    number_of_glyphs: u16,
    r: &mut Reader<'a>,
) -> Option<Charset<'a>> {
    if number_of_glyphs < 2 {
        return None;
    }

    // -1 everywhere, since `.notdef` is omitted.
    let format = r.read::<u8>()?;
    match format {
        0 => Some(Charset::Format0(r.read_array16::<StringId>(number_of_glyphs - 1)?)),
        1 => {
            // The number of ranges is not defined, so we have to
            // read until no glyphs are left.
            let mut count = 0;
            {
                let mut s = r.clone();
                let mut total_left = number_of_glyphs - 1;
                while total_left > 0 {
                    s.skip::<StringId>(); // first
                    let left = s.read::<u8>()?;
                    total_left = total_left.checked_sub(u16::from(left) + 1)?;
                    count += 1;
                }
            }

            r.read_array16::<Format1Range>(count).map(Charset::Format1)
        }
        2 => {
            // The same as format 1, but Range::left is u16.
            let mut count = 0;
            {
                let mut s = r.clone();
                let mut total_left = number_of_glyphs - 1;
                while total_left > 0 {
                    s.skip::<StringId>(); // first
                    let left = s.read::<u16>()?.checked_add(1)?;
                    total_left = total_left.checked_sub(left)?;
                    count += 1;
                }
            }

            r.read_array16::<Format2Range>(count).map(Charset::Format2)
        }
        _ => None,
    }
}

pub mod charset_id {
    pub const ISO_ADOBE: usize = 0;
    pub const EXPERT: usize = 1;
    pub const EXPERT_SUBSET: usize = 2;
}

#[rustfmt::skip]
const EXPERT_ENCODING: &[u16] = &[
    0,    1,  229,  230,  231,  232,  233,  234,  235,  236,  237,  238,   13,   14,   15,   99,
    239,  240,  241,  242,  243,  244,  245,  246,  247,  248,   27,   28,  249,  250,  251,  252,
    253,  254,  255,  256,  257,  258,  259,  260,  261,  262,  263,  264,  265,  266,  109,  110,
    267,  268,  269,  270,  271,  272,  273,  274,  275,  276,  277,  278,  279,  280,  281,  282,
    283,  284,  285,  286,  287,  288,  289,  290,  291,  292,  293,  294,  295,  296,  297,  298,
    299,  300,  301,  302,  303,  304,  305,  306,  307,  308,  309,  310,  311,  312,  313,  314,
    315,  316,  317,  318,  158,  155,  163,  319,  320,  321,  322,  323,  324,  325,  326,  150,
    164,  169,  327,  328,  329,  330,  331,  332,  333,  334,  335,  336,  337,  338,  339,  340,
    341,  342,  343,  344,  345,  346,  347,  348,  349,  350,  351,  352,  353,  354,  355,  356,
    357,  358,  359,  360,  361,  362,  363,  364,  365,  366,  367,  368,  369,  370,  371,  372,
    373,  374,  375,  376,  377,  378,
];

/// The Expert Subset Encoding conversion as defined in the Adobe Technical Note #5176 Appendix C.
#[rustfmt::skip]
const EXPERT_SUBSET_ENCODING: &[u16] = &[
    0,    1,  231,  232,  235,  236,  237,  238,   13,   14,   15,   99,  239,  240,  241,  242,
    243,  244,  245,  246,  247,  248,   27,   28,  249,  250,  251,  253,  254,  255,  256,  257,
    258,  259,  260,  261,  262,  263,  264,  265,  266,  109,  110,  267,  268,  269,  270,  272,
    300,  301,  302,  305,  314,  315,  158,  155,  163,  320,  321,  322,  323,  324,  325,  326,
    150,  164,  169,  327,  328,  329,  330,  331,  332,  333,  334,  335,  336,  337,  338,  339,
    340,  341,  342,  343,  344,  345,  346
];

pub(crate) fn write_charset(
    sid_remapper: &SidRemapper,
    kind: &FontKind,
    charset: &Charset,
    gid_mapper: &GlyphRemapper,
) -> crate::Result<Vec<u8>> {
    let mut w = Writer::new();
    // Format 0
    w.write::<u8>(0);

    // Skip 0
    for old_gid in gid_mapper.remapped_gids().skip(1) {
        let original_sid = charset.gid_to_sid(old_gid).ok_or(MalformedFont)?;
        let new_sid = match kind {
            // For SID-keyed fonts, we need to find out the remapped SID
            FontKind::Sid(_) => sid_remapper.get(original_sid).ok_or(SubsetError)?,
            // For CID-keyed fonts, the SID actually represents CIDs, so it stays the same.
            FontKind::Cid(_) => original_sid,
        };
        w.write(new_sid)
    }

    Ok(w.finish())
}
