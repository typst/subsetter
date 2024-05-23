mod argstack;
mod charset;
mod charstring;
mod cid_font;
mod dict;
mod index;
mod number;
mod operator;
mod remapper;
mod sid_font;
mod subroutines;

use super::*;
use crate::cff::charset::{parse_charset, rewrite_charset, Charset};
use crate::cff::charstring::Decompiler;
use crate::cff::cid_font::{generate_fd_index, rewrite_fd_index, CIDMetadata};
use crate::cff::dict::font_dict::{generate_font_dict_index, rewrite_font_dict_index};
use crate::cff::dict::private_dict::{
    rewrite_cid_private_dict, rewrite_cid_private_dicts,
};
use crate::cff::dict::top_dict::{
    parse_top_dict_index, rewrite_top_dict_index, TopDictData,
};
use crate::cff::index::{create_index, parse_index, skip_index, Index, OwnedIndex};
use crate::cff::remapper::{FontDictRemapper, SidRemapper};
use crate::cff::sid_font::SIDMetadata;
use crate::cff::subroutines::{SubroutineCollection, SubroutineContainer};
use crate::Error::SubsetError;
use charset::charset_id;
use number::{IntegerNumber, StringId};
use std::cmp::PartialEq;
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
    top_dict_data: TopDictData<'a>,
    strings: Index<'a>,
    global_subrs: Index<'a>,
    charset: Charset<'a>,
    char_strings: Index<'a>,
    font_kind: FontKind<'a>,
}

/// An offset that needs to be written after the whole font
/// has been written. location indicates where in the buffer the offset needs to be written to
/// and value indicates the value of the offset.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct DeferredOffset {
    location: usize,
    value: IntegerNumber,
}

const DUMMY_VALUE: IntegerNumber = IntegerNumber(0);
/// This represents a not-existing offset.
const DUMMY_OFFSET: DeferredOffset = DeferredOffset { location: 0, value: DUMMY_VALUE };

impl DeferredOffset {
    fn update_value(&mut self, value: usize) -> Result<()> {
        self.value = IntegerNumber(i32::try_from(value).map_err(|_| SubsetError)?);
        Ok(())
    }

    /// Adjust the location of an offset, if it has been set (i.e. it's not a dummy offset).
    fn adjust_location(&mut self, delta: usize) {
        if *self != DUMMY_OFFSET {
            self.location += delta;
        }
    }

    fn update_location(&mut self, location: usize) {
        self.location = location;
    }

    /// Write the deferred offset into a buffer.
    fn write_into(&self, buffer: &mut [u8]) -> Result<()> {
        let mut w = Writer::new();
        // Always write using 5 bytes, to prevent its size from changing.
        self.value.write_as_5_bytes(&mut w);
        let encoded = w.finish();
        let pos = buffer.get_mut(self.location..self.location + 5).ok_or(SubsetError)?;

        pos.copy_from_slice(&encoded);

        Ok(())
    }
}

/// Keeps track of the offsets that need to be written in the font.
#[derive(Debug)]
struct Offsets {
    // TOP DICT DATA
    charset_offset: DeferredOffset,
    encoding_offset: DeferredOffset,
    char_strings_offset: DeferredOffset,
    private_dicts_lens: Vec<DeferredOffset>,
    private_dicts_offsets: Vec<DeferredOffset>,
    fd_array_offset: DeferredOffset,
    fd_select_offset: DeferredOffset,
    registry_sid: StringId,
    ordering_sid: StringId,
}

impl Offsets {
    // TODO: Unify?
    pub fn new_cid(num_font_dicts: u8) -> Self {
        Self {
            char_strings_offset: DUMMY_OFFSET,
            encoding_offset: DUMMY_OFFSET,
            charset_offset: DUMMY_OFFSET,
            private_dicts_lens: vec![DUMMY_OFFSET; num_font_dicts as usize],
            private_dicts_offsets: vec![DUMMY_OFFSET; num_font_dicts as usize],
            fd_select_offset: DUMMY_OFFSET,
            fd_array_offset: DUMMY_OFFSET,
            registry_sid: StringId(0),
            ordering_sid: StringId(0),
        }
    }

    pub fn new_sid() -> Self {
        Self {
            char_strings_offset: DUMMY_OFFSET,
            encoding_offset: DUMMY_OFFSET,
            charset_offset: DUMMY_OFFSET,
            private_dicts_lens: vec![DUMMY_OFFSET],
            private_dicts_offsets: vec![DUMMY_OFFSET],
            fd_select_offset: DUMMY_OFFSET,
            fd_array_offset: DUMMY_OFFSET,
            registry_sid: StringId(0),
            ordering_sid: StringId(0),
        }
    }
}

pub fn subset(ctx: &mut Context<'_>) -> Result<()> {
    let table = Table::parse(ctx).unwrap();

    let sid_remapper = get_sid_remapper(&table, &ctx.mapper);
    // NOTE: The charstrings are already in the new order that they need be written in.
    let (char_strings, fd_remapper) = subset_charstrings(&table, &ctx.mapper)?;

    let mut offsets = match &table.font_kind {
        FontKind::Sid(_) => Offsets::new_sid(),
        FontKind::Cid(_) => Offsets::new_cid(fd_remapper.len()),
    };

    let mut subsetted_font = {
        let mut w = Writer::new();
        // HEADER
        w.write(table.header);
        // Name INDEX
        w.write(table.names);

        // Get the strings that will be written into the String INDEX. This is necessary to do
        // now because we need to push the strings for ROS (since SID-keyed fonts, which will be
        // converted to CID-keyed, don't have this operator). So we need to do this before writing
        // the Top DICT Index.
        let new_strings = {
            let mut new_strings = vec![];
            for sid in sid_remapper.sids() {
                new_strings.push(
                    table
                        .strings
                        .get(sid.0.checked_sub(StringId::STANDARD_STRING_LEN).unwrap()
                            as u32)
                        .unwrap()
                        .to_vec(),
                );
            }

            offsets.registry_sid =
                StringId(new_strings.len() as u16 + StringId::STANDARD_STRING_LEN);
            new_strings.push(Vec::from(b"Adobe"));
            offsets.ordering_sid =
                StringId(new_strings.len() as u16 + StringId::STANDARD_STRING_LEN);
            new_strings.push(Vec::from(b"Identity"));
            new_strings
        };

        // Top DICT INDEX
        rewrite_top_dict_index(
            table.top_dict_data.top_dict_raw,
            &mut offsets,
            &sid_remapper,
            &mut w,
        )?;
        // String INDEX
        let index = create_index(new_strings)?;
        w.extend(&index.data);
        // Global Subr INDEX
        // We desubroutinized, so no global subroutines and thus empty index.
        w.write(&OwnedIndex::default());

        offsets.charset_offset.update_value(w.len())?;
        // Charsets
        rewrite_charset(&ctx.mapper, &mut w)?;

        match &table.font_kind {
            FontKind::Sid(sid) => {
                // Since we convert SID-keyed to CID-keyed, we write one private dict with index 0.
                rewrite_cid_private_dict(&mut offsets, sid.private_dict_data, &mut w, 0)?;
            }
            FontKind::Cid(cid) => {
                rewrite_cid_private_dicts(&fd_remapper, &mut offsets, cid, &mut w)?;
            }
        }

        offsets.fd_select_offset.update_value(w.len())?;
        // FDSelect
        match &table.font_kind {
            FontKind::Sid(_) => generate_fd_index(&ctx.mapper, &mut w)?,
            FontKind::Cid(cid_metadata) => rewrite_fd_index(
                &ctx.mapper,
                cid_metadata.fd_select,
                &fd_remapper,
                &mut w,
            )?,
        };

        // FDArray
        offsets.fd_array_offset.update_value(w.len())?;
        match &table.font_kind {
            FontKind::Sid(_) => generate_font_dict_index(&mut offsets, &mut w)?,
            FontKind::Cid(cid_metadata) => rewrite_font_dict_index(
                &fd_remapper,
                &sid_remapper,
                &mut offsets,
                cid_metadata,
                &mut w,
            )?,
        }

        // Charstrings INDEX
        offsets.char_strings_offset.update_value(w.len())?;
        w.extend(&create_index(char_strings)?.data);

        w.finish()
    };

    // Rewrite the dummy offsets.
    update_offsets(&offsets, subsetted_font.as_mut_slice())?;

    ctx.push(Tag::CFF, subsetted_font);

    Ok(())
}

fn update_offsets(font_write_context: &Offsets, buffer: &mut [u8]) -> Result<()> {
    let mut write = |offset: DeferredOffset| {
        if offset != DUMMY_OFFSET {
            offset.write_into(buffer)?;
        }
        Ok(())
    };

    write(font_write_context.encoding_offset)?;
    write(font_write_context.charset_offset)?;
    write(font_write_context.char_strings_offset)?;
    write(font_write_context.fd_select_offset)?;
    write(font_write_context.fd_array_offset)?;

    Ok(())
}

/// Create the list of bytes that constitute the programs of the charstrings, sorted in the new glyph order.
fn subset_charstrings<'a>(
    table: &Table,
    remapper: &GlyphRemapper,
) -> Result<(Vec<Vec<u8>>, FontDictRemapper)> {
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
    let mut char_strings = vec![];

    for old_gid in remapper.remapped_gids() {
        let fd_index = match &table.font_kind {
            FontKind::Cid(ref cid) => {
                let fd_index =
                    cid.fd_select.font_dict_index(old_gid).ok_or(MalformedFont)?;
                used_fds.insert(fd_index);
                fd_index
            }
            FontKind::Sid(_) => 0,
        };

        let decompiler = Decompiler::new(
            gsubrs.get_handler(),
            lsubrs.get_handler(fd_index).ok_or(MalformedFont)?,
        );
        let charstring = table.char_strings.get(old_gid as u32).ok_or(MalformedFont)?;
        char_strings.push(decompiler.decompile(charstring)?);
    }

    let mut fd_remapper = FontDictRemapper::new();

    for fd in used_fds {
        fd_remapper.remap(fd);
    }

    Ok((char_strings.iter().map(|p| p.compile()).collect(), fd_remapper))
}

/// Get the SID remapper
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

/// A high-level container that contains important information we need when
/// accesseing the data in the CFF font.
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
        let top_dict_data = parse_top_dict_index(&mut r).ok_or(MalformedFont)?;

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
            top_dict_data,
            strings,
            global_subrs,
            charset,
            char_strings,
            font_kind,
        })
    }
}
