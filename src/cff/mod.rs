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
mod sid_font;

use super::*;
use crate::cff::charset::{Charset, parse_charset, write_charset};
use crate::cff::charstring::Decompiler;
use crate::cff::cid_font::{build_fd_index, CIDMetadata};
use crate::cff::dict::font_dict::write_font_dict_index;
use crate::cff::dict::private_dict::write_private_dicts;
use crate::cff::dict::top_dict::{parse_top_dict, TopDictData, write_top_dict_index};
use crate::cff::index::{create_index, Index, parse_index, skip_index};
use crate::cff::remapper::{FontDictRemapper, SidRemapper};
use crate::cff::subroutines::{SubroutineCollection, SubroutineContainer};
use charset::charset_id;
use std::collections::BTreeSet;
use types::{IntegerNumber, StringId};
use crate::cff::sid_font::SIDMetadata;

#[derive(Clone, Debug)]
pub(crate) enum FontKind<'a> {
    SID(SIDMetadata<'a>),
    CID(CIDMetadata<'a>),
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

struct SIDWriteContext<'a> {
    private_dict_offset: (IntegerNumber<'a>, IntegerNumber<'a>)
}

struct CIDWriteContext<'a> {
    fd_array_offset: IntegerNumber<'a>,
    fd_select_offset: IntegerNumber<'a>,
    private_dicts_offsets: Vec<(IntegerNumber<'a>, IntegerNumber<'a>)>,
}

enum ContextType<'a> {
    SIDWriteContext(SIDWriteContext<'a>),
    CIDWriteContext(CIDWriteContext<'a>),
}

struct FontWriteContext<'a> {
    // TOP DICT DATA
    charset_offset: IntegerNumber<'a>,
    encoding_offset: IntegerNumber<'a>,
    char_strings_offset: IntegerNumber<'a>,
    context_type: ContextType<'a>
}

impl FontWriteContext<'_> {
    pub fn new_cid(num_font_dicts: u8) -> Self {
        Self {
            char_strings_offset: IntegerNumber::from_i32_as_int5(0),
            encoding_offset: IntegerNumber::from_i32_as_int5(0),
            charset_offset: IntegerNumber::from_i32_as_int5(0),
            context_type: ContextType::CIDWriteContext(CIDWriteContext {
                fd_select_offset: IntegerNumber::from_i32_as_int5(0),
                fd_array_offset: IntegerNumber::from_i32_as_int5(0),
                private_dicts_offsets: vec![
                    (
                        IntegerNumber::from_i32_as_int5(0),
                        IntegerNumber::from_i32_as_int5(0)
                    );
                    num_font_dicts as usize
                ],
            })
        }
    }

    pub fn new_sid() -> Self {
        Self {
            char_strings_offset: IntegerNumber::from_i32_as_int5(0),
            encoding_offset: IntegerNumber::from_i32_as_int5(0),
            charset_offset: IntegerNumber::from_i32_as_int5(0),
            context_type: ContextType::SIDWriteContext(SIDWriteContext {
                private_dict_offset:  (
                    IntegerNumber::from_i32_as_int5(0),
                    IntegerNumber::from_i32_as_int5(0)
                )
            })
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
        match table.font_kind {
            FontKind::CID(cid) => {
                let subroutines = cid
                    .font_dicts
                    .iter()
                    .map(|font_dict| font_dict.local_subrs.into_iter().collect::<Vec<_>>())
                    .collect::<Vec<_>>();
                SubroutineCollection::new(subroutines)
            }
            FontKind::SID(sid) => {
                let subroutines = sid.local_subrs.into_iter().collect::<Vec<_>>();
                SubroutineCollection::new(vec![subroutines])
            }
        }
    };

    let mut used_fds = BTreeSet::new();
    let sid_remapper = get_sid_remapper(&table);
    let mut char_strings = vec![];

    for old_gid in ctx.mapper.old_gids() {
        let fd_index = match table.font_kind {
            FontKind::CID(ref cid) => {
                let fd_index = cid.fd_select.font_dict_index(old_gid).unwrap();
                used_fds.insert(fd_index);
                fd_index
            }
            FontKind::SID(_) => 0
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

    let mut font_write_context = FontWriteContext::new(fd_remapper.len());
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

        font_write_context.charset_offset =
            IntegerNumber::from_i32_as_int5(w.len() as i32);
        // Charsets
        w.extend(&write_charset(&sid_remapper, &table.charset, &ctx.mapper).unwrap());

        font_write_context.fd_select_offset =
            IntegerNumber::from_i32_as_int5(w.len() as i32);
        // FDSelect
        w.extend(&build_fd_index(
            &ctx.mapper,
            table.cid_metadata.fd_select,
            &fd_remapper,
        )?);

        // Charstrings INDEX
        font_write_context.char_strings_offset =
            IntegerNumber::from_i32_as_int5(w.len() as i32);
        w.extend(&create_index(char_strings.iter().map(|p| p.compile()).collect())?);

        // FD Array
        font_write_context.fd_array_offset =
            IntegerNumber::from_i32_as_int5(w.len() as i32);
        w.extend(&write_font_dict_index(
            &fd_remapper,
            &sid_remapper,
            &mut font_write_context,
            &table.cid_metadata,
        )?);

        write_private_dicts(
            &fd_remapper,
            &mut font_write_context,
            &table.cid_metadata,
            &mut w,
        )?;

        subsetted_font = w.finish();
    }

    // let table = ttf_parser::cff::Table::parse(&subsetted_font).unwrap();
    // let mut sink = Sink(vec![]);
    // table.outline(GlyphId(1), &mut sink).unwrap();
    //
    // println!("{:?}", sink);

    ctx.push(Tag::CFF, subsetted_font);

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

fn get_sid_remapper(table: &Table) -> SidRemapper {
    let mut sid_remapper = SidRemapper::new();
    for sid in &table.top_dict_data.used_sids {
        sid_remapper.remap(*sid);
    }

    match table.font_kind {
        FontKind::SID(sid) => {

        }
        FontKind::CID(ref cid) => {
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

        let cid_metadata =
            cid_font::parse_cid_metadata(cff, &top_dict_data, number_of_glyphs)
                .ok_or(MalformedFont)?;

        Ok(Self {
            header,
            names,
            raw_top_dict,
            top_dict_data,
            strings,
            global_subrs,
            charset,
            char_strings,
            cid_metadata,
        })
    }
}

#[derive(Debug, Default, PartialEq)]
struct Sink(Vec<Inst>);

#[derive(Debug, PartialEq)]
enum Inst {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    QuadTo(f32, f32, f32, f32),
    CurveTo(f32, f32, f32, f32, f32, f32),
    Close,
}

impl ttf_parser::OutlineBuilder for Sink {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(Inst::MoveTo(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(Inst::LineTo(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.push(Inst::QuadTo(x1, y1, x, y));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.push(Inst::CurveTo(x1, y1, x2, y2, x, y));
    }

    fn close(&mut self) {
        self.0.push(Inst::Close);
    }
}
