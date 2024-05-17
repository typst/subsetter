use crate::cff::dict::private_dict::parse_subr_offset;
use crate::cff::dict::top_dict::TopDictData;
use crate::cff::dict::{top_dict, DictionaryParser};
use crate::cff::index::{parse_index, Index};
use crate::cff::types::Number;
use crate::cff::{charset_id, dict, CIDMetadata, FDSelect, FontKind};
use crate::read::Reader;
use std::array;
use std::ops::Range;

pub fn parse_cid_metadata<'a>(
    data: &'a [u8],
    top_dict: &TopDictData,
    number_of_glyphs: u16,
) -> Option<FontKind<'a>> {
    let (charset_offset, fd_array_offset, fd_select_offset) =
        match (top_dict.charset, top_dict.fd_array, top_dict.fd_select) {
            (Some(a), Some(b), Some(c)) => (a, b, c),
            _ => return None, // charset, FDArray and FDSelect must be set.
        };

    if charset_offset <= charset_id::EXPERT_SUBSET {
        // 'There are no predefined charsets for CID fonts.'
        // Adobe Technical Note #5176, chapter 18 CID-keyed Fonts
        return None;
    }

    let mut metadata = CIDMetadata::default();

    metadata.fd_array = {
        let mut r = Reader::new_at(data, fd_array_offset);
        parse_index::<u16>(&mut r)?
    };

    for font_dict_data in metadata.fd_array {
        metadata
            .local_subrs
            .push(parse_cid_private_dict(data, font_dict_data).unwrap_or_default());
    }

    metadata.fd_select = {
        let mut s = Reader::new_at(data, fd_select_offset);
        parse_fd_select(number_of_glyphs, &mut s)?
    };

    Some(FontKind::CID(metadata))
}

fn parse_cid_private_dict<'a>(
    data: &'a [u8],
    font_dict_data: &'a [u8],
) -> Option<Index<'a>> {
    let private_dict_range = parse_font_dict(font_dict_data)?;
    let private_dict_data = data.get(private_dict_range.clone())?;
    let subrs_offset = parse_subr_offset(private_dict_data)?;

    let start = private_dict_range.start.checked_add(subrs_offset)?;
    let subrs_data = data.get(start..)?;
    let mut r = Reader::new(subrs_data);
    parse_index::<u16>(&mut r)
}

// TODO: Need to grab SIDs as well
fn parse_font_dict(data: &[u8]) -> Option<Range<usize>> {
    let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
    let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);
    while let Some(operator) = dict_parser.parse_next() {
        if operator == dict::operators::PRIVATE {
            return dict_parser.parse_range();
        }
    }

    None
}

fn parse_fd_select<'a>(
    number_of_glyphs: u16,
    r: &mut Reader<'a>,
) -> Option<FDSelect<'a>> {
    let format = r.read::<u8>()?;
    match format {
        0 => Some(FDSelect::Format0(r.read_array16::<u8>(number_of_glyphs)?)),
        3 => Some(FDSelect::Format3(r.tail()?)),
        _ => None,
    }
}
