use crate::cff::dict;
use crate::cff::dict::private_dict::parse_subr_offset;
use crate::cff::dict::DictionaryParser;
use crate::cff::index::{parse_index, Index};
use crate::cff::types::{Number, StringId};
use crate::read::Reader;
use std::array;

#[derive(Default, Clone, Debug)]
pub(crate) struct FontDict<'a> {
    pub(crate) local_subrs: Index<'a>,
    pub(crate) used_sids: Vec<StringId>,
    pub(crate) private_dict: &'a [u8],
}

pub fn parse_font_dict<'a>(
    font_data: &'a [u8],
    font_dict_data: &[u8],
) -> Option<FontDict<'a>> {
    let mut font_dict = FontDict::default();

    let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
    let mut dict_parser = DictionaryParser::new(font_dict_data, &mut operands_buffer);
    while let Some(operator) = dict_parser.parse_next() {
        if operator == dict::operators::PRIVATE {
            let private_dict_range = dict_parser.parse_range()?;
            let private_dict_data = font_data.get(private_dict_range.clone())?;
            font_dict.private_dict = private_dict_data;
            font_dict.local_subrs = {
                let subrs_offset = parse_subr_offset(private_dict_data)?;
                let start = private_dict_range.start.checked_add(subrs_offset)?;
                let subrs_data = font_data.get(start..)?;
                let mut r = Reader::new(subrs_data);
                parse_index::<u16>(&mut r)?
            };
        } else if operator == dict::operators::FONT_NAME {
            font_dict.used_sids.push(dict_parser.parse_sid()?);
        }
    }

    Some(font_dict)
}
