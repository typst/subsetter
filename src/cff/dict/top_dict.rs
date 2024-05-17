use crate::cff::dict::DictionaryParser;
use crate::cff::index::parse_index;
use crate::cff::types::{Number, StringId};
use crate::read::Reader;
use std::array;
use std::collections::BTreeSet;
use std::ops::Range;

#[derive(Default, Debug, Clone)]
pub struct TopDictData {
    pub(crate) used_sids: BTreeSet<StringId>,
    pub(crate) charset: Option<usize>,
    pub(crate) encoding: Option<usize>,
    pub(crate) char_strings: Option<usize>,
    pub(crate) private: Option<Range<usize>>,
    pub(crate) fd_array: Option<usize>,
    pub(crate) fd_select: Option<usize>,
    pub(crate) has_ros: bool, // pub(crate) font_name: Option<StringId>,
}

pub fn parse_top_dict<'a>(r: &mut Reader<'_>) -> Option<TopDictData> {
    use super::operators::*;
    let mut top_dict = TopDictData::default();

    let index = parse_index::<u16>(r)?;

    // The Top DICT INDEX should have only one dictionary.
    let data = index.get(0)?;

    let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
    let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);

    while let Some(operator) = dict_parser.parse_next() {
        match operator {
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
