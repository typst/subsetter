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
use crate::cff::charset::{parse_charset, write_charset, Charset};
use crate::cff::charstring::{CharString, Decompiler};
use crate::cff::cid_font::CIDMetadata;
use crate::cff::dict::top_dict::{parse_top_dict, write_top_dict, TopDictData};
use crate::cff::index::{create_index, parse_index, skip_index, Index};
use crate::cff::remapper::{FontDictRemapper, SidRemapper, SubroutineRemapper};
use crate::cff::subroutines::{write_gsubrs, SubroutineCollection, SubroutineContainer};
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
        // println!("GID: {:?}", old_gid);
        let fd_index = table.cid_metadata.fd_select.font_dict_index(old_gid).unwrap();
        fd_remapper.remap(fd_index);

        let mut decompiler = Decompiler::new(
            gsubrs.get_handler(),
            lsubrs.get_handler(fd_index).ok_or(MalformedFont)?,
        );
        let raw_charstring = table.char_strings.get(old_gid as u32).unwrap();
        let mut charstring = CharString::new(raw_charstring);
        charstring.decompile(&mut decompiler).map_err(|_| MalformedFont)?;

        charstring.used_gsubs().iter().for_each(|n| {
            gsubr_remapper.remap(*n);
        });

        let mapped_lsubrs = lsubr_remapper.get_mut(fd_index as usize).unwrap();

        charstring.used_lsubs().iter().for_each(|n| {
            mapped_lsubrs.remap(*n);
        });

        char_strings.push(RefCell::new(charstring))
    }

    let mut font_write_context = FontWriteContext::default();
    let mut subsetted_font = vec![];

    for _ in 0..2 {
        let mut w = Writer::new();
        // HEADER
        w.write(table.header);
        // NAME INDEX
        w.write(table.names);
        // TOP DICT
        w.extend(
            &write_top_dict(table.raw_top_dict, &mut font_write_context, &sid_remapper)
                .unwrap(),
        );
        // STRINGS
        w.extend(&write_sids(&sid_remapper, table.strings).unwrap());
        // GSUBRS
        w.extend(&write_gsubrs(&gsubr_remapper, gsubrs.get_handler()).unwrap());

        font_write_context.charset_offset =
            Number::IntegerNumber(IntegerNumber::from_i32_as_int5(w.len() as i32));
        w.extend(&write_charset(&sid_remapper, &table.charset, &ctx.mapper).unwrap());

        // font_write_context.char_strings_offset =
        //     Number::IntegerNumber(IntegerNumber::from_i32_as_int5(w.len() as i32));
        // w.extend(
        //     &write_char_strings(
        //         &ctx.mapper,
        //         &char_strings,
        //         &gsubr_remapper,
        //         &gsubrs,
        //         table.cid_metadata.fd_select,
        //         &lsubr_remapper,
        //         &lsubrs,
        //     )
        //     .unwrap(),
        // );

        subsetted_font = w.finish();
    }
    // // ttf_parser::cff::Table::parse(&subsetted_font);
    //
    // std::fs::write("outt.ttf", subsetted_font).unwrap();

    Ok(())
}

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
fn write_sids(sid_remapper: &SidRemapper, strings: Index) -> Result<Vec<u8>> {
    let mut new_strings = vec![];
    for sid in sid_remapper.sids() {
        new_strings.push(
            strings
                .get(sid.0.checked_sub(StringId::CUSTOM_SID).unwrap() as u32)
                .unwrap()
                .to_vec(),
        );
    }

    create_index(new_strings)
}

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
