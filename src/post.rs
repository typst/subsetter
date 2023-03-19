use super::*;

/// Subset the glyf and loca tables by removing glyph data for unused glyphs.
pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let post = ctx.expect_table(Tag::POST)?;
    let mut r = Reader::new(post);

    // Version 2 is the only one worth subsetting.
    let version = r.read::<u32>()?;
    if version != 0x00020000 {
        ctx.push(Tag::POST, post);
        return Ok(());
    }

    // Reader remaining header.
    let header = r.take(28)?;

    // Read glyph name table.
    let num_glyphs = r.read::<u16>()?;
    let mut indices = vec![];
    for _ in 0..num_glyphs {
        indices.push(r.read::<u16>()?);
    }

    // Read the strings.
    let mut strings = vec![];
    while !r.eof() {
        let len = r.read::<u8>()?;
        strings.push(r.take(len as usize)?);
    }

    // Start writing a new subsetted post table.
    let mut sub_post = Writer::new();
    sub_post.write::<u32>(0x00020000);
    sub_post.give(header);
    sub_post.write::<u16>(num_glyphs);

    let mut sub_strings = Writer::new();
    let mut count = 0;
    for (i, mut index) in indices.into_iter().enumerate() {
        // Rewrite unused glyphs to .notdef.
        if !ctx.subset.contains(&(i as u16)) {
            index = 0;
        }

        if index <= 257 {
            sub_post.write::<u16>(index);
            continue;
        }

        let index = index - 258;
        let name = strings.get(index as usize).ok_or(Error::InvalidOffset)?;
        sub_post.write::<u16>(count + 258);
        sub_strings.write::<u8>(name.len() as u8);
        sub_strings.give(name);
        count += 1;
    }

    sub_post.give(&sub_strings.finish());
    ctx.push(Tag::POST, sub_post.finish());

    Ok(())
}
