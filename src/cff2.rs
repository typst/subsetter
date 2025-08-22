use crate::{glyf, Context};

/// CFF2 fonts will currently be converted into TTF fonts.
pub fn subset(ctx: &mut Context) -> crate::Result<()> {
    glyf::subset(ctx)
}
