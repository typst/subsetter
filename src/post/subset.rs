use crate::post::read::Version2Table;
use crate::stream::{Writeable, Writer};
use crate::Context;

#[derive(Clone, Debug)]
pub struct SubsettedVersion2Table<'a> {
    header: &'a [u8],
    glyph_indexes: Vec<u16>,
    names_data: Vec<&'a str>,
}

pub fn subset<'a>(
    ctx: &Context,
    table: &Version2Table<'a>,
) -> Option<SubsettedVersion2Table<'a>> {
    let old_names = table.names().collect::<Vec<_>>();

    let num_glyphs = ctx.mapper.num_gids();
    let mut glyph_indexes = Vec::with_capacity(num_glyphs as usize);

    let mut names_data = Vec::new();

    let mut count = 0;
    for i in 0..num_glyphs {
        let old_gid = ctx.mapper.get_reverse(i).unwrap();
        let index = *table.glyph_indexes.get(old_gid as usize)?;

        if index <= 257 {
            glyph_indexes.push(index);
            continue;
        }

        let index = index - 258;

        let name = *old_names.get(usize::from(index))?;
        glyph_indexes.push(count + 258);
        names_data.push(name);
        count += 1;
    }

    Some(SubsettedVersion2Table { header: table.header, glyph_indexes, names_data })
}

impl Writeable for SubsettedVersion2Table<'_> {
    fn write(&self, w: &mut Writer) {
        w.extend(self.header);
        w.write::<u16>(u16::try_from(self.glyph_indexes.len()).unwrap());

        for index in &self.glyph_indexes {
            w.write::<u16>(*index);
        }

        for name in &self.names_data {
            w.write::<u8>(u8::try_from(name.len()).unwrap());
            w.extend(name.as_bytes());
        }
    }
}
