mod char_strings;
mod charset;
mod sid;
mod top_dict;

use crate::cff::subset::charset::subset_charset;
use crate::cff::subset::sid::SidRemapper;
use crate::cff::subset::top_dict::update_top_dict;
use crate::cff::{operator, FDSelect, FontKind, Table, TopDict, MAX_ARGUMENTS_STACK_LEN};
use crate::stream::{Fixed, Reader};
use crate::Error::{MalformedFont, SubsetError};
use crate::Result;
use crate::{Context, Tag};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

#[derive(Clone)]
pub struct Remapper<T: Hash + Eq + PartialEq + From<u16>> {
    counter: u16,
    map: HashMap<T, T>,
}

impl<T: Hash + Eq + PartialEq + From<u16>> Remapper<T> {
    pub fn new() -> Self {
        Self { counter: 0, map: HashMap::new() }
    }

    pub(crate) fn new_from(start: u16) -> Self {
        Self { counter: start, map: HashMap::new() }
    }

    pub fn remap(&mut self, item: T) -> T
    where
        T: Copy,
    {
        *self.map.entry(item).or_insert_with(|| {
            let new_id = self.counter;
            self.counter = self
                .counter
                .checked_add(1)
                .expect("remapper contains too many strings");
            new_id.into()
        })
    }
}

struct SubsettedTable<'a> {
    header: &'a [u8],
    names: &'a [u8],
    top_dict: TopDict,
}

pub(crate) fn subset(ctx: &mut Context) -> crate::Result<()> {
    let name = ctx.expect_table(Tag::CFF).ok_or(MalformedFont)?;
    let parsed_table = Table::parse(ctx)?;

    let header = parsed_table.header;
    let names = parsed_table.names;

    let mut sid_remapper = SidRemapper::new();

    let charset = subset_charset(&parsed_table.charset, ctx, &mut sid_remapper)
        .ok_or(SubsetError)?;

    if let Some(FontKind::CID(cid_metadata)) = &parsed_table.kind {
        let mut fd_remapper =
            remap_font_dicts(ctx, &cid_metadata.fd_select).ok_or(MalformedFont)?;
        let mut used_gsubs = HashSet::new();
        let mut used_lsubs: Vec<HashSet<u32>> = [HashSet::new()]
            .into_iter()
            .cycle()
            .take(fd_remapper.counter as usize)
            .collect();

        let num_gsubr = parsed_table.global_subrs.len();

        for gid in 0..ctx.mapper.num_gids() {
            println!("{:?}", gid);
            let original_gid = ctx.mapper.get_reverse(gid).ok_or(SubsetError)?;
            let fd_index = cid_metadata
                .fd_select
                .font_dict_index(original_gid)
                .ok_or(MalformedFont)?;
            let fd_used_lsubs =
                used_lsubs.get_mut(fd_index as usize).ok_or(SubsetError)?;

            let num_lsubr = cid_metadata
                .local_subrs
                .get(fd_index as usize)
                .and_then(|i| i.map(|i| i.len()))
                .unwrap_or(0);

            discover_subrs(
                &mut used_gsubs,
                fd_used_lsubs,
                num_gsubr,
                num_lsubr,
                parsed_table
                    .char_strings
                    .get(original_gid as u32)
                    .ok_or(MalformedFont)?,
            )?;
        }

        println!("Local: {:?}, Global: {:?}", used_lsubs, used_gsubs);
    }

    let top_dict =
        update_top_dict(&parsed_table.top_dict, &mut sid_remapper).ok_or(SubsetError)?;

    Ok(())
}

fn discover_subrs(
    gsubr: &mut HashSet<u32>,
    lsubr: &mut HashSet<u32>,
    num_gsubr: u32,
    num_lsubr: u32,
    char_string: &[u8],
) -> Result<()> {
    let mut last_arg = None;

    let mut r = Reader::new(char_string);
    println!("{:?}", r.tail());

    while !r.at_end() {
        println!("{:?}", r.offset());
        let op = r.read::<u8>().ok_or(MalformedFont)?;
        match op {
            0 | 2 | 9 | 13 | 15 | 16 | 17 => {
                // Reserved.
                return Err(MalformedFont);
            }
            operator::CALL_LOCAL_SUBROUTINE => {
                let biased_subroutine = last_arg.ok_or(MalformedFont)?;
                let subroutine = conv_subroutine_index(
                    biased_subroutine,
                    calc_subroutine_bias(num_lsubr),
                )
                .ok_or(MalformedFont)?;
                lsubr.insert(subroutine);
            }
            operator::SHORT_INT => {
                let n = r.read::<i16>().ok_or(MalformedFont)?;
                last_arg = Some(f32::from(n));
            }
            operator::CALL_GLOBAL_SUBROUTINE => {
                let subroutine = conv_subroutine_index(
                    last_arg.ok_or(MalformedFont)?,
                    calc_subroutine_bias(num_gsubr),
                )
                .ok_or(MalformedFont)?;
                gsubr.insert(subroutine);
            }
            32..=246 => {
                last_arg = Some(parse_int1(op)?);
            }
            247..=250 => {
                last_arg = Some(parse_int2(op, &mut r)?);
            }
            251..=254 => {
                last_arg = Some(parse_int3(op, &mut r)?);
            }
            operator::FIXED_16_16 => {
                last_arg = Some(parse_fixed(&mut r)?);
            }
            _ => {}
        }
    }

    Ok(())
}

#[inline]
pub fn parse_int1(op: u8) -> Result<f32> {
    let n = i16::from(op) - 139;
    Ok(f32::from(n))
}

#[inline]
pub fn parse_int2(op: u8, r: &mut Reader) -> Result<f32> {
    let b1 = r.read::<u8>().ok_or(MalformedFont)?;
    let n = (i16::from(op) - 247) * 256 + i16::from(b1) + 108;
    debug_assert!((108..=1131).contains(&n));
    Ok(f32::from(n))
}

#[inline]
pub fn parse_int3(op: u8, r: &mut Reader) -> Result<f32> {
    let b1 = r.read::<u8>().ok_or(MalformedFont)?;
    let n = -(i16::from(op) - 251) * 256 - i16::from(b1) - 108;
    debug_assert!((-1131..=-108).contains(&n));
    Ok(f32::from(n))
}

#[inline]
pub fn parse_fixed(r: &mut Reader) -> Result<f32> {
    let n = r.read::<Fixed>().ok_or(MalformedFont)?;
    Ok(n.0)
}

fn remap_font_dicts(ctx: &Context, fd_select: &FDSelect) -> Option<Remapper<u16>> {
    let mut fds = HashSet::new();

    for glyph in &ctx.requested_glyphs {
        fds.insert(fd_select.font_dict_index(*glyph)? as u16);
    }

    let mut fds = fds.into_iter().collect::<Vec<_>>();
    fds.sort();

    let mut remapper = Remapper::new();

    for fd in fds {
        remapper.remap(fd);
    }

    Some(remapper)
}

// Adobe Technical Note #5176, Chapter 16 "Local / Global Subrs INDEXes"
#[inline]
pub fn calc_subroutine_bias(len: u32) -> u16 {
    if len < 1240 {
        107
    } else if len < 33900 {
        1131
    } else {
        32768
    }
}

fn conv_subroutine_index(index: f32, bias: u16) -> Option<u32> {
    let index = index as i32;
    let bias = i32::from(bias);

    let index = index.checked_add(bias)?;
    u32::try_from(index).ok()
}
