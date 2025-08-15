use crate::interjector::Interjector;
use crate::Error::MalformedFont;
use crate::{glyf, Context, MaxpData};
use std::borrow::Cow;

/// CFF2 fonts will currently be converted into TTF fonts.
pub fn subset(ctx: &mut Context) -> crate::Result<()> {
    let mut maxp_data = MaxpData::default();

    let result = glyf::subset_with(ctx, |old_gid, ctx| {
        let data = match &ctx.interjector {
            // We reject CFF2 fonts earlier if `variable-fonts` feature is not enabled.
            Interjector::Dummy => unreachable!(),
            #[cfg(feature = "variable-fonts")]
            Interjector::Skrifa(s) => {
                Cow::Owned(s.glyph_data(&mut maxp_data, old_gid).ok_or(MalformedFont)?)
            }
        };

        Ok(data)
    });

    ctx.custom_maxp_data = Some(maxp_data);
    result
}
