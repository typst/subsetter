mod charset;
mod dict;
mod encoding;
mod index;
mod private_dict;
pub(crate) mod subset;

use super::*;
use crate::cff::charset::{parse_charset, Charset};
use crate::cff::dict::DictionaryParser;
use crate::cff::encoding::Encoding;
use crate::cff::index::{parse_index, skip_index, Index};
use crate::cff::private_dict::parse_private_dict;
use crate::stream::StringId;
use crate::util::LazyArray16;

// Limits according to the Adobe Technical Note #5176, chapter 4 DICT Data.
const MAX_OPERANDS_LEN: usize = 48;

/// A [Compact Font Format Table](
/// https://docs.microsoft.com/en-us/typography/opentype/spec/cff).
#[derive(Clone)]
pub struct Table<'a> {
    table_data: &'a [u8],
    header: &'a [u8],
    names: &'a [u8],
    top_dict: TopDict,
    strings: Index<'a>,
    global_subrs: Index<'a>,
    charset: Charset<'a>,
    number_of_glyphs: u16,
    char_strings: Index<'a>,
    kind: Option<FontKind<'a>>,
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
        let top_dict = parse_top_dict(&mut r).ok_or(MalformedFont)?;

        let strings = parse_index::<u16>(&mut r).ok_or(MalformedFont)?;
        let global_subrs = parse_index::<u16>(&mut r).ok_or(MalformedFont)?;

        let char_strings_offset = top_dict.char_strings.ok_or(MalformedFont)?;
        let char_strings = {
            let mut r = Reader::new_at(cff, char_strings_offset);
            parse_index::<u16>(&mut r).ok_or(MalformedFont)?
        };

        let number_of_glyphs = u16::try_from(char_strings.len())
            .ok()
            .filter(|n| *n > 0)
            .ok_or(MalformedFont)?;

        let charset = match top_dict.charset {
            Some(charset_id::ISO_ADOBE) => Charset::ISOAdobe,
            Some(charset_id::EXPERT) => Charset::Expert,
            Some(charset_id::EXPERT_SUBSET) => Charset::ExpertSubset,
            Some(offset) => {
                let mut s = Reader::new_at(cff, offset);
                parse_charset(number_of_glyphs, &mut s).ok_or(MalformedFont)?
            }
            None => Charset::ISOAdobe, // default
        };

        let kind = if top_dict.ros.is_some() {
            parse_cid_metadata(cff, &top_dict, number_of_glyphs)
        } else {
            None
        };

        Ok(Self {
            table_data: cff,
            header,
            names,
            top_dict,
            strings,
            global_subrs,
            charset,
            number_of_glyphs,
            char_strings,
            kind,
        })
    }
}

pub(crate) fn discover(ctx: &mut Context) -> Result<()> {
    ctx.subset.insert(0);
    ctx.subset
        .extend(ctx.requested_glyphs.iter().filter(|g| **g < ctx.num_glyphs));
    Ok(())
}

#[derive(Default, Debug, Clone)]
struct TopDict {
    version: Option<StringId>,
    notice: Option<StringId>,
    copyright: Option<StringId>,
    full_name: Option<StringId>,
    family_name: Option<StringId>,
    weight: Option<StringId>,
    is_fixed_pitch: Option<bool>,
    italic_angle: Option<f64>,
    underline_position: Option<f64>,
    underline_thickness: Option<f64>,
    paint_type: Option<f64>,
    char_string_type: Option<f64>,
    font_matrix: Option<[f64; 6]>,
    unique_id: Option<f64>,
    font_bbox: Option<[f64; 4]>,
    stroke_width: Option<f64>,
    xuid: Option<Vec<f64>>,
    charset: Option<usize>,
    encoding: Option<usize>,
    char_strings: Option<usize>,
    private: Option<(usize, usize)>,
    synthetic_base: Option<f64>,
    postscript: Option<StringId>,
    base_font_name: Option<StringId>,
    base_font_blend: Option<Vec<f64>>,

    ros: Option<(StringId, StringId, f64)>,
    cid_font_version: Option<f64>,
    cid_font_revision: Option<f64>,
    cid_font_type: Option<f64>,
    cid_count: Option<f64>,
    uid_base: Option<f64>,
    fd_array: Option<usize>,
    fd_select: Option<usize>,
    font_name: Option<StringId>,
}

fn parse_top_dict<'a>(r: &mut Reader<'_>) -> Option<TopDict> {
    let mut top_dict = TopDict::default();

    let index = parse_index::<u16>(r)?;

    // The Top DICT INDEX should have only one dictionary.
    let data = index.get(0)?;

    let mut operands_buffer = [0.0; MAX_OPERANDS_LEN];
    let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);
    while let Some(operator) = dict_parser.parse_next() {
        match operator.get() {
            top_dict_operator::VERSION => top_dict.version = dict_parser.parse_sid(),
            top_dict_operator::NOTICE => top_dict.notice = dict_parser.parse_sid(),
            top_dict_operator::COPYRIGHT => top_dict.copyright = dict_parser.parse_sid(),
            top_dict_operator::FULL_NAME => top_dict.full_name = dict_parser.parse_sid(),
            top_dict_operator::FAMILY_NAME => {
                top_dict.family_name = dict_parser.parse_sid()
            }
            top_dict_operator::WEIGHT => top_dict.weight = dict_parser.parse_sid(),
            top_dict_operator::IS_FIXED_PITCH => {
                top_dict.is_fixed_pitch = dict_parser.parse_bool()
            }
            top_dict_operator::ITALIC_ANGLE => {
                top_dict.italic_angle = dict_parser.parse_number()
            }
            top_dict_operator::UNDERLINE_POSITION => {
                top_dict.underline_position = dict_parser.parse_number()
            }
            top_dict_operator::UNDERLINE_THICKNESS => {
                top_dict.underline_thickness = dict_parser.parse_number()
            }
            top_dict_operator::PAINT_TYPE => {
                top_dict.paint_type = dict_parser.parse_number()
            }
            top_dict_operator::CHAR_STRING_TYPE => {
                top_dict.char_string_type = dict_parser.parse_number()
            }
            top_dict_operator::FONT_MATRIX => {
                dict_parser.parse_operands()?;
                let operands = dict_parser.operands();

                if operands.len() == 6 {
                    top_dict.font_matrix = Some([
                        operands[0],
                        operands[1],
                        operands[2],
                        operands[3],
                        operands[4],
                        operands[5],
                    ])
                }
            }
            top_dict_operator::UNIQUE_ID => {
                top_dict.unique_id = dict_parser.parse_number()
            }
            top_dict_operator::FONT_BBOX => {
                dict_parser.parse_operands()?;
                let operands = dict_parser.operands();

                if operands.len() == 4 {
                    top_dict.font_bbox =
                        Some([operands[0], operands[1], operands[2], operands[3]])
                }
            }
            top_dict_operator::STROKE_WIDTH => {
                top_dict.stroke_width = dict_parser.parse_number()
            }
            top_dict_operator::XUID => {
                dict_parser.parse_operands()?;
                let operands = dict_parser.operands();

                top_dict.xuid = Some(operands.into())
            }
            top_dict_operator::CHARSET => top_dict.charset = dict_parser.parse_offset(),
            top_dict_operator::ENCODING => top_dict.encoding = dict_parser.parse_offset(),
            top_dict_operator::CHAR_STRINGS => {
                top_dict.char_strings = dict_parser.parse_offset()
            }
            top_dict_operator::PRIVATE => top_dict.private = dict_parser.parse_range(),
            top_dict_operator::SYNTHETIC_BASE => {
                top_dict.synthetic_base = dict_parser.parse_number()
            }
            top_dict_operator::POSTSCRIPT => {
                top_dict.postscript = dict_parser.parse_sid()
            }
            top_dict_operator::BASE_FONT_NAME => {
                top_dict.base_font_name = dict_parser.parse_sid()
            }
            top_dict_operator::BASE_FONT_BLEND => {
                dict_parser.parse_operands()?;
                let operands = dict_parser.operands();

                top_dict.base_font_blend = Some(operands.into())
            }
            top_dict_operator::ROS => {
                dict_parser.parse_operands()?;
                let operands = dict_parser.operands();

                if operands.len() == 3 {
                    top_dict.ros = Some((
                        StringId(u16::try_from(operands[0] as i64).ok()?),
                        StringId(u16::try_from(operands[1] as i64).ok()?),
                        operands[2],
                    ))
                }
            }
            top_dict_operator::CID_FONT_VERSION => {
                top_dict.cid_font_version = dict_parser.parse_number()
            }
            top_dict_operator::CID_FONT_REVISION => {
                top_dict.cid_font_revision = dict_parser.parse_number()
            }
            top_dict_operator::CID_FONT_TYPE => {
                top_dict.cid_font_type = dict_parser.parse_number()
            }
            top_dict_operator::CID_COUNT => {
                top_dict.cid_count = dict_parser.parse_number()
            }
            top_dict_operator::UID_BASE => top_dict.uid_base = dict_parser.parse_number(),
            top_dict_operator::FD_ARRAY => top_dict.fd_array = dict_parser.parse_offset(),
            top_dict_operator::FD_SELECT => {
                top_dict.fd_select = dict_parser.parse_offset()
            }
            top_dict_operator::FONT_NAME => top_dict.font_name = dict_parser.parse_sid(),
            _ => {}
        }
    }

    Some(top_dict)
}

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct CIDMetadata<'a> {
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
    top_dict: &TopDict,
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

    for el in metadata.fd_array {
        println!("{:?}", parse_private_dict(el));
    }

    metadata.fd_select = {
        let mut s = Reader::new_at(data, fd_select_offset);
        parse_fd_select(number_of_glyphs, &mut s)?
    };

    Some(FontKind::CID(metadata))
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
/// Table 9 Top DICT Operator Entries
mod top_dict_operator {
    pub const VERSION: u16 = 0;
    pub const NOTICE: u16 = 1;
    pub const COPYRIGHT: u16 = 1200;
    pub const FULL_NAME: u16 = 2;
    pub const FAMILY_NAME: u16 = 3;
    pub const WEIGHT: u16 = 4;
    pub const IS_FIXED_PITCH: u16 = 1201;
    pub const ITALIC_ANGLE: u16 = 1202;
    pub const UNDERLINE_POSITION: u16 = 1203;
    pub const UNDERLINE_THICKNESS: u16 = 1204;
    pub const PAINT_TYPE: u16 = 1205;
    pub const CHAR_STRING_TYPE: u16 = 1206;
    pub const FONT_MATRIX: u16 = 1207;
    pub const UNIQUE_ID: u16 = 13;
    pub const FONT_BBOX: u16 = 5;
    pub const STROKE_WIDTH: u16 = 1208;
    pub const XUID: u16 = 14;
    pub const CHARSET: u16 = 15;
    pub const ENCODING: u16 = 16;
    pub const CHAR_STRINGS: u16 = 17;
    pub const PRIVATE: u16 = 18;
    pub const SYNTHETIC_BASE: u16 = 1220;
    pub const POSTSCRIPT: u16 = 1221;
    pub const BASE_FONT_NAME: u16 = 1222;
    pub const BASE_FONT_BLEND: u16 = 1223;

    pub const ROS: u16 = 1230;
    pub const CID_FONT_VERSION: u16 = 1231;
    pub const CID_FONT_REVISION: u16 = 1232;
    pub const CID_FONT_TYPE: u16 = 1233;
    pub const CID_COUNT: u16 = 1234;
    pub const UID_BASE: u16 = 1235;
    pub const FD_ARRAY: u16 = 1236;
    pub const FD_SELECT: u16 = 1237;
    pub const FONT_NAME: u16 = 1238;
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
