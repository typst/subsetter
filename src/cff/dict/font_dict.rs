use crate::cff::cid_font::CIDMetadata;
use crate::cff::dict::private_dict::parse_subr_offset;
use crate::cff::dict::DictionaryParser;
use crate::cff::index::{create_index, parse_index, Index};
use crate::cff::number::{Number, StringId};
use crate::cff::operator::Operator;
use crate::cff::remapper::{FontDictRemapper, SidRemapper};
use crate::cff::{dict, FontWriteContext};
use crate::read::Reader;
use crate::write::Writer;
use crate::Error::{MalformedFont, SubsetError};
use crate::Result;
use std::array;

#[derive(Default, Clone, Debug)]
pub(crate) struct FontDict<'a> {
    pub(crate) local_subrs: Index<'a>,
    pub(crate) private_dict: &'a [u8],
    pub(crate) font_name_sid: Option<StringId>,
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
            font_dict.font_name_sid = Some(dict_parser.parse_sid()?);
        }
    }

    Some(font_dict)
}

pub(crate) fn write_font_dict_index(
    fd_remapper: &FontDictRemapper,
    sid_remapper: &SidRemapper,
    font_write_context: &mut FontWriteContext,
    metadata: &CIDMetadata,
) -> Result<Vec<u8>> {
    let mut dicts = vec![];

    for (new_df, old_df) in fd_remapper.sorted_iter().enumerate() {
        let new_df = new_df as u8;

        let dict = metadata.font_dicts.get(old_df as usize).ok_or(SubsetError)?;
        let mut w = Writer::new();

        if let Some(sid) = dict.font_name_sid {
            let new_sid = sid_remapper.get(sid).ok_or(MalformedFont)?;
            w.write(Number::from_i32(new_sid.0 as i32));
            w.write(dict::operators::FONT_NAME);
        }

        // TODO: Offsets can be u32?
        let private_dict_offset = font_write_context
            .private_dicts_offsets
            .get(new_df as usize)
            .ok_or(SubsetError)?;

        private_dict_offset.0.write_as_5_bytes(&mut w);
        private_dict_offset.1.write_as_5_bytes(&mut w);
        w.write(dict::operators::PRIVATE);
        dicts.push(w.finish());
    }

    create_index(dicts)
}
