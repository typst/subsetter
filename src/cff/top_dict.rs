use crate::cff::dict::DictionaryParser;
use crate::cff::index::parse_index;
use crate::cff::number::Number;
use crate::stream::{Reader, StringId};
use std::array;
use std::collections::BTreeSet;
use std::ops::Range;

#[derive(Default, Debug, Clone)]
pub struct TopDictData {
    pub(crate) used_sids: BTreeSet<StringId>,
    // pub(crate) version: Option<StringId>,
    // pub(crate) notice: Option<StringId>,
    // pub(crate) copyright: Option<StringId>,
    // pub(crate) full_name: Option<StringId>,
    // pub(crate) family_name: Option<StringId>,
    // pub(crate) weight: Option<StringId>,
    pub(crate) charset: Option<usize>,
    pub(crate) encoding: Option<usize>,
    pub(crate) char_strings: Option<usize>,
    pub(crate) private: Option<Range<usize>>,
    // pub(crate) postscript: Option<StringId>,
    // pub(crate) base_font_name: Option<StringId>,
    // pub(crate) ros: Option<(StringId, StringId, f64)>,
    pub(crate) fd_array: Option<usize>,
    pub(crate) fd_select: Option<usize>,
    pub(crate) has_ros: bool, // pub(crate) font_name: Option<StringId>,
}

pub fn parse_top_dict<'a>(r: &mut Reader<'_>) -> Option<TopDictData> {
    use top_dict_operator::*;
    let mut top_dict = TopDictData::default();

    let index = parse_index::<u16>(r)?;

    // The Top DICT INDEX should have only one dictionary.
    let data = index.get(0)?;

    let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
    let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);

    while let Some(operator) = dict_parser.parse_next() {
        match operator.get() {
            VERSION | NOTICE | COPYRIGHT | FULL_NAME | FAMILY_NAME | WEIGHT
            | POSTSCRIPT | BASE_FONT_NAME | BASE_FONT_BLEND | FONT_NAME => {
                let sid = dict_parser.parse_sid()?;
                top_dict.used_sids.insert(sid);
            }
            CHARSET => top_dict.charset = Some(dict_parser.parse_offset()?),
            ENCODING => top_dict.encoding = Some(dict_parser.parse_offset()?),
            CHAR_STRINGS => top_dict.char_strings = Some(dict_parser.parse_offset()?),
            PRIVATE => top_dict.private = Some(dict_parser.parse_range()?),
            ROS => {
                dict_parser.parse_operands()?;
                let operands = dict_parser.operands();

                top_dict
                    .used_sids
                    .insert(StringId(u16::try_from(operands[0].as_u32()?).ok()?));
                top_dict
                    .used_sids
                    .insert(StringId(u16::try_from(operands[1].as_u32()?).ok()?));
                top_dict.has_ros = true;
            }
            FD_ARRAY => top_dict.fd_array = Some(dict_parser.parse_offset()?),
            FD_SELECT => top_dict.fd_select = Some(dict_parser.parse_offset()?),
            _ => {}
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
