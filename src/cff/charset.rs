use crate::write::Writer;
use crate::GlyphRemapper;
use crate::Result;

/// Rewrite the charset of the font. We do not perserve the CID's from the original font. Instead,
/// we assign each glyph it's original glyph as the CID. This makes it easier to reference them
/// from the PDF, since we know the CID a glyph will have before it's subsetted.
pub fn rewrite_charset(gid_mapper: &GlyphRemapper, w: &mut Writer) -> Result<()> {
    if gid_mapper.num_gids() == 1 {
        // We only have .notdef
        w.write::<u8>(0);
    } else {
        w.write::<u8>(2);
        w.write::<u16>(1);
        w.write::<u16>(gid_mapper.num_gids() - 2);
    }

    Ok(())
}
