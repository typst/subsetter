use crate::cff::cid_font::CIDMetadata;
use crate::cff::dict::private_dict::parse_subr_offset;
use crate::cff::dict::DictionaryParser;
use crate::cff::index::{create_index, parse_index, Index};
use crate::cff::number::{Number, StringId};
use crate::cff::remapper::{FontDictRemapper, SidRemapper};
use crate::cff::{dict, Offsets};
use crate::read::Reader;
use crate::write::Writer;
use crate::Error::{MalformedFont, SubsetError};
use crate::Result;
use std::array;

/// A font DICT.
#[derive(Default, Clone, Debug)]
pub struct FontDict<'a> {
    /// The local subroutines that are linked in the font DICT.
    pub local_subrs: Index<'a>,
    /// The underlying data of the private dict.
    pub private_dict: &'a [u8],
    /// The StringID of the font name in this font DICT, if it exists.
    pub font_name_sid: Option<StringId>,
}

/// Parse a font DICT.
pub fn parse_font_dict<'a>(
    font_data: &'a [u8],
    font_dict_data: &[u8],
) -> Option<FontDict<'a>> {
    let mut font_dict = FontDict::default();

    let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
    let mut dict_parser = DictionaryParser::new(font_dict_data, &mut operands_buffer);
    // TODO: Can a font dict include other operators as well? Wasn't able to find information
    // on that in the spec, and the CFF fonts I tried only included those two.
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

/// Write the new font DICT INDEX.
pub fn rewrite_font_dict_index(
    fd_remapper: &FontDictRemapper,
    sid_remapper: &SidRemapper,
    font_write_context: &mut Offsets,
    metadata: &CIDMetadata,
    w: &mut Writer,
) -> Result<()> {
    let mut dicts = vec![];

    for (new_df, old_df) in fd_remapper.sorted_iter().enumerate() {
        let new_df = new_df as u8;

        let dict = metadata.font_dicts.get(old_df as usize).ok_or(SubsetError)?;
        let mut w = Writer::new();

        // Write the font name, if applicable.
        if let Some(sid) = dict.font_name_sid {
            let new_sid = sid_remapper.get(sid).ok_or(MalformedFont)?;
            w.write(Number::from_i32(new_sid.0 as i32));
            w.write(dict::operators::FONT_NAME);
        }

        // Write the length and offset of the private dict.
        // Private dicts have already been written, so the offsets are already correct.
        font_write_context
            .private_dicts_lens
            .get(new_df as usize)
            .ok_or(SubsetError)?
            .value
            .write_as_5_bytes(&mut w);

        font_write_context
            .private_dicts_offsets
            .get_mut(new_df as usize)
            .ok_or(SubsetError)?
            .value
            .write_as_5_bytes(&mut w);

        w.write(dict::operators::PRIVATE);
        dicts.push(w.finish());
    }

    w.write(create_index(dicts)?);

    Ok(())
}
