// TODO: Add acknowledgements

mod argstack;
mod charset;
pub(crate) mod charstring;
mod dict;
mod encoding;
mod index;
// mod private_dict;
mod remapper;
// mod top_dict;
mod cid_font;
mod operator;
mod subroutines;
mod types;

use super::*;
use crate::cff::charset::{parse_charset, Charset};
use crate::cff::charstring::{CharString, Decompiler};
use crate::cff::cid_font::CIDMetadata;
use crate::cff::dict::top_dict::{parse_top_dict, TopDictData};
use crate::cff::index::{parse_index, skip_index, Index};
use crate::cff::remapper::{FontDictRemapper, SidRemapper, SubroutineRemapper};
use crate::cff::subroutines::{SubroutineCollection, SubroutineContainer};
use charset::charset_id;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use types::{IntegerNumber, Number, StringId};

#[derive(Clone)]
pub struct Table<'a> {
    table_data: &'a [u8],
    header: &'a [u8],
    names: &'a [u8],
    raw_top_dict: &'a [u8],
    top_dict_data: TopDictData,
    strings: Index<'a>,
    global_subrs: Index<'a>,
    charset: Charset<'a>,
    number_of_glyphs: u16,
    char_strings: Index<'a>,
    cid_metadata: CIDMetadata<'a>,
}

struct FontWriteContext<'a> {
    // TOP DICT DATA
    charset_offset: Number<'a>,
    encoding_offset: Number<'a>,
    char_strings_offset: Number<'a>,
    // pub(crate) private: Option<Range<usize>>,
    fd_array_offset: Number<'a>,
    fd_select_offset: Number<'a>,
}

impl Default for FontWriteContext<'_> {
    fn default() -> Self {
        Self {
            char_strings_offset: Number::IntegerNumber(IntegerNumber::from_i32_as_int5(
                0,
            )),
            encoding_offset: Number::IntegerNumber(IntegerNumber::from_i32_as_int5(0)),
            charset_offset: Number::IntegerNumber(IntegerNumber::from_i32_as_int5(0)),
            fd_select_offset: Number::IntegerNumber(IntegerNumber::from_i32_as_int5(0)),
            fd_array_offset: Number::IntegerNumber(IntegerNumber::from_i32_as_int5(0)),
        }
    }
}

pub fn subset<'a>(ctx: &mut Context<'a>) -> Result<()> {
    let table = Table::parse(ctx).unwrap();

    let gsubrs = {
        let subroutines = table
            .global_subrs
            .into_iter()
            .map(|g| Rc::new(RefCell::new(CharString::new(g))))
            .collect::<Vec<_>>();
        SubroutineContainer::new(subroutines)
    };

    let lsubrs = {
        let subroutines = table
            .cid_metadata
            .local_subrs
            .into_iter()
            .map(|index| {
                index
                    .into_iter()
                    .map(|g| Rc::new(RefCell::new(CharString::new(g))))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        SubroutineCollection::new(subroutines)
    };

    let mut gsubr_remapper = SubroutineRemapper::new();
    let mut lsubr_remapper =
        vec![SubroutineRemapper::new(); lsubrs.num_entries() as usize];
    let mut fd_remapper = FontDictRemapper::new();
    let sid_remapper = get_sid_remapper(ctx, &table.top_dict_data.used_sids);
    let mut char_strings = vec![];

    for old_gid in ctx.mapper.old_gids() {
        let fd_index = table.cid_metadata.fd_select.font_dict_index(old_gid).unwrap();
        fd_remapper.remap(fd_index);

        let mut decompiler = Decompiler::new(
            lsubrs.get_handler(fd_index).ok_or(MalformedFont)?,
            gsubrs.get_handler(),
        );
        let raw_charstring = table.char_strings.get(old_gid as u32).unwrap();
        let mut charstring = CharString::new(raw_charstring);
        charstring.decompile(&mut decompiler).unwrap();

        charstring.used_gsubs().unwrap().iter().for_each(|n| {
            gsubr_remapper.remap(*n);
        });

        let mapped_lsubrs = lsubr_remapper.get_mut(fd_index as usize).unwrap();

        charstring.used_lsubs().unwrap().iter().for_each(|n| {
            mapped_lsubrs.remap(*n);
        });

        char_strings.push(RefCell::new(charstring))
    }

    // let mut font_write_context = FontWriteContext::default();
    // let mut subsetted_font = vec![];
    //
    // for i in 0..2 {
    //     let mut w = Writer::new();
    //     // HEADER
    //     w.write(table.header);
    //     // NAME INDEX
    //     w.write(table.names);
    //     // TOP DICT
    //     w.extend(
    //         &write_top_dict(table.raw_top_dict, &mut font_write_context, &sid_remapper)
    //             .unwrap(),
    //     );
    //     // STRINGS
    //     w.extend(&write_sids(&sid_remapper, table.strings).unwrap());
    //     // GSUBRS
    //     w.extend(&write_gsubrs(&gsubr_remapper, &gsubrs).unwrap());
    //
    //     font_write_context.charset_offset =
    //         Number::IntegerNumber(IntegerNumber::from_i32_as_int5(w.len() as i32));
    //     w.extend(&write_charset(&sid_remapper, &table.charset, &ctx.mapper).unwrap());
    //
    //     font_write_context.char_strings_offset =
    //         Number::IntegerNumber(IntegerNumber::from_i32_as_int5(w.len() as i32));
    //     w.extend(
    //         &write_char_strings(
    //             &ctx.mapper,
    //             &char_strings,
    //             &gsubr_remapper,
    //             &gsubrs,
    //             kind.fd_select,
    //             &lsubr_remapper,
    //             &lsubrs,
    //         )
    //         .unwrap(),
    //     );
    //
    //     subsetted_font = w.finish();
    // }
    // ttf_parser::cff::Table::parse(&subsetted_font);

    Ok(())
}

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

fn get_sid_remapper(ctx: &Context, used_sids: &BTreeSet<StringId>) -> SidRemapper {
    let mut sid_remapper = SidRemapper::new();
    for sid in used_sids {
        sid_remapper.remap(*sid);
    }

    sid_remapper
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

        let names_start = r.offset();
        skip_index::<u16>(&mut r).ok_or(MalformedFont)?;
        let names = cff.get(names_start..r.offset()).ok_or(MalformedFont)?;
        let raw_top_dict = r.tail().ok_or(MalformedFont)?;
        let top_dict_data = parse_top_dict(&mut r).ok_or(MalformedFont)?;

        let strings = parse_index::<u16>(&mut r).ok_or(MalformedFont)?;
        let global_subrs = parse_index::<u16>(&mut r).ok_or(MalformedFont)?;

        let char_strings_offset = top_dict_data.char_strings.ok_or(MalformedFont)?;
        let char_strings = {
            let mut r = Reader::new_at(cff, char_strings_offset);
            parse_index::<u16>(&mut r).ok_or(MalformedFont)?
        };

        let number_of_glyphs = u16::try_from(char_strings.len())
            .ok()
            .filter(|n| *n > 0)
            .ok_or(MalformedFont)?;

        let charset = match top_dict_data.charset {
            Some(charset_id::ISO_ADOBE) => Charset::ISOAdobe,
            Some(charset_id::EXPERT) => Charset::Expert,
            Some(charset_id::EXPERT_SUBSET) => Charset::ExpertSubset,
            Some(offset) => {
                let mut s = Reader::new_at(cff, offset);
                parse_charset(number_of_glyphs, &mut s).ok_or(MalformedFont)?
            }
            None => Charset::ISOAdobe, // default
        };

        let cid_metadata =
            cid_font::parse_cid_metadata(cff, &top_dict_data, number_of_glyphs)
                .ok_or(MalformedFont)?;

        Ok(Self {
            table_data: cff,
            header,
            names,
            raw_top_dict,
            top_dict_data,
            strings,
            global_subrs,
            charset,
            number_of_glyphs,
            char_strings,
            cid_metadata,
        })
    }
}

/// Enumerates Charset IDs defined in the Adobe Technical Note #5176, Table 16
mod encoding_id {
    pub const STANDARD: usize = 0;
    pub const EXPERT: usize = 1;
}
