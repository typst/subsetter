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
    pub registry_sid: Option<StringId>,
    pub ordering_sd: Option<StringId>,
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

                let registry = StringId(u16::try_from(operands[0].as_u32()?).ok()?);
                let ordering = StringId(u16::try_from(operands[1].as_u32()?).ok()?);
                top_dict.used_sids.insert(registry);
                top_dict.used_sids.insert(ordering);

                top_dict.has_ros = true;
                top_dict.registry_sid = Some(registry);
                top_dict.ordering_sd = Some(ordering);
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
    offsets: &mut Offsets,
    sid_remapper: &SidRemapper,
    w: &mut Writer,
) -> Result<()> {
    use super::operators::*;

    let mut sub_w = Writer::new();

    let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
    let mut dict_parser = DictionaryParser::new(raw_top_dict, &mut operands_buffer);

    assert_ne!(offsets.ordering_sid, StringId(0));
    assert_ne!(offsets.registry_sid, StringId(0));

    // Write ROS operator.
    sub_w.write(Number::from_i32(offsets.registry_sid.0 as i32));
    sub_w.write(Number::from_i32(offsets.ordering_sid.0 as i32));
    sub_w.write(Number::zero());
    sub_w.write(ROS);

    // Write FD_ARRAY
    offsets.fd_array_offset.update_location(sub_w.len() + w.len());
    DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
    sub_w.write(FD_ARRAY);

    // Write FD_SELECT
    offsets.fd_select_offset.update_location(sub_w.len() + w.len());
    DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
    sub_w.write(&FD_SELECT);

    // TODO: What about UIDBase and CIDCount?

    while let Some(operator) = dict_parser.parse_next() {
        match operator {
            // Important: When writing the offsets, we need to add the current length of w AND sub_w.
            CHARSET => {
                offsets.charset_offset.update_location(sub_w.len() + w.len());
                DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
                sub_w.write(operator)
            }
            ENCODING => {
                offsets.encoding_offset.update_location(sub_w.len() + w.len());
                DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
                sub_w.write(operator);
            }
            CHAR_STRINGS => {
                offsets.char_strings_offset.update_location(sub_w.len() + w.len());
                DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
                sub_w.write(operator);
            }
            FD_ARRAY => {
                // We already wrote this.
            }
            FD_SELECT => {
                // We already wrote this.
            }
            VERSION | NOTICE | COPYRIGHT | FULL_NAME | FAMILY_NAME | WEIGHT
            | POSTSCRIPT | BASE_FONT_NAME | BASE_FONT_BLEND | FONT_NAME => {
                let sid = sid_remapper.get(dict_parser.parse_sid().unwrap()).unwrap();
                sub_w.write(Number::from_i32(sid.0 as i32));
                sub_w.write(operator);
            }
            ROS => {
                // We already wrote the ROS operator.
            }
            PRIVATE => {
                // We convert SID-keyed fonts into CID-keyed fonts, so do not rewrite the
                // private dict. The private dict of the SID-keyed font will be written into the
                // font dict.
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
    offsets.charset_offset.adjust_location(index.header_size);
    offsets.char_strings_offset.adjust_location(index.header_size);
    offsets.encoding_offset.adjust_location(index.header_size);
    offsets.fd_array_offset.adjust_location(index.header_size);
    offsets.fd_select_offset.adjust_location(index.header_size);

    w.write(index);

    Ok(())
}
