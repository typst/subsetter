use crate::cff::dict::DictionaryParser;
use crate::cff::index::parse_index;
use crate::cff::MAX_OPERANDS_LEN;
use crate::stream::{Reader, StringId};
use std::ops::Range;

#[derive(Default, Debug, Clone)]
pub struct TopDict {
    pub(crate) version: Option<StringId>,
    pub(crate) notice: Option<StringId>,
    pub(crate) copyright: Option<StringId>,
    pub(crate) full_name: Option<StringId>,
    pub(crate) family_name: Option<StringId>,
    pub(crate) weight: Option<StringId>,
    pub(crate) is_fixed_pitch: Option<bool>,
    pub(crate) italic_angle: Option<f64>,
    pub(crate) underline_position: Option<f64>,
    pub(crate) underline_thickness: Option<f64>,
    pub(crate) paint_type: Option<f64>,
    pub(crate) char_string_type: Option<f64>,
    pub(crate) font_matrix: Option<[f64; 6]>,
    pub(crate) unique_id: Option<f64>,
    pub(crate) font_bbox: Option<[f64; 4]>,
    pub(crate) stroke_width: Option<f64>,
    pub(crate) xuid: Option<Vec<f64>>,
    pub(crate) charset: Option<usize>,
    pub(crate) encoding: Option<usize>,
    pub(crate) char_strings: Option<usize>,
    pub(crate) private: Option<Range<usize>>,
    pub(crate) synthetic_base: Option<f64>,
    pub(crate) postscript: Option<StringId>,
    pub(crate) base_font_name: Option<StringId>,
    pub(crate) base_font_blend: Option<Vec<f64>>,
    pub(crate) ros: Option<(StringId, StringId, f64)>,
    pub(crate) cid_font_version: Option<f64>,
    pub(crate) cid_font_revision: Option<f64>,
    pub(crate) cid_font_type: Option<f64>,
    pub(crate) cid_count: Option<f64>,
    pub(crate) uid_base: Option<f64>,
    pub(crate) fd_array: Option<usize>,
    pub(crate) fd_select: Option<usize>,
    pub(crate) font_name: Option<StringId>,
}

pub fn parse_top_dict<'a>(r: &mut Reader<'_>) -> Option<TopDict> {
    let mut top_dict = TopDict::default();

    let index = parse_index::<u16>(r)?;

    // The Top DICT INDEX should have only one dictionary.
    let data = index.get(0)?;

    let mut operands_buffer = [0.0; MAX_OPERANDS_LEN];
    let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);
    while let Some(operator) = dict_parser.parse_next() {
        match operator.get() {
            top_dict_operator::VERSION => {
                top_dict.version = Some(dict_parser.parse_sid()?)
            }
            top_dict_operator::NOTICE => top_dict.notice = Some(dict_parser.parse_sid()?),
            top_dict_operator::COPYRIGHT => {
                top_dict.copyright = Some(dict_parser.parse_sid()?)
            }
            top_dict_operator::FULL_NAME => {
                top_dict.full_name = Some(dict_parser.parse_sid()?)
            }
            top_dict_operator::FAMILY_NAME => {
                top_dict.family_name = Some(dict_parser.parse_sid()?)
            }
            top_dict_operator::WEIGHT => top_dict.weight = Some(dict_parser.parse_sid()?),
            top_dict_operator::IS_FIXED_PITCH => {
                top_dict.is_fixed_pitch = Some(dict_parser.parse_bool()?)
            }
            top_dict_operator::ITALIC_ANGLE => {
                top_dict.italic_angle = Some(dict_parser.parse_number()?)
            }
            top_dict_operator::UNDERLINE_POSITION => {
                top_dict.underline_position = Some(dict_parser.parse_number()?)
            }
            top_dict_operator::UNDERLINE_THICKNESS => {
                top_dict.underline_thickness = Some(dict_parser.parse_number()?)
            }
            top_dict_operator::PAINT_TYPE => {
                top_dict.paint_type = Some(dict_parser.parse_number()?)
            }
            top_dict_operator::CHAR_STRING_TYPE => {
                top_dict.char_string_type = Some(dict_parser.parse_number()?)
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
                } else {
                    return None;
                }
            }
            top_dict_operator::UNIQUE_ID => {
                top_dict.unique_id = Some(dict_parser.parse_number()?)
            }
            top_dict_operator::FONT_BBOX => {
                dict_parser.parse_operands()?;
                let operands = dict_parser.operands();

                if operands.len() == 4 {
                    top_dict.font_bbox =
                        Some([operands[0], operands[1], operands[2], operands[3]])
                } else {
                    return None;
                }
            }
            top_dict_operator::STROKE_WIDTH => {
                top_dict.stroke_width = Some(dict_parser.parse_number()?)
            }
            top_dict_operator::XUID => top_dict.xuid = Some(dict_parser.parse_delta()?),
            top_dict_operator::CHARSET => {
                top_dict.charset = Some(dict_parser.parse_offset()?)
            }
            top_dict_operator::ENCODING => {
                top_dict.encoding = Some(dict_parser.parse_offset()?)
            }
            top_dict_operator::CHAR_STRINGS => {
                top_dict.char_strings = Some(dict_parser.parse_offset()?)
            }
            top_dict_operator::PRIVATE => {
                top_dict.private = Some(dict_parser.parse_range()?)
            }
            top_dict_operator::SYNTHETIC_BASE => {
                top_dict.synthetic_base = Some(dict_parser.parse_number()?)
            }
            top_dict_operator::POSTSCRIPT => {
                top_dict.postscript = Some(dict_parser.parse_sid()?)
            }
            top_dict_operator::BASE_FONT_NAME => {
                top_dict.base_font_name = Some(dict_parser.parse_sid()?)
            }
            top_dict_operator::BASE_FONT_BLEND => {
                top_dict.base_font_blend = Some(dict_parser.parse_delta()?)
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
                top_dict.cid_font_version = Some(dict_parser.parse_number()?)
            }
            top_dict_operator::CID_FONT_REVISION => {
                top_dict.cid_font_revision = Some(dict_parser.parse_number()?)
            }
            top_dict_operator::CID_FONT_TYPE => {
                top_dict.cid_font_type = Some(dict_parser.parse_number()?)
            }
            top_dict_operator::CID_COUNT => {
                top_dict.cid_count = Some(dict_parser.parse_number()?)
            }
            top_dict_operator::UID_BASE => {
                top_dict.uid_base = Some(dict_parser.parse_number()?)
            }
            top_dict_operator::FD_ARRAY => {
                top_dict.fd_array = Some(dict_parser.parse_offset()?)
            }
            top_dict_operator::FD_SELECT => {
                top_dict.fd_select = Some(dict_parser.parse_offset()?)
            }
            top_dict_operator::FONT_NAME => {
                top_dict.font_name = Some(dict_parser.parse_sid()?)
            }
            _ => {
                // Invalid operator
                return None;
            }
        }
    }

    Some(top_dict)
}

/// Enumerates some operators defined in the Adobe Technical Note #5176,
/// Table 9 Top DICT Operator Entries
pub mod top_dict_operator {
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
