use super::*;
use crate::Error::MalformedFont;

/// Subset the glyf and loca tables by removing glyph data for unused glyphs.
pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let post = ctx.expect_table(Tag::POST).ok_or(MalformedFont)?;
    let mut r = Reader::new(post);

    // Version 2 is the only one worth subsetting.
    let version = r.read::<u32>().ok_or(MalformedFont)?;
    if version != 0x00020000 {
        ctx.push(Tag::POST, post);
        return Ok(());
    }

    // Reader remaining header.
    let header = r.read_bytes(28).ok_or(MalformedFont)?;

    // Read glyph name table.
    let num_glyphs = r.read::<u16>().ok_or(MalformedFont)?;
    let mut indices = vec![];
    for _ in 0..num_glyphs {
        indices.push(r.read::<u16>().ok_or(MalformedFont)?);
    }

    // Read the strings.
    let mut strings = vec![];
    while !r.at_end() {
        let len = r.read::<u8>().ok_or(MalformedFont)?;
        strings.push(r.read_bytes(len as usize).ok_or(MalformedFont)?);
    }

    let num_glyphs = ctx.subset.len() as u16;

    // Start writing a new subsetted post table.
    let mut sub_post = Writer::new();
    sub_post.write::<u32>(0x00020000);
    sub_post.extend(header);
    sub_post.write::<u16>(num_glyphs);

    let mut sub_strings = Writer::new();
    let mut count = 0;
    for i in 0..num_glyphs {
        let old_gid = ctx.mapper.get_reverse(i).unwrap();
        let index = indices[old_gid as usize];

        if index <= 257 {
            sub_post.write::<u16>(index);
            continue;
        }

        let index = index - 258;
        let name = strings.get(index as usize).ok_or(Error::SubsetError)?;
        sub_post.write::<u16>(count + 258);
        sub_strings.write::<u8>(name.len() as u8);
        sub_strings.extend(name);
        count += 1;
    }

    sub_post.extend(&sub_strings.finish());
    ctx.push(Tag::POST, sub_post.finish());

    Ok(())
}
