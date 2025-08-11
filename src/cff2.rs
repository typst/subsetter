use std::borrow::Cow;
use crate::{glyf, Context};
use crate::Error::MalformedFont;

pub fn subset(ctx: &mut Context) -> crate::Result<()> {
    glyf::subset_with(ctx, |old_gid, ctx| {
        let data = match ctx.interjector.glyph_data() {
            Some(mut c) => Cow::Owned(c(old_gid).ok_or(MalformedFont)?),
            // CFF2 fonts are only 
            None => return Err(MalformedFont),
        };

        Ok(data)
    })
}