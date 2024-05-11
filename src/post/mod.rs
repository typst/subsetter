mod read;

use super::*;
use crate::post::read::Version2Table;
use crate::Error::{MalformedFont, SubsetError};

pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let post = ctx.expect_table(Tag::POST).ok_or(MalformedFont)?;
    let mut r = Reader::new(post);

    // Version 2 is the only one worth subsetting.
    let version = r.read::<u32>().ok_or(MalformedFont)?;
    if version != 0x00020000 {
        ctx.push(Tag::POST, post);
        return Ok(());
    }

    let table = Version2Table::parse(post).ok_or(MalformedFont)?;
    let names = table.names().collect::<Vec<_>>();

    let mut sub_post = Writer::new();
    sub_post.extend(table.header);
    sub_post.write(ctx.mapper.num_gids());

    let mut string_storage = Writer::new();
    let mut string_index = 0;

    for i in 0..ctx.mapper.num_gids() {
        let old_gid = ctx.mapper.get_reverse(i).unwrap();
        let index = table.glyph_indexes.get(old_gid).ok_or(MalformedFont)?;

        if index <= 257 {
            sub_post.write(index);
        } else {
            let index = index - 258;
            let name = *names.get(index as usize).ok_or(MalformedFont)?;
            let name_len = u8::try_from(name.len()).map_err(|_| MalformedFont)?;
            let index = u16::try_from(string_index + 258).map_err(|_| SubsetError)?;
            sub_post.write(index);

            string_storage.write(name_len);
            string_storage.write(name);
            string_index += 1;
        }
    }

    sub_post.extend(&string_storage.finish());

    ctx.push(Tag::POST, sub_post.finish());
    Ok(())
}
