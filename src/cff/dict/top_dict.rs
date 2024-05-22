use crate::cff::dict::DictionaryParser;
use crate::cff::index::{create_index, parse_index};
use crate::cff::number::{Number, StringId};
use crate::cff::remapper::SidRemapper;
use crate::cff::{Offsets, DUMMY_VALUE};
use crate::read::Reader;
use crate::write::Writer;
use crate::Result;
use std::array;
use std::collections::BTreeSet;
use std::ops::Range;

/// Data parsed from a top dict.
#[derive(Default, Debug, Clone)]
pub struct TopDictData<'a> {
    pub top_dict_raw: &'a [u8],
    pub used_sids: BTreeSet<StringId>,
    pub charset: Option<usize>,
    pub encoding: Option<usize>,
    pub char_strings: Option<usize>,
    pub private: Option<Range<usize>>,
    pub fd_array: Option<usize>,
    pub fd_select: Option<usize>,
    pub has_ros: bool,
}

/// Parse the top dict and extract relevant data.
pub fn parse_top_dict_index<'a>(r: &mut Reader<'a>) -> Option<TopDictData<'a>> {
    use super::operators::*;
    let mut top_dict = TopDictData::default();

    let index = parse_index::<u16>(r)?;

    // The Top DICT INDEX should have only one dictionary in CFF fonts.
    let data = index.get(0)?;
    top_dict.top_dict_raw = data;

    let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
    let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);

    while let Some(operator) = dict_parser.parse_next() {
        match operator {
            // Grab the SIDs so that we can remap them.
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

/// Rewrite the top dict. Essentially, the two things we need to do are
/// 1. Rewrite every operator that takes a SID with the new, remapped SID.
/// 2. Update all offsets.
pub fn rewrite_top_dict_index(
    raw_top_dict: &[u8],
    font_write_context: &mut Offsets,
    sid_remapper: &SidRemapper,
    w: &mut Writer,
) -> Result<()> {
    use super::operators::*;

    let mut sub_w = Writer::new();

    let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
    let mut dict_parser = DictionaryParser::new(raw_top_dict, &mut operands_buffer);

    while let Some(operator) = dict_parser.parse_next() {
        match operator {
            // Important: When writing the offsets, we need to add the current length of w AND sub_w.
            CHARSET => {
                font_write_context
                    .charset_offset
                    .update_location(sub_w.len() + w.len());
                DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
                sub_w.write(operator)
            }
            ENCODING => {
                font_write_context
                    .encoding_offset
                    .update_location(sub_w.len() + w.len());
                DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
                sub_w.write(operator);
            }
            CHAR_STRINGS => {
                font_write_context
                    .char_strings_offset
                    .update_location(sub_w.len() + w.len());
                DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
                sub_w.write(operator);
            }
            FD_ARRAY => {
                font_write_context
                    .fd_array_offset
                    .update_location(sub_w.len() + w.len());
                DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
                sub_w.write(operator);
            }
            FD_SELECT => {
                font_write_context
                    .fd_select_offset
                    .update_location(sub_w.len() + w.len());
                DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
                sub_w.write(&operator);
            }
            VERSION | NOTICE | COPYRIGHT | FULL_NAME | FAMILY_NAME | WEIGHT
            | POSTSCRIPT | BASE_FONT_NAME | BASE_FONT_BLEND | FONT_NAME => {
                let sid = sid_remapper.get(dict_parser.parse_sid().unwrap()).unwrap();
                sub_w.write(Number::from_i32(sid.0 as i32));
                sub_w.write(operator);
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

                sub_w.write(Number::from_i32(arg1.0 as i32));
                sub_w.write(Number::from_i32(arg2.0 as i32));
                sub_w.write(&operands[2]);
                sub_w.write(operator);
            }
            // TODO: Unify SID with CID?
            PRIVATE => {
                if let (Some(lens), Some(offsets)) = (
                    font_write_context.private_dicts_lens.first_mut(),
                    font_write_context.private_dicts_offsets.first_mut(),
                ) {
                    lens.update_location(sub_w.len() + w.len());
                    DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
                    offsets.update_location(sub_w.len() + w.len());
                    DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
                    sub_w.write(PRIVATE);
                }
            }
            _ => {
                dict_parser.parse_operands().unwrap();
                let operands = dict_parser.operands();

                sub_w.write(operands);
                sub_w.write(operator);
            }
        }
    }

    let finished = sub_w.finish();

    // TOP DICT INDEX always has size 1 in CFF.
    let index = create_index(vec![finished])?;

    // This is important: The offsets we calculated before were calculated under the assumption
    // that the contents of sub_w will be appended directly to w. However, when we create an index,
    // the INDEX header data will be appended in the beginning, meaning that we need to adjust the offsets
    // to account for that.
    font_write_context.charset_offset.adjust_location(index.header_size);
    font_write_context
        .char_strings_offset
        .adjust_location(index.header_size);
    font_write_context.encoding_offset.adjust_location(index.header_size);
    font_write_context.fd_array_offset.adjust_location(index.header_size);
    font_write_context.fd_select_offset.adjust_location(index.header_size);

    // TODO: This might not be entirely correct.
    if let (Some(lens), Some(offsets)) = (
        font_write_context.private_dicts_lens.first_mut(),
        font_write_context.private_dicts_offsets.first_mut(),
    ) {
        lens.adjust_location(index.header_size);
        offsets.adjust_location(index.header_size);
    }

    w.write(index);

    Ok(())
}
