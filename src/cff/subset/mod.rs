mod char_strings;
mod charset;
mod sid;
mod top_dict;

use crate::cff::subset::charset::subset_charset;
use crate::cff::subset::sid::get_sid_remapper;
use crate::cff::subset::top_dict::update_top_dict;
use crate::cff::{Table, TopDict};
use crate::stream::StringId;
use crate::Error::{MalformedFont, SubsetError};
use crate::{Context, Tag};
use std::collections::HashMap;

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

    let sid_remapper =
        get_sid_remapper(&parsed_table, &ctx.requested_glyphs).ok_or(SubsetError)?;

    let charset =
        subset_charset(&parsed_table.charset, ctx, &sid_remapper).ok_or(SubsetError)?;

    let top_dict =
        update_top_dict(&parsed_table.top_dict, sid_remapper).ok_or(SubsetError)?;

    Ok(())
}
