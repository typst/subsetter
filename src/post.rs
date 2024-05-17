use super::*;
use crate::read::LazyArray16;
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

    for old_gid in ctx.mapper.old_gids() {
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

/// An iterator over glyph names.
///
/// The `post` table doesn't provide the glyph names count,
/// so we have to simply iterate over all of them to find it out.
#[derive(Clone, Copy, Default)]
pub struct Names<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for Names<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        // Glyph names are stored as Pascal Strings.
        // Meaning u8 (len) + [u8] (data).

        if self.offset >= self.data.len() {
            return None;
        }

        let len = self.data[self.offset];
        self.offset += 1;

        // An empty name is an error.
        if len == 0 {
            return None;
        }

        let name = self.data.get(self.offset..self.offset + usize::from(len))?;
        self.offset += usize::from(len);
        Some(name)
    }
}

#[derive(Clone, Debug)]
pub struct Version2Table<'a> {
    pub header: &'a [u8],
    pub glyph_indexes: LazyArray16<'a, u16>,
    pub names_data: &'a [u8],
}

impl<'a> Version2Table<'a> {
    /// Parse a version 2 table.
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        // Do not check the exact length, because some fonts include
        // padding in table's length in table records, which is incorrect.
        if data.len() < 32 || Reader::new(data).read::<u32>()? != 0x00020000 {
            return None;
        }

        let mut r = Reader::new(data);
        let header = r.read_bytes(32)?;

        let indexes_count = r.read::<u16>()?;
        let glyph_indexes = r.read_array16::<u16>(indexes_count)?;
        let names_data = r.tail()?;

        Some(Version2Table { header, glyph_indexes, names_data })
    }

    /// Returns an iterator over glyph names.
    ///
    /// Default/predefined names are not included. Just the one in the font file.
    pub fn names(&self) -> Names<'a> {
        Names { data: self.names_data, offset: 0 }
    }
}
