use crate::write::Writer;
use crate::CidMapping;
use crate::GlyphRemapper;
use crate::Result;

/// Rewrite the charset of the font. We do not preserve the CIDs from the original font. Instead,
/// each glyph gets as its CID either its glyph ID in the subset or, with
/// [`CidMapping::OriginalGlyphId`], its glyph ID in the original font. Either way, the CID of a
/// glyph is known before the font is subsetted, which makes it easy to reference from the PDF.
pub fn rewrite_charset(
    gid_mapper: &GlyphRemapper,
    cid_mapping: CidMapping,
    w: &mut Writer,
) -> Result<()> {
    if gid_mapper.num_gids() == 1 {
        // We only have .notdef, so use format 0.
        w.write::<u8>(0);
        return Ok(());
    }

    // Use format 2.
    w.write::<u8>(2);

    match cid_mapping {
        CidMapping::SubsetGlyphId => {
            w.write::<u16>(1);
            // -2 because -1 for not including .notdef and -1 since the first glyph
            // in the range is not counted.
            w.write::<u16>(gid_mapper.num_gids() - 2);
        }
        CidMapping::OriginalGlyphId => {
            // One range for each run of consecutive old glyph IDs, in the order of the
            // new glyph IDs. .notdef is not part of the charset.
            let mut gids = gid_mapper.remapped_gids().skip(1);
            let mut first = gids.next().unwrap();
            let mut last = first;

            for gid in gids {
                if gid == last + 1 {
                    last = gid;
                } else {
                    w.write::<u16>(first);
                    w.write::<u16>(last - first);
                    first = gid;
                    last = gid;
                }
            }

            w.write::<u16>(first);
            w.write::<u16>(last - first);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn charset(glyphs: &[u16], cid_mapping: CidMapping) -> Vec<u8> {
        let mut w = Writer::new();
        rewrite_charset(&GlyphRemapper::new_from_glyphs(glyphs), cid_mapping, &mut w)
            .unwrap();
        w.finish()
    }

    #[test]
    fn subset_glyph_ids() {
        assert_eq!(charset(&[], CidMapping::SubsetGlyphId), [0]);
        assert_eq!(
            charset(&[5, 6, 7, 20, 3], CidMapping::SubsetGlyphId),
            [2, 0, 1, 0, 4]
        );
    }

    #[test]
    fn original_glyph_ids() {
        assert_eq!(charset(&[], CidMapping::OriginalGlyphId), [0]);
        assert_eq!(charset(&[7], CidMapping::OriginalGlyphId), [2, 0, 7, 0, 0]);
        assert_eq!(
            charset(&[5, 6, 7, 20, 3], CidMapping::OriginalGlyphId),
            [2, 0, 5, 0, 2, 0, 20, 0, 0, 0, 3, 0, 0]
        );
    }
}
