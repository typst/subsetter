mod charset;
mod dict;
mod index;

use super::*;
use crate::cff::charset::{parse_charset, Charset};
use crate::cff::dict::DictionaryParser;
use crate::cff::index::{parse_index, Index};
use crate::stream::StringId;
use std::num::NonZeroU16;
use std::ops::Range;

// Limits according to the Adobe Technical Note #5176, chapter 4 DICT Data.
const MAX_OPERANDS_LEN: usize = 48;

/// A [Compact Font Format Table](
/// https://docs.microsoft.com/en-us/typography/opentype/spec/cff).
#[derive(Clone)]
pub struct Table<'a> {
    table_data: &'a [u8],
    header: &'a [u8],
    names: Index<'a>,
    top_dict: TopDict,
    strings: Index<'a>,
    global_subrs: Index<'a>,
    charset: Charset<'a>,
    // number_of_glyphs: NonZeroU16,
    // matrix: Matrix,
    // char_strings: Index<'a>,
    // kind: FontKind<'a>,
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

        let names = parse_index::<u16>(&mut r).ok_or(MalformedFont)?;
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

        Ok(Self {
            table_data: cff,
            header,
            names,
            top_dict,
            strings,
            global_subrs,
            charset,
        })
    }
}

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let table = Table::parse(ctx)?;

    Ok(())
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

/// Enumerates Charset IDs defined in the Adobe Technical Note #5176, Table 22
mod charset_id {
    pub const ISO_ADOBE: usize = 0;
    pub const EXPERT: usize = 1;
    pub const EXPERT_SUBSET: usize = 2;
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
