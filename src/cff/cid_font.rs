use crate::cff::charset::charset_id;
use crate::cff::dict;
use crate::cff::dict::private_dict::parse_subr_offset;
use crate::cff::dict::top_dict::TopDictData;
use crate::cff::dict::DictionaryParser;
use crate::cff::index::{parse_index, Index};
use crate::cff::types::{Number, StringId};
use crate::read::{LazyArray16, Reader};
use std::array;
use std::ops::Range;

pub fn parse_cid_metadata<'a>(
    data: &'a [u8],
    top_dict: &TopDictData,
    number_of_glyphs: u16,
) -> Option<CIDMetadata<'a>> {
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
            .font_dicts
            .push(parse_font_dict(data, font_dict_data).unwrap_or_default());
    }

    metadata.fd_select = {
        let mut s = Reader::new_at(data, fd_select_offset);
        parse_fd_select(number_of_glyphs, &mut s)?
    };

    Some(metadata)
}

#[derive(Default, Clone, Debug)]
pub(crate) struct FontDict<'a> {
    pub(crate) local_subrs: Index<'a>,
    pub(crate) used_sids: Vec<StringId>,
    pub(crate) private_dict: &'a [u8],
}

fn parse_font_dict<'a>(
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

#[derive(Clone, Default, Debug)]
pub(crate) struct CIDMetadata<'a> {
    pub(crate) font_dicts: Vec<FontDict<'a>>,
    pub(crate) fd_array: Index<'a>,
    pub(crate) fd_select: FDSelect<'a>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum FDSelect<'a> {
    Format0(LazyArray16<'a, u8>),
    Format3(&'a [u8]), // It's easier to parse it in-place.
}

impl Default for FDSelect<'_> {
    fn default() -> Self {
        FDSelect::Format0(LazyArray16::default())
    }
}

impl FDSelect<'_> {
    pub(crate) fn font_dict_index(&self, glyph_id: u16) -> Option<u8> {
        match self {
            FDSelect::Format0(ref array) => array.get(glyph_id),
            FDSelect::Format3(data) => {
                let mut r = Reader::new(data);
                let number_of_ranges = r.read::<u16>()?;
                if number_of_ranges == 0 {
                    return None;
                }

                let number_of_ranges = number_of_ranges.checked_add(1)?;

                let mut prev_first_glyph = r.read::<u16>()?;
                let mut prev_index = r.read::<u8>()?;
                for _ in 1..number_of_ranges {
                    let curr_first_glyph = r.read::<u16>()?;
                    if (prev_first_glyph..curr_first_glyph).contains(&glyph_id) {
                        return Some(prev_index);
                    } else {
                        prev_index = r.read::<u8>()?;
                    }

                    prev_first_glyph = curr_first_glyph;
                }

                None
            }
        }
    }
}
