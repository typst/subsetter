mod dict;
mod index;

use super::*;
use crate::cff::dict::DictionaryParser;
use crate::cff::index::{parse_index, Index};
use std::num::NonZeroU16;
use std::ops::Range;

// Limits according to the Adobe Technical Note #5176, chapter 4 DICT Data.
const MAX_OPERANDS_LEN: usize = 48;

/// A [Compact Font Format Table](
/// https://docs.microsoft.com/en-us/typography/opentype/spec/cff).
#[derive(Clone, Copy)]
pub struct Table<'a> {
    table_data: &'a [u8],
    header: &'a [u8],
    names: Index<'a>, // strings: Index<'a>,
                      // global_subrs: Index<'a>,
                      // charset: Charset<'a>,
                      // number_of_glyphs: NonZeroU16,
                      // matrix: Matrix,
                      // char_strings: Index<'a>,
                      // kind: FontKind<'a>,
}

impl<'a> Table<'a> {
    pub fn parse(ctx: &mut Context<'a>) -> Result<Self> {
        let cff = ctx.expect_table(Tag::CFF).ok_or(MalformedFont)?;

        let mut r = Reader::new(cff);

        let major = r.read::<u8>().ok_or(MalformedFont)?;

        if major != 1 {
            return Err(Error::Unimplemented);
        }

        r.skip::<u8>(); // minor
        let header_size = r.read::<u8>().ok_or(MalformedFont)?;
        let header = cff.get(0..header_size as usize).ok_or(MalformedFont)?;

        r.jump(header_size as usize);

        let names = parse_index::<u16>(&mut r).ok_or(MalformedFont)?;

        Ok(Self { table_data: cff, header, names })
    }
}

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let table = Table::parse(ctx)?;

    Ok(())
}

pub(crate) fn discover(ctx: &mut Context) -> Result<()> {
    ctx.subset.insert(0);
    ctx.subset
        .extend(ctx.requested_glyphs.iter().filter(|g| **g < ctx.num_glyphs));
    Ok(())
}

#[derive(Default)]
struct TopDict {
    charset_offset: Option<usize>,
    encoding_offset: Option<usize>,
    char_strings_offset: usize,
    private_dict_range: Option<Range<usize>>,
    matrix: Vec<f64>,
    has_ros: bool,
    fd_array_offset: Option<usize>,
    fd_select_offset: Option<usize>,
}

fn parse_top_dict<'a>(r: &mut Reader<'_>) -> Option<TopDict> {
    let mut top_dict = TopDict::default();

    let index = parse_index::<u16>(r)?;

    // The Top DICT INDEX should have only one dictionary.
    let data = index.get(0)?;

    let mut operands_buffer = [0.0; MAX_OPERANDS_LEN];
    let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);
    while let Some(operator) = dict_parser.parse_next() {
        match operator.get() {
            top_dict_operator::CHARSET_OFFSET => {
                top_dict.charset_offset = dict_parser.parse_offset();
            }
            top_dict_operator::ENCODING_OFFSET => {
                top_dict.encoding_offset = dict_parser.parse_offset();
            }
            top_dict_operator::CHAR_STRINGS_OFFSET => {
                top_dict.char_strings_offset = dict_parser.parse_offset()?;
            }
            top_dict_operator::PRIVATE_DICT_SIZE_AND_OFFSET => {
                top_dict.private_dict_range = dict_parser.parse_range();
            }
            top_dict_operator::FONT_MATRIX => {
                dict_parser.parse_operands()?;
                top_dict.matrix = Vec::from(dict_parser.operands());
            }
            top_dict_operator::ROS => {
                top_dict.has_ros = true;
            }
            top_dict_operator::FD_ARRAY => {
                top_dict.fd_array_offset = dict_parser.parse_offset();
            }
            top_dict_operator::FD_SELECT => {
                top_dict.fd_select_offset = dict_parser.parse_offset();
            }
            _ => {}
        }
    }

    Some(top_dict)
}

/// Enumerates some operators defined in the Adobe Technical Note #5176,
/// Table 9 Top DICT Operator Entries
mod top_dict_operator {
    pub const CHARSET_OFFSET: u16 = 15;
    pub const ENCODING_OFFSET: u16 = 16;
    pub const CHAR_STRINGS_OFFSET: u16 = 17;
    pub const PRIVATE_DICT_SIZE_AND_OFFSET: u16 = 18;
    pub const FONT_MATRIX: u16 = 1207;
    pub const ROS: u16 = 1230;
    pub const FD_ARRAY: u16 = 1236;
    pub const FD_SELECT: u16 = 1237;
}
