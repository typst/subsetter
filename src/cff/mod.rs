// TODO: Add acknowledgements

// mod argstack;
// mod charset;
// pub(crate) mod charstring;
mod dict;
// mod encoding;
mod index;
// mod private_dict;
mod remapper;
// mod top_dict;
mod types;
mod subroutines;
mod operator;

// use super::*;
// use crate::cff::charset::{parse_charset, Charset};
// use crate::cff::charstring::{
//     apply_bias, calc_subroutine_bias, unapply_bias, CharString, Decompiler, Instruction,
//     Program, SharedCharString,
// };
// use crate::cff::dict::DictionaryParser;
// use crate::cff::encoding::Encoding;
// use crate::cff::index::{parse_index, skip_index, Index, OffsetSize};
// use crate::cff::operator::{CALL_GLOBAL_SUBROUTINE, CALL_LOCAL_SUBROUTINE};
// use crate::cff::private_dict::parse_subr_offset;
// use crate::cff::top_dict::top_dict_operator::{
//     BASE_FONT_BLEND, BASE_FONT_NAME, COPYRIGHT, FAMILY_NAME, FONT_NAME, FULL_NAME,
//     NOTICE, POSTSCRIPT, ROS, VERSION, WEIGHT,
// };
// use crate::read::LazyArray16;
// use remapper::Remapper;
// use std::array;
// use std::cell::RefCell;
// use std::collections::BTreeSet;
// use std::hash::Hash;
// use std::ops::{Add, Range};
// use top_dict::{top_dict_operator, TopDictData};
// use types::U24;
// use types::{IntegerNumber, Number, StringId};
//
// // Limits according to the Adobe Technical Note #5176, chapter 4 DICT Data.
// const MAX_OPERANDS_LEN: usize = 48;
// const MAX_ARGUMENTS_STACK_LEN: usize = 513;
//
// /// A [Compact Font Format Table](
// /// https://docs.microsoft.com/en-us/typography/opentype/spec/cff).
// #[derive(Clone)]
// pub struct Table<'a> {
//     table_data: &'a [u8],
//     header: &'a [u8],
//     names: &'a [u8],
//     raw_top_dict: &'a [u8],
//     top_dict_data: TopDictData,
//     strings: Index<'a>,
//     global_subrs: Index<'a>,
//     charset: Charset<'a>,
//     number_of_glyphs: u16,
//     char_strings: Index<'a>,
//     kind: Option<FontKind<'a>>,
// }
//
// struct FontWriteContext<'a> {
//     // TOP DICT DATA
//     charset_offset: Number<'a>,
//     encoding_offset: Number<'a>,
//     char_strings_offset: Number<'a>,
//     // pub(crate) private: Option<Range<usize>>,
//     fd_array_offset: Number<'a>,
//     fd_select_offset: Number<'a>,
// }
//
// impl Default for FontWriteContext<'_> {
//     fn default() -> Self {
//         Self {
//             char_strings_offset: Number::IntegerNumber(IntegerNumber::from_i32_as_int5(
//                 0,
//             )),
//             encoding_offset: Number::IntegerNumber(IntegerNumber::from_i32_as_int5(0)),
//             charset_offset: Number::IntegerNumber(IntegerNumber::from_i32_as_int5(0)),
//             fd_select_offset: Number::IntegerNumber(IntegerNumber::from_i32_as_int5(0)),
//             fd_array_offset: Number::IntegerNumber(IntegerNumber::from_i32_as_int5(0)),
//         }
//     }
// }
//
// pub fn subset<'a>(ctx: &mut Context<'a>) {
//     let table = Table::parse(ctx).unwrap();
//
//     let Some(FontKind::CID(kind)) = table.kind else {
//         return;
//     };
//
//     let gsubrs = table
//         .global_subrs
//         .into_iter()
//         .map(|g| RefCell::new(CharString::new(g)))
//         .collect::<Vec<_>>();
//     let lsubrs = kind
//         .local_subrs
//         .into_iter()
//         .map(|index| {
//             index
//                 .into_iter()
//                 .map(|g| RefCell::new(CharString::new(g)))
//                 .collect::<Vec<_>>()
//         })
//         .collect::<Vec<_>>();
//
//     let mut gsubr_remapper = Remapper::new();
//     let mut lsubr_remapper = vec![Remapper::new(); lsubrs.len()];
//     let mut fd_remapper = Remapper::new();
//     let sid_remapper = get_sid_remapper(ctx, &table.top_dict_data.used_sids);
//     let mut char_strings = vec![];
//
//     for i in 0..ctx.mapper.num_gids() {
//         let original_gid = ctx.mapper.get_reverse(i).unwrap();
//         let fd_index = kind.fd_select.font_dict_index(original_gid).unwrap();
//         fd_remapper.remap(fd_index);
//         let lsubrs = lsubrs.get(fd_index as usize).unwrap();
//
//         let mut decompiler = Decompiler::new(&lsubrs, &gsubrs);
//         let raw_charstring = table.char_strings.get(original_gid as u32).unwrap();
//         let mut charstring = CharString::new(raw_charstring);
//         charstring.decompile(&mut decompiler).unwrap();
//
//         charstring.used_gsubs().unwrap().iter().for_each(|n| {
//             gsubr_remapper.remap(*n);
//         });
//
//         let mapped_lsubrs = lsubr_remapper.get_mut(fd_index as usize).unwrap();
//
//         charstring.used_lsubs().unwrap().iter().for_each(|n| {
//             mapped_lsubrs.remap(*n);
//         });
//
//         char_strings.push(RefCell::new(charstring))
//     }
//
//     let mut font_write_context = FontWriteContext::default();
//     let mut subsetted_font = vec![];
//
//     for i in 0..2 {
//         let mut w = Writer::new();
//         // HEADER
//         w.write(table.header);
//         // NAME INDEX
//         w.write(table.names);
//         // TOP DICT
//         w.extend(
//             &write_top_dict(table.raw_top_dict, &mut font_write_context, &sid_remapper)
//                 .unwrap(),
//         );
//         // STRINGS
//         w.extend(&write_sids(&sid_remapper, table.strings).unwrap());
//         // GSUBRS
//         w.extend(&write_gsubrs(&gsubr_remapper, &gsubrs).unwrap());
//
//         font_write_context.charset_offset =
//             Number::IntegerNumber(IntegerNumber::from_i32_as_int5(w.len() as i32));
//         w.extend(&write_charset(&sid_remapper, &table.charset, &ctx.mapper).unwrap());
//
//         font_write_context.char_strings_offset =
//             Number::IntegerNumber(IntegerNumber::from_i32_as_int5(w.len() as i32));
//         w.extend(
//             &write_char_strings(
//                 &ctx.mapper,
//                 &char_strings,
//                 &gsubr_remapper,
//                 &gsubrs,
//                 kind.fd_select,
//                 &lsubr_remapper,
//                 &lsubrs,
//             )
//             .unwrap(),
//         );
//
//         subsetted_font = w.finish();
//     }
//     ttf_parser::cff::Table::parse(&subsetted_font);
// }
//
// fn write_charset(
//     sid_remapper: &SidRemapper,
//     charset: &Charset,
//     gid_mapper: &GidMapper,
// ) -> Result<Vec<u8>> {
//     // TODO: Explore using Format 1/2
//     let mut w = Writer::new();
//     // Format 0
//     w.write::<u8>(0);
//
//     for gid in 1..gid_mapper.num_gids() {
//         let old_gid = gid_mapper.get_reverse(gid).unwrap();
//         let sid = charset.gid_to_sid(old_gid).unwrap();
//         // TODO: need to remap SID in SID-keyed fonts.
//         w.write(sid)
//     }
//
//     Ok(w.finish())
// }
//
// fn write_gsubrs(
//     gsubr_remapper: &Remapper<u32>,
//     gsubrs: &[SharedCharString],
// ) -> Result<Vec<u8>> {
//     let mut new_gsubrs = vec![];
//
//     for (new, old) in gsubr_remapper.sorted().iter().enumerate() {
//         let new = new as u32;
//         let mut new_program = Program::default();
//         let program = &gsubrs.get(*old as usize).unwrap().borrow().program;
//
//         let mut iter = program.instructions().iter().peekable();
//
//         while let Some(instruction) = iter.next() {
//             match instruction {
//                 Instruction::HintMask(mask) => {
//                     new_program.push(Instruction::HintMask(*mask))
//                 }
//                 Instruction::Operand(num) => {
//                     if let Some(Instruction::SingleByteOperator(op)) = iter.peek() {
//                         if *op == CALL_GLOBAL_SUBROUTINE {
//                             let old_gsubr = unapply_bias(
//                                 num.as_i32().unwrap(),
//                                 calc_subroutine_bias(gsubrs.len() as u32),
//                             )
//                             .unwrap();
//                             let new_gsubr = apply_bias(
//                                 gsubr_remapper.get(old_gsubr).unwrap() as i32,
//                                 calc_subroutine_bias(gsubr_remapper.len()),
//                             )
//                             .unwrap();
//                             new_program
//                                 .push(Instruction::Operand(Number::from_i32(new_gsubr)));
//                             continue;
//                         }
//                     }
//
//                     new_program.push(Instruction::Operand(num.clone()))
//                 }
//                 // TODO: What if two gsubr/lsubr next to each other>
//                 Instruction::DoubleByteOperator(op) => {
//                     new_program.push(Instruction::DoubleByteOperator(*op))
//                 }
//                 Instruction::SingleByteOperator(op) => {
//                     new_program.push(Instruction::SingleByteOperator(*op))
//                 }
//             }
//         }
//
//         let mut w = Writer::new();
//         new_program.compile(&mut w);
//         new_gsubrs.push(w.finish());
//     }
//
//     create_index(new_gsubrs)
// }
//
// fn write_char_strings(
//     gid_mapper: &GidMapper,
//     char_strings: &[SharedCharString],
//     gsubr_remapper: &Remapper<u32>,
//     gsubrs: &[SharedCharString],
//     fd_select: FDSelect,
//     lsubr_remappers: &Vec<Remapper<u32>>,
//     lsubrs: &[Vec<SharedCharString>],
// ) -> Result<Vec<u8>> {
//     let mut new_char_strings = vec![];
//
//     for i in 0..gid_mapper.num_gids() {
//         let old = gid_mapper.get_reverse(i).unwrap();
//         let mut new_program = Program::default();
//         println!("{:?}", char_strings.get(i as usize).unwrap().borrow().used_lsubs());
//         let program = &char_strings.get(i as usize).unwrap().borrow().program;
//
//         let mut iter = program.instructions().iter().peekable();
//
//         while let Some(instruction) = iter.next() {
//             match instruction {
//                 Instruction::HintMask(mask) => {
//                     new_program.push(Instruction::HintMask(*mask))
//                 }
//                 Instruction::Operand(num) => {
//                     if let Some(Instruction::SingleByteOperator(op)) = iter.peek() {
//                         if *op == CALL_GLOBAL_SUBROUTINE {
//                             let old_gsubr = unapply_bias(
//                                 num.as_i32().unwrap(),
//                                 calc_subroutine_bias(gsubrs.len() as u32),
//                             )
//                             .unwrap();
//                             let new_gsubr = apply_bias(
//                                 gsubr_remapper.get(old_gsubr).unwrap() as i32,
//                                 calc_subroutine_bias(gsubr_remapper.len()),
//                             )
//                             .unwrap();
//                             new_program
//                                 .push(Instruction::Operand(Number::from_i32(new_gsubr)));
//                             continue;
//                         } else if *op == CALL_LOCAL_SUBROUTINE {
//                             let fd_index = fd_select.font_dict_index(old).unwrap();
//
//                             let lsubr_remapper =
//                                 lsubr_remappers.get(fd_index as usize).unwrap();
//                             let lsubrs = lsubrs.get(fd_index as usize).unwrap();
//                             let old_lsubr = unapply_bias(
//                                 num.as_i32().unwrap(),
//                                 calc_subroutine_bias(lsubrs.len() as u32),
//                             )
//                             .unwrap();
//                             let new_lsubr = apply_bias(
//                                 lsubr_remapper.get(old_lsubr).unwrap() as i32,
//                                 calc_subroutine_bias(gsubr_remapper.len()),
//                             )
//                             .unwrap();
//
//                             let new_lsubr = apply_bias(
//                                 new_lsubr,
//                                 calc_subroutine_bias(lsubr_remapper.len()),
//                             )
//                             .unwrap();
//                             new_program
//                                 .push(Instruction::Operand(Number::from_i32(new_lsubr)));
//                             continue;
//                         }
//                     }
//
//                     new_program.push(Instruction::Operand(num.clone()))
//                 }
//                 // TODO: What if two gsubr/lsubr next to each other>
//                 Instruction::DoubleByteOperator(op) => {
//                     new_program.push(Instruction::DoubleByteOperator(*op))
//                 }
//                 Instruction::SingleByteOperator(op) => {
//                     new_program.push(Instruction::SingleByteOperator(*op))
//                 }
//             }
//         }
//
//         let mut w = Writer::new();
//         new_program.compile(&mut w);
//         new_char_strings.push(w.finish());
//     }
//
//     create_index(new_char_strings)
// }
//
// fn write_sids(sid_remapper: &SidRemapper, strings: Index) -> Result<Vec<u8>> {
//     let mut new_strings = vec![];
//     for (_, old) in sid_remapper.sorted().iter().enumerate() {
//         new_strings
//             .push(strings.get(old.checked_sub(391).unwrap() as u32).unwrap().to_vec());
//     }
//
//     create_index(new_strings)
// }
//
// fn write_top_dict(
//     raw_top_dict: &[u8],
//     font_write_context: &mut FontWriteContext,
//     sid_remapper: &SidRemapper,
// ) -> Result<Vec<u8>> {
//     use top_dict_operator::*;
//
//     let mut w = Writer::new();
//     let mut r = Reader::new(raw_top_dict);
//
//     let index = parse_index::<u16>(&mut r).unwrap();
//
//     // The Top DICT INDEX should have only one dictionary.
//     let data = index.get(0).unwrap();
//
//     let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
//     let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);
//
//     let mut write = |operands: &[u8], operator: u16| {
//         for operand in operands {
//             w.write(*operand);
//         }
//
//         if operator > 255 {
//             w.write::<u8>(12);
//             w.write(u8::try_from(operator - 1200).unwrap());
//         } else {
//             w.write::<u8>(operator as u8);
//         }
//     };
//
//     while let Some(operator) = dict_parser.parse_next() {
//         match operator.get() {
//             CHARSET => {
//                 write(font_write_context.charset_offset.as_bytes(), operator.get())
//             }
//             ENCODING => {
//                 write(font_write_context.encoding_offset.as_bytes(), operator.get())
//             }
//             CHAR_STRINGS => {
//                 write(font_write_context.char_strings_offset.as_bytes(), operator.get())
//             }
//             FD_ARRAY => {
//                 write(font_write_context.fd_array_offset.as_bytes(), operator.get())
//             }
//             FD_SELECT => {
//                 write(font_write_context.fd_select_offset.as_bytes(), operator.get())
//             }
//             VERSION | NOTICE | COPYRIGHT | FULL_NAME | FAMILY_NAME | WEIGHT
//             | POSTSCRIPT | BASE_FONT_NAME | BASE_FONT_BLEND | FONT_NAME => {
//                 let sid = sid_remapper.get(dict_parser.parse_sid().unwrap().0).unwrap();
//                 write(Number::from_i32(sid as i32).as_bytes(), operator.get())
//             }
//             ROS => {
//                 dict_parser.parse_operands().unwrap();
//                 let operands = dict_parser.operands();
//
//                 let arg1 = sid_remapper
//                     .get(u16::try_from(operands[0].as_u32().unwrap()).unwrap())
//                     .unwrap();
//                 let arg2 = sid_remapper
//                     .get(u16::try_from(operands[1].as_u32().unwrap()).unwrap())
//                     .unwrap();
//
//                 let mut w = Writer::new();
//                 w.write(Number::from_i32(arg1 as i32).as_bytes());
//                 w.write(Number::from_i32(arg2 as i32).as_bytes());
//                 w.write(operands[2].as_bytes());
//                 write(&w.finish(), operator.get());
//             }
//             PRIVATE => unimplemented!(),
//             _ => {
//                 dict_parser.parse_operands().unwrap();
//                 let operands = dict_parser.operands();
//
//                 let mut w = Writer::new();
//
//                 for operand in operands {
//                     w.write(operand.as_bytes());
//                 }
//
//                 write(&w.finish(), operator.get());
//             }
//         }
//     }
//
//     let finished = w.finish();
//     create_index(vec![finished])
// }
//
// fn create_index(data: Vec<Vec<u8>>) -> Result<Vec<u8>> {
//     let count = u16::try_from(data.len()).map_err(|_| MalformedFont)?;
//     // + 1 Since we start counting from the preceding byte.
//     let offsize = data.iter().map(|v| v.len() as u32).sum::<u32>() + 1;
//
//     // Empty Index only contains the count field
//     if count == 0 {
//         return Ok(vec![0, 0]);
//     }
//
//     let offset_size = if offsize <= u8::MAX as u32 {
//         OffsetSize::Size1
//     } else if offsize <= u16::MAX as u32 {
//         OffsetSize::Size2
//     } else if offsize <= U24::MAX {
//         OffsetSize::Size3
//     } else {
//         OffsetSize::Size4
//     };
//
//     let mut w = Writer::new();
//     w.write(count);
//     w.write(offset_size as u8);
//
//     let mut cur_offset: u32 = 0;
//
//     let mut write_offset = |len| {
//         cur_offset += len;
//
//         match offset_size {
//             OffsetSize::Size1 => {
//                 let num = u8::try_from(cur_offset).map_err(|_| MalformedFont)?;
//                 w.write(num);
//             }
//             OffsetSize::Size2 => {
//                 let num = u16::try_from(cur_offset).map_err(|_| MalformedFont)?;
//                 w.write(num);
//             }
//             OffsetSize::Size3 => {
//                 let num = U24(cur_offset);
//                 w.write(num);
//             }
//             OffsetSize::Size4 => w.write(cur_offset),
//         }
//
//         Ok(())
//     };
//
//     write_offset(1)?;
//     for el in &data {
//         write_offset(el.len() as u32)?;
//     }
//
//     for el in &data {
//         w.extend(el);
//     }
//
//     Ok(w.finish())
// }
//
// fn get_sid_remapper(ctx: &Context, used_sids: &BTreeSet<StringId>) -> SidRemapper {
//     // SIDs can appear in the top dict and charset
//     // There are 391 standard strings, so we need to start from 392
//     let mut sid_remapper = SidRemapper::new();
//     for sid in used_sids {
//         sid_remapper.remap(sid.0);
//     }
//
//     sid_remapper
// }
//
// impl<'a> Table<'a> {
//     pub fn parse(ctx: &mut Context<'a>) -> Result<Self> {
//         let cff = ctx.expect_table(Tag::CFF).ok_or(MalformedFont)?;
//
//         let mut r = Reader::new(cff);
//
//         let major = r.read::<u8>().ok_or(MalformedFont)?;
//
//         if major != 1 {
//             return Err(Error::Unimplemented);
//         }
//
//         r.skip::<u8>(); // minor
//         let header_size = r.read::<u8>().ok_or(MalformedFont)?;
//         let header = cff.get(0..header_size as usize).ok_or(MalformedFont)?;
//
//         r.jump(header_size as usize);
//
//         let names_start = r.offset();
//         skip_index::<u16>(&mut r).ok_or(MalformedFont)?;
//         let names = cff.get(names_start..r.offset()).ok_or(MalformedFont)?;
//         let raw_top_dict = r.tail().ok_or(MalformedFont)?;
//         let top_dict_data = top_dict::parse_top_dict(&mut r).ok_or(MalformedFont)?;
//
//         let strings = parse_index::<u16>(&mut r).ok_or(MalformedFont)?;
//         let global_subrs = parse_index::<u16>(&mut r).ok_or(MalformedFont)?;
//
//         let char_strings_offset = top_dict_data.char_strings.ok_or(MalformedFont)?;
//         let char_strings = {
//             let mut r = Reader::new_at(cff, char_strings_offset);
//             parse_index::<u16>(&mut r).ok_or(MalformedFont)?
//         };
//
//         let number_of_glyphs = u16::try_from(char_strings.len())
//             .ok()
//             .filter(|n| *n > 0)
//             .ok_or(MalformedFont)?;
//
//         let charset = match top_dict_data.charset {
//             Some(charset_id::ISO_ADOBE) => Charset::ISOAdobe,
//             Some(charset_id::EXPERT) => Charset::Expert,
//             Some(charset_id::EXPERT_SUBSET) => Charset::ExpertSubset,
//             Some(offset) => {
//                 let mut s = Reader::new_at(cff, offset);
//                 parse_charset(number_of_glyphs, &mut s).ok_or(MalformedFont)?
//             }
//             None => Charset::ISOAdobe, // default
//         };
//
//         let kind = if top_dict_data.has_ros {
//             parse_cid_metadata(cff, &top_dict_data, number_of_glyphs)
//         } else {
//             None
//         };
//
//         Ok(Self {
//             table_data: cff,
//             header,
//             names,
//             raw_top_dict,
//             top_dict_data,
//             strings,
//             global_subrs,
//             charset,
//             number_of_glyphs,
//             char_strings,
//             kind,
//         })
//     }
// }
//
// #[derive(Clone, Debug)]
// pub(crate) enum FontKind<'a> {
//     SID(SIDMetadata<'a>),
//     CID(CIDMetadata<'a>),
// }
//
// #[derive(Clone, Copy, Default, Debug)]
// pub(crate) struct SIDMetadata<'a> {
//     local_subrs: Index<'a>,
//     /// Can be zero.
//     default_width: f32,
//     /// Can be zero.
//     nominal_width: f32,
//     encoding: Encoding<'a>,
// }
//
// #[derive(Clone, Default, Debug)]
// pub(crate) struct CIDMetadata<'a> {
//     local_subrs: Vec<Index<'a>>,
//     fd_array: Index<'a>,
//     fd_select: FDSelect<'a>,
// }
//
// #[derive(Clone, Copy, Debug)]
// enum FDSelect<'a> {
//     Format0(LazyArray16<'a, u8>),
//     Format3(&'a [u8]), // It's easier to parse it in-place.
// }
//
// impl Default for FDSelect<'_> {
//     fn default() -> Self {
//         FDSelect::Format0(LazyArray16::default())
//     }
// }
//
// impl FDSelect<'_> {
//     fn font_dict_index(&self, glyph_id: u16) -> Option<u8> {
//         match self {
//             FDSelect::Format0(ref array) => array.get(glyph_id),
//             FDSelect::Format3(data) => {
//                 let mut r = Reader::new(data);
//                 let number_of_ranges = r.read::<u16>()?;
//                 if number_of_ranges == 0 {
//                     return None;
//                 }
//
//                 // 'A sentinel GID follows the last range element and serves
//                 // to delimit the last range in the array.'
//                 // So we can simply increase the number of ranges by one.
//                 let number_of_ranges = number_of_ranges.checked_add(1)?;
//
//                 // Range is: GlyphId + u8
//                 let mut prev_first_glyph = r.read::<u16>()?;
//                 let mut prev_index = r.read::<u8>()?;
//                 for _ in 1..number_of_ranges {
//                     let curr_first_glyph = r.read::<u16>()?;
//                     if (prev_first_glyph..curr_first_glyph).contains(&glyph_id) {
//                         return Some(prev_index);
//                     } else {
//                         prev_index = r.read::<u8>()?;
//                     }
//
//                     prev_first_glyph = curr_first_glyph;
//                 }
//
//                 None
//             }
//         }
//     }
// }
//
// fn parse_cid_metadata<'a>(
//     data: &'a [u8],
//     top_dict: &TopDictData,
//     number_of_glyphs: u16,
// ) -> Option<FontKind<'a>> {
//     let (charset_offset, fd_array_offset, fd_select_offset) =
//         match (top_dict.charset, top_dict.fd_array, top_dict.fd_select) {
//             (Some(a), Some(b), Some(c)) => (a, b, c),
//             _ => return None, // charset, FDArray and FDSelect must be set.
//         };
//
//     if charset_offset <= charset_id::EXPERT_SUBSET {
//         // 'There are no predefined charsets for CID fonts.'
//         // Adobe Technical Note #5176, chapter 18 CID-keyed Fonts
//         return None;
//     }
//
//     let mut metadata = CIDMetadata::default();
//
//     metadata.fd_array = {
//         let mut r = Reader::new_at(data, fd_array_offset);
//         parse_index::<u16>(&mut r)?
//     };
//
//     for font_dict_data in metadata.fd_array {
//         metadata
//             .local_subrs
//             .push(parse_cid_private_dict(data, font_dict_data).unwrap_or_default());
//     }
//
//     metadata.fd_select = {
//         let mut s = Reader::new_at(data, fd_select_offset);
//         parse_fd_select(number_of_glyphs, &mut s)?
//     };
//
//     Some(FontKind::CID(metadata))
// }
//
// fn parse_cid_private_dict<'a>(
//     data: &'a [u8],
//     font_dict_data: &'a [u8],
// ) -> Option<Index<'a>> {
//     let private_dict_range = parse_font_dict(font_dict_data)?;
//     let private_dict_data = data.get(private_dict_range.clone())?;
//     let subrs_offset = parse_subr_offset(private_dict_data)?;
//
//     let start = private_dict_range.start.checked_add(subrs_offset)?;
//     let subrs_data = data.get(start..)?;
//     let mut r = Reader::new(subrs_data);
//     parse_index::<u16>(&mut r)
// }
//
// fn parse_font_dict(data: &[u8]) -> Option<Range<usize>> {
//     let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
//     let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);
//     while let Some(operator) = dict_parser.parse_next() {
//         if operator.get() == top_dict_operator::PRIVATE {
//             return dict_parser.parse_range();
//         }
//     }
//
//     None
// }
//
// fn parse_fd_select<'a>(
//     number_of_glyphs: u16,
//     r: &mut Reader<'a>,
// ) -> Option<FDSelect<'a>> {
//     let format = r.read::<u8>()?;
//     match format {
//         0 => Some(FDSelect::Format0(r.read_array16::<u8>(number_of_glyphs)?)),
//         3 => Some(FDSelect::Format3(r.tail()?)),
//         _ => None,
//     }
// }
//
// /// Enumerates Charset IDs defined in the Adobe Technical Note #5176, Table 22
// mod charset_id {
//     pub const ISO_ADOBE: usize = 0;
//     pub const EXPERT: usize = 1;
//     pub const EXPERT_SUBSET: usize = 2;
// }
//
// /// Enumerates Charset IDs defined in the Adobe Technical Note #5176, Table 16
// mod encoding_id {
//     pub const STANDARD: usize = 0;
//     pub const EXPERT: usize = 1;
// }
//

//
// mod operator {
//     pub const HORIZONTAL_STEM: u8 = 1;
//     pub const VERTICAL_STEM: u8 = 3;
//     pub const VERTICAL_MOVE_TO: u8 = 4;
//     pub const LINE_TO: u8 = 5;
//     pub const HORIZONTAL_LINE_TO: u8 = 6;
//     pub const VERTICAL_LINE_TO: u8 = 7;
//     pub const CURVE_TO: u8 = 8;
//     pub const CALL_LOCAL_SUBROUTINE: u8 = 10;
//     pub const RETURN: u8 = 11;
//     pub const TWO_BYTE_OPERATOR_MARK: u8 = 12;
//     pub const ENDCHAR: u8 = 14;
//     pub const HORIZONTAL_STEM_HINT_MASK: u8 = 18;
//     pub const HINT_MASK: u8 = 19;
//     pub const COUNTER_MASK: u8 = 20;
//     pub const MOVE_TO: u8 = 21;
//     pub const HORIZONTAL_MOVE_TO: u8 = 22;
//     pub const VERTICAL_STEM_HINT_MASK: u8 = 23;
//     pub const CURVE_LINE: u8 = 24;
//     pub const LINE_CURVE: u8 = 25;
//     pub const VV_CURVE_TO: u8 = 26;
//     pub const HH_CURVE_TO: u8 = 27;
//     pub const SHORT_INT: u8 = 28;
//     pub const CALL_GLOBAL_SUBROUTINE: u8 = 29;
//     pub const VH_CURVE_TO: u8 = 30;
//     pub const HV_CURVE_TO: u8 = 31;
//     pub const HFLEX: u8 = 34;
//     pub const FLEX: u8 = 35;
//     pub const HFLEX1: u8 = 36;
//     pub const FLEX1: u8 = 37;
//     pub const FIXED_16_16: u8 = 255;
// }
