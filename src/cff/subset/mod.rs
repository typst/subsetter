mod char_strings;
mod charset;
mod sid;
mod top_dict;

use crate::cff::subset::charset::subset_charset;
use crate::cff::subset::sid::SidRemapper;
use crate::cff::subset::top_dict::update_top_dict;
use crate::cff::{Table, TopDict};
use crate::stream::StringId;
use crate::Error::{MalformedFont, SubsetError};
use crate::{Context, Tag};
use std::collections::HashMap;
use std::hash::Hash;

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

struct SubsetContext {
    sid_mapper: HashMap<StringId, StringId>,
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

    let top_dict =
        update_top_dict(&parsed_table.top_dict, &mut sid_remapper).ok_or(SubsetError)?;

    Ok(())
}
