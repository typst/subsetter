// TODO: Add acknowledgements

mod argstack;
mod charset;
mod charstring;
mod dict;
mod index;
// mod private_dict;
mod remapper;
// mod top_dict;
mod cid_font;
mod number;
mod operator;
mod sid_font;
mod subroutines;

use super::*;
use crate::cff::charset::{parse_charset, write_charset, Charset};
use crate::cff::charstring::Decompiler;
use crate::cff::cid_font::{build_fd_index, CIDMetadata};
use crate::cff::dict::font_dict::write_font_dict_index;
use crate::cff::dict::private_dict::{write_private_dicts, write_sid_private_dicts};
use crate::cff::dict::top_dict::{parse_top_dict, write_top_dict_index, TopDictData};
use crate::cff::index::{create_index, parse_index, skip_index, Index};
use crate::cff::remapper::{FontDictRemapper, SidRemapper};
use crate::cff::sid_font::SIDMetadata;
use crate::cff::subroutines::{SubroutineCollection, SubroutineContainer};
use crate::Error::SubsetError;
use charset::charset_id;
use number::{IntegerNumber, StringId};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub(crate) enum FontKind<'a> {
    Sid(SIDMetadata<'a>),
    Cid(CIDMetadata<'a>),
}

#[derive(Clone)]
pub struct Table<'a> {
    header: &'a [u8],
    names: &'a [u8],
    raw_top_dict: &'a [u8],
    top_dict_data: TopDictData,
    strings: Index<'a>,
    global_subrs: Index<'a>,
    charset: Charset<'a>,
    char_strings: Index<'a>,
    font_kind: FontKind<'a>,
}

#[derive(Debug)]
struct CIDWriteContext {
    fd_array_offset: IntegerNumber,
    fd_select_offset: IntegerNumber,
}

#[derive(Debug)]
struct FontWriteContext {
    // TOP DICT DATA
    charset_offset: IntegerNumber,
    encoding_offset: IntegerNumber,
    char_strings_offset: IntegerNumber,
    private_dicts_offsets: Vec<(IntegerNumber, IntegerNumber)>,
    cid_context: Option<CIDWriteContext>,
}

impl FontWriteContext {
    pub fn new_cid(num_font_dicts: u8) -> Self {
        Self {
            char_strings_offset: IntegerNumber(0),
            encoding_offset: IntegerNumber(0),
            charset_offset: IntegerNumber(0),
            private_dicts_offsets: vec![
                (IntegerNumber(0), IntegerNumber(0));
                num_font_dicts as usize
            ],
            cid_context: Some(CIDWriteContext {
                fd_select_offset: IntegerNumber(0),
                fd_array_offset: IntegerNumber(0),
            }),
        }
    }

    pub fn new_sid() -> Self {
        Self {
            char_strings_offset: IntegerNumber(0),
            encoding_offset: IntegerNumber(0),
            charset_offset: IntegerNumber(0),
            private_dicts_offsets: vec![(IntegerNumber(0), IntegerNumber(0))],
            cid_context: None,
        }
    }
}

pub fn subset(ctx: &mut Context<'_>) -> Result<()> {
    let table = Table::parse(ctx).unwrap();

    let gsubrs = {
        let subroutines = table.global_subrs.into_iter().collect::<Vec<_>>();
        SubroutineContainer::new(subroutines)
    };

    let lsubrs = {
        match &table.font_kind {
            FontKind::Cid(cid) => {
                let subroutines = cid
                    .font_dicts
                    .iter()
                    .map(|font_dict| {
                        font_dict.local_subrs.into_iter().collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                SubroutineCollection::new(subroutines)
            }
            FontKind::Sid(sid) => {
                let subroutines = sid.local_subrs.into_iter().collect::<Vec<_>>();
                SubroutineCollection::new(vec![subroutines])
            }
        }
    };

    let mut used_fds = BTreeSet::new();
    let sid_remapper = get_sid_remapper(&table, &ctx.mapper);
    let mut char_strings = vec![];

    for old_gid in ctx.mapper.remapped_gids() {
        let fd_index = match &table.font_kind {
            FontKind::Cid(ref cid) => {
                let fd_index = cid.fd_select.font_dict_index(old_gid).unwrap();
                used_fds.insert(fd_index);
                fd_index
            }
            FontKind::Sid(_) => 0,
        };

        let decompiler = Decompiler::new(
            gsubrs.get_handler(),
            lsubrs.get_handler(fd_index).ok_or(MalformedFont)?,
        );
        let charstring = table.char_strings.get(old_gid as u32).unwrap();
        char_strings.push(decompiler.decompile(charstring)?);
    }

    let mut fd_remapper = FontDictRemapper::new();

    for fd in used_fds {
        fd_remapper.remap(fd);
    }

    let mut font_write_context = match table.font_kind {
        FontKind::Sid(_) => FontWriteContext::new_sid(),
        FontKind::Cid(_) => FontWriteContext::new_cid(fd_remapper.len()),
    };
    let mut subsetted_font = vec![];

    // TODO: Don't write two times
    for _ in 0..3 {
        println!("{:?}", font_write_context);
        let mut w = Writer::new();
        // HEADER
        w.write(table.header);
        // Name INDEX
        w.write(table.names);
        // Top DICT INDEX
        // Note: CFF fonts only have 1 top dict, so index of length 1.
        w.extend(
            &write_top_dict_index(
                table.raw_top_dict,
                &mut font_write_context,
                &sid_remapper,
            )
            .unwrap(),
        );
        // String INDEX
        w.extend(&write_sids(&sid_remapper, table.strings).unwrap());
        // Global Subr INDEX
        // Note: We desubroutinized, so no global subroutines and thus empty index.
        w.extend(&create_index(vec![]).unwrap());

        font_write_context.charset_offset = IntegerNumber(w.len() as i32);
        // Charsets
        w.extend(
            &write_charset(&sid_remapper, &table.font_kind, &table.charset, &ctx.mapper)
                .unwrap(),
        );

        if let Some(ref mut cid) = font_write_context.cid_context {
            let FontKind::Cid(ref cid_metadata) = table.font_kind else {
                return Err(SubsetError);
            };

            cid.fd_select_offset = IntegerNumber(w.len() as i32);
            // FDSelect
            w.extend(&build_fd_index(&ctx.mapper, cid_metadata.fd_select, &fd_remapper)?);

            // FD Array
            cid.fd_array_offset = IntegerNumber(w.len() as i32);
            w.extend(&write_font_dict_index(
                &fd_remapper,
                &sid_remapper,
                &mut font_write_context,
                cid_metadata,
            )?);
        }

        // Charstrings INDEX
        font_write_context.char_strings_offset = IntegerNumber(w.len() as i32);
        w.extend(&create_index(char_strings.iter().map(|p| p.compile()).collect())?);

        match table.font_kind {
            FontKind::Sid(ref sid) => {
                write_sid_private_dicts(&mut font_write_context, sid, &mut w)?
            }
            FontKind::Cid(ref cid) => {
                write_private_dicts(&fd_remapper, &mut font_write_context, cid, &mut w)?;
            }
        }

        subsetted_font = w.finish();
    }
    ctx.push(Tag::CFF, subsetted_font);

    Ok(())
}
fn write_sids(sid_remapper: &SidRemapper, strings: Index) -> Result<Vec<u8>> {
    let mut new_strings = vec![];
    for sid in sid_remapper.sids() {
        new_strings.push(
            strings
                .get(sid.0.checked_sub(StringId::STANDARD_STRING_LEN).unwrap() as u32)
                .unwrap()
                .to_vec(),
        );
    }

    create_index(new_strings)
}

fn get_sid_remapper(table: &Table, gid_remapper: &GlyphRemapper) -> SidRemapper {
    let mut sid_remapper = SidRemapper::new();
    for sid in &table.top_dict_data.used_sids {
        sid_remapper.remap(*sid);
    }

    match table.font_kind {
        FontKind::Sid(_) => {
            for gid in gid_remapper.remapped_gids() {
                if let Some(sid) = table.charset.gid_to_sid(gid) {
                    sid_remapper.remap(sid);
                }
            }
        }
        FontKind::Cid(ref cid) => {
            for font_dict in &cid.font_dicts {
                if let Some(sid) = font_dict.font_name_sid {
                    sid_remapper.remap(sid);
                }
            }
        }
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

        let font_kind = if top_dict_data.has_ros {
            FontKind::Cid(
                cid_font::parse_cid_metadata(cff, &top_dict_data, number_of_glyphs)
                    .ok_or(MalformedFont)?,
            )
        } else {
            FontKind::Sid(sid_font::parse_sid_metadata(cff, &top_dict_data))
        };

        Ok(Self {
            header,
            names,
            raw_top_dict,
            top_dict_data,
            strings,
            global_subrs,
            charset,
            char_strings,
            font_kind,
        })
    }
}
