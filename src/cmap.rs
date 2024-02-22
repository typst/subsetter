use super::*;
use crate::Error::MissingTable;
use ttf_parser::cmap::{Format, Subtable, Subtable4};
use ttf_parser::PlatformId;

trait SubtableExt {
    fn is_unicode(&self) -> bool;
    fn is_symbol(&self) -> bool;
}

impl SubtableExt for ttf_parser::cmap::Subtable<'_> {
    fn is_unicode(&self) -> bool {
        self.platform_id == PlatformId::Unicode
            || (self.platform_id == PlatformId::Windows
                && [0, 1, 10].contains(&self.encoding_id))
    }

    fn is_symbol(&self) -> bool {
        self.platform_id == PlatformId::Unicode && self.encoding_id == 0
    }
}

// This function is heavily inspired by how fonttools does the subsetting of that
// table.
pub(crate) fn subset(ctx: &mut Context) -> Result<()> {
    let cmap = ctx.ttf_face.tables().cmap.ok_or(MissingTable(Tag::CMAP))?;
    let mut writer = Writer::new();

    for table in cmap.subtables {
        if !table.is_unicode() {
            continue;
        }

        // We don't support unicode variation sequences
        if matches!(table.format, Format::UnicodeVariationSequences(_)) {
            continue;
        }

        match table.format {
            // Only those 2 formats are actually used in practice for unicode subtables
            // (verified this locally with 300+ fonts)
            Format::SegmentMappingToDeltaValues(table) => {
                subset_subtable4(ctx, &mut writer, &table)?;
            }
            Format::SegmentedCoverage(_) => {}
            _ => {}
        }
    }

    // println!("{:?}", r);

    Ok(())
}

fn subset_subtable4(
    ctx: &mut Context,
    writer: &mut Writer,
    subtable: &Subtable4,
) -> Result<Vec<u8>> {
    let mut writer = Writer::new();
    let mut all_codepoints = vec![];
    subtable.codepoints(|c| all_codepoints.push(c));

    let new_mappings = all_codepoints
        .into_iter()
        .filter_map(|c| {
            if let Some(g) = subtable.glyph_index(c) {
                if ctx.subset.contains(&g.0) {
                    if let Some(new_g) = ctx.gid_map.get(&g.0) {
                        return Some((c, new_g));
                    }
                }
            }

            return None;
        })
        .collect::<Vec<_>>();

    println!("{:?}", new_mappings);

    Ok(vec![])
}
