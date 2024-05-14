// Acknowledgement: Most of this code has been adapted from
// ttf-parser.

mod argstack;
mod charset;
mod dict;
mod encoding;
mod index;
mod private_dict;
mod top_dict;
// mod subset;

use super::*;
use crate::cff::charset::{parse_charset, Charset};
use crate::cff::dict::{DictionaryParser, Number};
use crate::cff::encoding::Encoding;
use crate::cff::index::{parse_index, skip_index, Index};
use crate::cff::private_dict::parse_subr_offset;
use crate::util::LazyArray16;
use std::array;
use std::collections::HashMap;
use std::ops::Range;
use top_dict::{top_dict_operator, TopDictData};

// Limits according to the Adobe Technical Note #5176, chapter 4 DICT Data.
const MAX_OPERANDS_LEN: usize = 48;
const MAX_ARGUMENTS_STACK_LEN: usize = 513;

/// A [Compact Font Format Table](
/// https://docs.microsoft.com/en-us/typography/opentype/spec/cff).
#[derive(Clone)]
pub struct Table<'a> {
    table_data: &'a [u8],
    header: &'a [u8],
    names: &'a [u8],
    top_dict_data: TopDictData,
    strings: Index<'a>,
    global_subrs: Index<'a>,
    charset: Charset<'a>,
    number_of_glyphs: u16,
    char_strings: Index<'a>,
    kind: Option<FontKind<'a>>,
}

#[derive(Debug, Clone)]
pub struct Remapper {
    counter: u16,
    forward: HashMap<u16, u16>,
}

impl Remapper {
    pub fn new() -> Self {
        let mut mapper = Self { counter: 0, forward: HashMap::new() };

        mapper
    }

    pub fn get(&self, old_gid: u16) -> Option<u16> {
        self.forward.get(&old_gid).copied()
    }

    pub fn remap(&mut self, gid: u16) -> u16 {
        *self.forward.entry(gid).or_insert_with(|| {
            let value = self.counter;
            self.counter += 1;
            value
        })
    }
}

pub fn subset<'a>(ctx: &mut Context<'a>) {
    let table = Table::parse(ctx).unwrap();

    let Some(FontKind::CID(kind)) = table.kind else {
        return;
    };

    let mut gsubr_remapper = Remapper::new();
    let mut lsubr_remapper = vec![Remapper::new(); kind.local_subrs.len()];
}

impl<'a> Table<'a> {
    pub fn parse(ctx: &mut Context<'a>) -> Result<Self> {
        let cff = ctx.expect_table(Tag::CFF).ok_or(MalformedFont)?;

        let mut r = Reader::new(cff);

        let major = r.read::<u8>().ok_or(MalformedFont)?;

        if major != 1 {
            return Err(Error::Unimplemented);
        }

        r.skip::<u8>(); // minor
        let header_size = r.read::<u8>().ok_or(MalformedFont)?;
        let header = cff.get(0..header_size as usize).ok_or(MalformedFont)?;

        r.jump(header_size as usize);

        let names_start = r.offset();
        skip_index::<u16>(&mut r).ok_or(MalformedFont)?;
        let names = cff.get(names_start..r.offset()).ok_or(MalformedFont)?;
        let top_dict_data = top_dict::parse_top_dict(&mut r).ok_or(MalformedFont)?;

        println!("{:?}", top_dict_data);

        let strings = parse_index::<u16>(&mut r).ok_or(MalformedFont)?;
        let global_subrs = parse_index::<u16>(&mut r).ok_or(MalformedFont)?;

        let char_strings_offset = top_dict_data.char_strings.ok_or(MalformedFont)?;
        let char_strings = {
            let mut r = Reader::new_at(cff, char_strings_offset);
            parse_index::<u16>(&mut r).ok_or(MalformedFont)?
        };

        let number_of_glyphs = u16::try_from(char_strings.len())
            .ok()
            .filter(|n| *n > 0)
            .ok_or(MalformedFont)?;

        let charset = match top_dict_data.charset {
            Some(charset_id::ISO_ADOBE) => Charset::ISOAdobe,
            Some(charset_id::EXPERT) => Charset::Expert,
            Some(charset_id::EXPERT_SUBSET) => Charset::ExpertSubset,
            Some(offset) => {
                let mut s = Reader::new_at(cff, offset);
                parse_charset(number_of_glyphs, &mut s).ok_or(MalformedFont)?
            }
            None => Charset::ISOAdobe, // default
        };

        let kind = if top_dict_data.has_ros {
            parse_cid_metadata(cff, &top_dict_data, number_of_glyphs)
        } else {
            None
        };

        Ok(Self {
            table_data: cff,
            header,
            names,
            top_dict_data,
            strings,
            global_subrs,
            charset,
            number_of_glyphs,
            char_strings,
            kind,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) enum FontKind<'a> {
    SID(SIDMetadata<'a>),
    CID(CIDMetadata<'a>),
}

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct SIDMetadata<'a> {
    local_subrs: Index<'a>,
    /// Can be zero.
    default_width: f32,
    /// Can be zero.
    nominal_width: f32,
    encoding: Encoding<'a>,
}

#[derive(Clone, Default, Debug)]
pub(crate) struct CIDMetadata<'a> {
    local_subrs: Vec<Option<Index<'a>>>,
    fd_array: Index<'a>,
    fd_select: FDSelect<'a>,
}

#[derive(Clone, Copy, Debug)]
enum FDSelect<'a> {
    Format0(LazyArray16<'a, u8>),
    Format3(&'a [u8]), // It's easier to parse it in-place.
}

impl Default for FDSelect<'_> {
    fn default() -> Self {
        FDSelect::Format0(LazyArray16::default())
    }
}

impl FDSelect<'_> {
    fn font_dict_index(&self, glyph_id: u16) -> Option<u8> {
        match self {
            FDSelect::Format0(ref array) => array.get(glyph_id),
            FDSelect::Format3(data) => {
                let mut r = Reader::new(data);
                let number_of_ranges = r.read::<u16>()?;
                if number_of_ranges == 0 {
                    return None;
                }

                // 'A sentinel GID follows the last range element and serves
                // to delimit the last range in the array.'
                // So we can simply increase the number of ranges by one.
                let number_of_ranges = number_of_ranges.checked_add(1)?;

                // Range is: GlyphId + u8
                let mut prev_first_glyph = r.read::<u16>()?;
                let mut prev_index = r.read::<u8>()?;
                for _ in 1..number_of_ranges {
                    let curr_first_glyph = r.read::<u16>()?;
                    if (prev_first_glyph..curr_first_glyph).contains(&glyph_id) {
                        return Some(prev_index);
                    } else {
                        prev_index = r.read::<u8>()?;
                    }

                    prev_first_glyph = curr_first_glyph;
                }

                None
            }
        }
    }
}

fn parse_cid_metadata<'a>(
    data: &'a [u8],
    top_dict: &TopDictData,
    number_of_glyphs: u16,
) -> Option<FontKind<'a>> {
    let (charset_offset, fd_array_offset, fd_select_offset) =
        match (top_dict.charset, top_dict.fd_array, top_dict.fd_select) {
            (Some(a), Some(b), Some(c)) => (a, b, c),
            _ => return None, // charset, FDArray and FDSelect must be set.
        };

    if charset_offset <= charset_id::EXPERT_SUBSET {
        // 'There are no predefined charsets for CID fonts.'
        // Adobe Technical Note #5176, chapter 18 CID-keyed Fonts
        return None;
    }

    let mut metadata = CIDMetadata::default();

    metadata.fd_array = {
        let mut r = Reader::new_at(data, fd_array_offset);
        parse_index::<u16>(&mut r)?
    };

    for font_dict_data in metadata.fd_array {
        metadata
            .local_subrs
            .push(parse_cid_private_dict(data, font_dict_data));
    }

    metadata.fd_select = {
        let mut s = Reader::new_at(data, fd_select_offset);
        parse_fd_select(number_of_glyphs, &mut s)?
    };

    Some(FontKind::CID(metadata))
}

fn parse_cid_private_dict<'a>(
    data: &'a [u8],
    font_dict_data: &'a [u8],
) -> Option<Index<'a>> {
    let private_dict_range = parse_font_dict(font_dict_data)?;
    let private_dict_data = data.get(private_dict_range.clone())?;
    let subrs_offset = parse_subr_offset(private_dict_data)?;

    let start = private_dict_range.start.checked_add(subrs_offset)?;
    let subrs_data = data.get(start..)?;
    let mut r = Reader::new(subrs_data);
    parse_index::<u16>(&mut r)
}

fn parse_font_dict(data: &[u8]) -> Option<Range<usize>> {
    let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
    let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);
    while let Some(operator) = dict_parser.parse_next() {
        if operator.get() == top_dict_operator::PRIVATE {
            return dict_parser.parse_range();
        }
    }

    None
}

fn parse_fd_select<'a>(
    number_of_glyphs: u16,
    r: &mut Reader<'a>,
) -> Option<FDSelect<'a>> {
    let format = r.read::<u8>()?;
    match format {
        0 => Some(FDSelect::Format0(r.read_array16::<u8>(number_of_glyphs)?)),
        3 => Some(FDSelect::Format3(r.tail()?)),
        _ => None,
    }
}

/// Enumerates Charset IDs defined in the Adobe Technical Note #5176, Table 22
mod charset_id {
    pub const ISO_ADOBE: usize = 0;
    pub const EXPERT: usize = 1;
    pub const EXPERT_SUBSET: usize = 2;
}

/// Enumerates Charset IDs defined in the Adobe Technical Note #5176, Table 16
mod encoding_id {
    pub const STANDARD: usize = 0;
    pub const EXPERT: usize = 1;
}

/// Enumerates some operators defined in the Adobe Technical Note #5176,
/// Table 23 Private DICT Operators
mod private_dict_operator {
    pub const BLUE_VALUES: u16 = 6;
    pub const OTHER_BLUES: u16 = 7;
    pub const FAMILY_BLUES: u16 = 8;
    pub const FAMILY_OTHER_BLUES: u16 = 9;
    pub const BLUE_SCALE: u16 = 1209;
    pub const BLUE_SHIFT: u16 = 1210;
    pub const BLUE_FUZZ: u16 = 1211;
    pub const STD_HW: u16 = 10;
    pub const STD_VW: u16 = 11;
    pub const STEM_SNAP_H: u16 = 1212;
    pub const STEM_SNAP_V: u16 = 1213;
    pub const FORCE_BOLD: u16 = 1214;
    pub const LANGUAGE_GROUP: u16 = 1217;
    pub const EXPANSION_FACTOR: u16 = 1218;
    pub const INITIAL_RANDOM_SEED: u16 = 1219;
    pub const SUBRS: u16 = 19;
    pub const DEFAULT_WIDTH_X: u16 = 20;
    pub const NOMINAL_WIDTH_X: u16 = 21;
}

mod operator {
    pub const HORIZONTAL_STEM: u8 = 1;
    pub const VERTICAL_STEM: u8 = 3;
    pub const VERTICAL_MOVE_TO: u8 = 4;
    pub const LINE_TO: u8 = 5;
    pub const HORIZONTAL_LINE_TO: u8 = 6;
    pub const VERTICAL_LINE_TO: u8 = 7;
    pub const CURVE_TO: u8 = 8;
    pub const CALL_LOCAL_SUBROUTINE: u8 = 10;
    pub const RETURN: u8 = 11;
    pub const ENDCHAR: u8 = 14;
    pub const HORIZONTAL_STEM_HINT_MASK: u8 = 18;
    pub const HINT_MASK: u8 = 19;
    pub const COUNTER_MASK: u8 = 20;
    pub const MOVE_TO: u8 = 21;
    pub const HORIZONTAL_MOVE_TO: u8 = 22;
    pub const VERTICAL_STEM_HINT_MASK: u8 = 23;
    pub const CURVE_LINE: u8 = 24;
    pub const LINE_CURVE: u8 = 25;
    pub const VV_CURVE_TO: u8 = 26;
    pub const HH_CURVE_TO: u8 = 27;
    pub const SHORT_INT: u8 = 28;
    pub const CALL_GLOBAL_SUBROUTINE: u8 = 29;
    pub const VH_CURVE_TO: u8 = 30;
    pub const HV_CURVE_TO: u8 = 31;
    pub const HFLEX: u8 = 34;
    pub const FLEX: u8 = 35;
    pub const HFLEX1: u8 = 36;
    pub const FLEX1: u8 = 37;
    pub const FIXED_16_16: u8 = 255;
}
