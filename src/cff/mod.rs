mod dict;
mod encoding;
mod index;
mod top_dict;
mod charset;

use crate::cff::charset::{Charset, charset_id, parse_charset};
use super::*;
use crate::cff::dict::Operator;
use crate::cff::index::{parse_index, Index};
use crate::cff::top_dict::parse_top_dict;
use crate::Error::InvalidData;

// Limits according to the Adobe Technical Note #5176, chapter 4 DICT Data.
const MAX_OPERANDS_LEN: usize = 48;

struct SIDMapper(Vec<String>);

impl SIDMapper {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn insert(&mut self, string: String) -> u16 {
        let sid = self.0.iter().position(|s| *s == string).unwrap_or_else(|| {
            self.0.push(string);
            self.0.len() - 1
        }) + 391;
        u16::try_from(sid).unwrap()
    }
}

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let cff = ctx.expect_table(Tag::CFF)?;

    let mut r = Reader::new(cff);

    // Parse Header.
    let major = r.read::<u8>()?;
    r.skip::<u8>()?; // minor
    let header_size = r.read::<u8>()?;
    r.skip::<u8>()?; // Absolute offset

    if major != 1 {
        return Err(Error::Unimplemented);
    }

    // Jump to Name INDEX. It's not necessarily right after the header.
    if header_size > 4 {
        r.advance(usize::from(header_size) - 4)?;
    }

    let name_index_start = r.offset();
    let _ = parse_index::<u16>(&mut r)?;
    let top_dict_index_start = r.offset();

    // let name_index_data = &cff[name_index_start..top_dict_index_start];

    let top_dict = parse_top_dict(&mut r).ok_or(InvalidData)?;

    let mut strings = parse_index::<u16>(&mut r)?
        .into_iter()
        // TODO: Remove this
        .map(|s| std::str::from_utf8(s).unwrap())
        .enumerate()
        .collect::<HashMap<_, _>>();

    // Skip global subrs for now
    let _ = parse_index::<u16>(&mut r)?;

    let mut num_glyphs = 0;

    let char_strings = top_dict.get(&Operator(17)).and_then(|offset| {
        let offset = offset.get(0).map(|o| *o as usize)?;
        let mut cs_r = Reader::new_at(cff, offset).ok()?;
        let char_strings_index = parse_index::<u16>(&mut cs_r).ok()?;
        num_glyphs = u16::try_from(char_strings_index.len()).unwrap();

        Some(char_strings_index
            .into_iter()
            .enumerate()
            .map(|(index, data)| (u16::try_from(index).unwrap(), data))
            .filter(|(index, data)| ctx.requested_glyphs.contains(index))
            .collect::<HashMap<_, _>>())
    });

    let charset = top_dict.get(&Operator(15)).and_then(|offset| {
        let offset = offset.get(0).map(|o| *o as usize);

        match offset {
            Some(charset_id::ISO_ADOBE) => Some(Charset::ISOAdobe),
            Some(charset_id::EXPERT) => Some(Charset::Expert),
            Some(charset_id::EXPERT_SUBSET) => Some(Charset::ExpertSubset),
            Some(offset) => {
                let mut r = Reader::new_at(cff, offset).ok()?;
                parse_charset(&mut r, num_glyphs)
            }
            None => Some(Charset::ISOAdobe), // default
        }
    }).ok_or(InvalidData)?;

    Ok(())
}

pub(crate) fn discover(ctx: &mut Context) -> Result<()> {
    ctx.subset.insert(0);
    ctx.subset
        .extend(ctx.requested_glyphs.iter().filter(|g| **g < ctx.num_glyphs));
    Ok(())
}
