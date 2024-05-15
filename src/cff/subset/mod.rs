mod argstack;


use crate::cff::argstack::ArgumentsStack;
use crate::cff::{operator, FDSelect, Table, MAX_ARGUMENTS_STACK_LEN, Remapper};
use crate::stream::{Fixed, Reader};
use crate::Error::{MalformedFont, Unimplemented};
use crate::Result;
use crate::{Context};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

struct CharStringParserContext {
    width: Option<f32>,
    stems_len: u32,
}

pub(crate) fn subset_charstrings(
    ctx: &mut Context,
    table: &Table,
    gsubr_remapper: &mut Remapper,
    lsubr_remapper: Vec<&mut Remapper>
) -> Result<Vec<u8>> {
    let mut subsetted_char_strings = vec![];

    for charstring in table.char_strings {
        subsetted_char_strings.push(subset_charstring(ctx, table, gsubr_remapper, &lsubr_remapper, charstring)?);
    }

    Ok(subsetted_char_strings)
}

fn subset_charstring(
    ctx: &mut Context,
    table: &Table,
    gsubr_remapper: &mut Remapper,
    lsubr_remapper: &[&mut Remapper],
    charstring: &[u8]
) {

}

fn discover_subrs(
    gsubr: &mut HashSet<u32>,
    lsubr: &mut HashSet<u32>,
    num_gsubr: u32,
    num_lsubr: u32,
    char_string: &[u8],
) -> Result<()> {
    let mut stems_len = 0;
    let mut width = None;

    let mut stack = ArgumentsStack {
        data: &mut [0.0; MAX_ARGUMENTS_STACK_LEN], // 192B
        len: 0,
        max_len: MAX_ARGUMENTS_STACK_LEN,
    };

    let mut r = Reader::new(char_string);

    while !r.at_end() {
        let op = r.read::<u8>().ok_or(MalformedFont)?;
        match op {
            0 | 2 | 9 | 13 | 15 | 16 | 17 => {
                // Reserved.
                return Err(MalformedFont);
            }
            operator::HORIZONTAL_STEM
            | operator::VERTICAL_STEM
            | operator::HORIZONTAL_STEM_HINT_MASK
            | operator::VERTICAL_STEM_HINT_MASK => {
                let len = if stack.len() % 2 == 1 && width.is_none() {
                    width = Some(stack.at(0));
                    stack.len() - 1
                } else {
                    stack.len()
                };

                stems_len += len as u32 >> 1;

                stack.clear();
            }
            operator::CALL_LOCAL_SUBROUTINE => {
                let biased_subroutine = stack.pop();
                let subroutine = conv_subroutine_index(
                    biased_subroutine,
                    calc_subroutine_bias(num_lsubr),
                )
                .ok_or(MalformedFont)?;
                lsubr.insert(subroutine);

                stack.clear();
            }
            operator::SHORT_INT => {
                stack.push(f32::from(r.read::<i16>().ok_or(MalformedFont)?))?;
            }
            operator::CALL_GLOBAL_SUBROUTINE => {
                let biased_subroutine = stack.pop();
                let subroutine = conv_subroutine_index(
                    biased_subroutine,
                    calc_subroutine_bias(num_gsubr),
                )
                .ok_or(MalformedFont)?;
                gsubr.insert(subroutine);

                stack.clear();
            }
            operator::ENDCHAR => {
                if stack.len() == 4 {
                    // We don't support seac.
                    return Err(Unimplemented);
                }
            }
            operator::HINT_MASK | operator::COUNTER_MASK => {
                let mut len = stack.len();
                stack.clear();

                stems_len += len as u32 >> 1;

                r.skip_bytes(((stems_len + 7) >> 3) as usize);
            }
            // Two byte operator
            12 => {
                r.read::<u8>().ok_or(MalformedFont)?;
                stack.clear();
            }
            32..=246 => {
                stack.push(parse_int1(op)?)?;
            }
            247..=250 => {
                stack.push(parse_int2(op, &mut r)?)?;
            }
            251..=254 => {
                stack.push(parse_int3(op, &mut r)?)?;
            }
            operator::FIXED_16_16 => {
                stack.push(parse_fixed(&mut r)?)?;
            }
            _ => {
                stack.clear();
            }
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

fn remap_font_dicts(ctx: &Context, fd_select: &FDSelect) -> Option<Remapper> {
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
