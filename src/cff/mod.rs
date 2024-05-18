// TODO: Add acknowledgements

mod argstack;
mod charset;
mod charstring;
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
use crate::cff::remapper::{FontDictRemapper, SidRemapper};
use crate::cff::subroutines::{SubroutineCollection, SubroutineContainer};
use charset::charset_id;
use std::collections::BTreeSet;
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
        let subroutines = table.global_subrs.into_iter().collect::<Vec<_>>();
        SubroutineContainer::new(subroutines)
    };

    let lsubrs = {
        let subroutines = table
            .cid_metadata
            .font_dicts
            .into_iter()
            .map(|font_dict| font_dict.local_subrs.into_iter().collect::<Vec<_>>())
            .collect::<Vec<_>>();
        SubroutineCollection::new(subroutines)
    };

    let mut fd_remapper = FontDictRemapper::new();
    let sid_remapper = get_sid_remapper(ctx, &table.top_dict_data.used_sids);
    let mut char_strings = vec![];

    for old_gid in ctx.mapper.old_gids() {
        let fd_index = table.cid_metadata.fd_select.font_dict_index(old_gid).unwrap();
        fd_remapper.remap(fd_index);

        let mut decompiler = Decompiler::new(
            gsubrs.get_handler(),
            lsubrs.get_handler(fd_index).ok_or(MalformedFont)?,
        );
        let charstring = table.char_strings.get(old_gid as u32).unwrap();
        char_strings.push(decompiler.decompile(charstring)?);
    }

    let mut font_write_context = FontWriteContext::default();
    let mut subsetted_font = vec![];

    // TODO: Don't write two times
    for _ in 0..2 {
        let mut w = Writer::new();
        // HEADER
        w.write(table.header);
        // Name INDEX
        w.write(table.names);
        // Top DICT INDEX
        // Note: CFF fonts only have 1 top dict, so index of length 1.
        w.extend(&create_index(vec![write_top_dict(
            table.raw_top_dict,
            &mut font_write_context,
            &sid_remapper,
        )
        .unwrap()])?);
        // String INDEX
        w.extend(&write_sids(&sid_remapper, table.strings).unwrap());
        // Global Subr INDEX
        // Note: We desubroutinized, so no global subroutines and thus empty index.
        w.extend(&create_index(vec![vec![]]).unwrap());

        font_write_context.charset_offset =
            Number::IntegerNumber(IntegerNumber::from_i32_as_int5(w.len() as i32));
        // Charset
        w.extend(&write_charset(&sid_remapper, &table.charset, &ctx.mapper).unwrap());

        font_write_context.char_strings_offset =
            Number::IntegerNumber(IntegerNumber::from_i32_as_int5(w.len() as i32));
        w.extend(&create_index(char_strings.iter().map(|p| p.compile()).collect())?);

        subsetted_font = w.finish();
    }
    // ttf_parser::cff::Table::parse(&subsetted_font);

    std::fs::write("outt.otf", subsetted_font).unwrap();

    Ok(())
}
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
