use crate::Error::MalformedFont;
use crate::{glyf, Context, MaxpData};
use std::borrow::Cow;

pub fn subset(ctx: &mut Context) -> crate::Result<()> {
    let mut maxp_data = MaxpData::default();

    let result = glyf::subset_with(ctx, |old_gid, ctx| {
        let data = match ctx.interjector.glyph_data(&mut maxp_data) {
            Some(mut c) => Cow::Owned(c(old_gid).ok_or(MalformedFont)?),
            // CFF2 fonts are only
            None => return Err(MalformedFont),
        };

        Ok(data)
    });

    ctx.custom_maxp_data = Some(maxp_data);
    result
}
