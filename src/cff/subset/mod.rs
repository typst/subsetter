mod sid;

use crate::cff::subset::sid::get_sid_remapper;
use crate::cff::Table;
use crate::stream::StringId;
use crate::Error::MalformedFont;
use crate::{parse, Context, Tag};
use std::collections::{HashMap, HashSet};

struct SubsetContext {
    sid_mapper: HashMap<StringId, StringId>,
}

struct SubsettedTable<'a> {
    header: &'a [u8],
    names: &'a [u8],
}

pub(crate) fn subset(ctx: &mut Context) -> crate::Result<()> {
    let name = ctx.expect_table(Tag::CFF).ok_or(MalformedFont)?;
    let parsed_table = Table::parse(ctx)?;

    let header = parsed_table.header;
    let names = parsed_table.names;

    get_sid_remapper(&parsed_table, &ctx.requested_glyphs);

    Ok(())
}
