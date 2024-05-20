use crate::cff::dict::DictionaryParser;
use crate::cff::index::{create_index, parse_index};
use crate::cff::remapper::SidRemapper;
use crate::cff::types::{Number, StringId};
use crate::cff::{dict, FontWriteContext};
use crate::read::Reader;
use crate::write::Writer;
use crate::Error::SubsetError;
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

pub(crate) fn write_top_dict_index(
    raw_top_dict: &[u8],
    font_write_context: &mut FontWriteContext,
    sid_remapper: &SidRemapper,
) -> crate::Result<Vec<u8>> {
    use super::operators::*;

    let mut w = Writer::new();
    let mut r = Reader::new(raw_top_dict);

    let index = parse_index::<u16>(&mut r).unwrap();

    // The Top DICT INDEX should have only one dictionary.
    let data = index.get(0).unwrap();

    let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
    let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);

    let mut write = |operands: &[u8], operator: &[u8]| {
        for operand in operands {
            w.write(*operand);
        }
        w.write(operator);
    };

    while let Some(operator) = dict_parser.parse_next() {
        match operator {
            CHARSET => {
                write(font_write_context.charset_offset.as_bytes(), operator.as_bytes())
            }
            ENCODING => {
                write(font_write_context.encoding_offset.as_bytes(), operator.as_bytes())
            }
            CHAR_STRINGS => write(
                font_write_context.char_strings_offset.as_bytes(),
                operator.as_bytes(),
            ),
            FD_ARRAY => {
                let Some(cid_context) = &font_write_context.cid_context else {
                    return Err(SubsetError);
                };
                write(cid_context.fd_array_offset.as_bytes(), operator.as_bytes())
            }
            FD_SELECT => {
                let Some(cid_context) = &font_write_context.cid_context else {
                    return Err(SubsetError);
                };
                write(cid_context.fd_select_offset.as_bytes(), operator.as_bytes())
            }
            VERSION | NOTICE | COPYRIGHT | FULL_NAME | FAMILY_NAME | WEIGHT
            | POSTSCRIPT | BASE_FONT_NAME | BASE_FONT_BLEND | FONT_NAME => {
                let sid = sid_remapper.get(dict_parser.parse_sid().unwrap()).unwrap();
                write(Number::from_i32(sid.0 as i32).as_bytes(), operator.as_bytes())
            }
            ROS => {
                dict_parser.parse_operands().unwrap();
                let operands = dict_parser.operands();

                let arg1 = sid_remapper
                    .get(StringId(u16::try_from(operands[0].as_u32().unwrap()).unwrap()))
                    .unwrap();
                let arg2 = sid_remapper
                    .get(StringId(u16::try_from(operands[1].as_u32().unwrap()).unwrap()))
                    .unwrap();

                let mut w = Writer::new();
                w.write(Number::from_i32(arg1.0 as i32).as_bytes());
                w.write(Number::from_i32(arg2.0 as i32).as_bytes());
                w.write(operands[2].as_bytes());
                write(&w.finish(), operator.as_bytes());
            }
            PRIVATE => {
                if let Some(offsets) = font_write_context.private_dicts_offsets.get(0) {
                    let mut op_w = Writer::new();
                    op_w.write(offsets.0.as_bytes());
                    op_w.write(offsets.1.as_bytes());

                    write(&op_w.finish(), PRIVATE.as_bytes());
                }
            }
            _ => {
                dict_parser.parse_operands().unwrap();
                let operands = dict_parser.operands();

                let mut w = Writer::new();

                for operand in operands {
                    w.write(operand.as_bytes());
                }

                write(&w.finish(), operator.as_bytes());
            }
        }
    }

    let finished = w.finish();

    let index = create_index(vec![finished])?;

    Ok(index)
}
