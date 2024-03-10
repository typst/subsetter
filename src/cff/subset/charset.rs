use crate::cff::charset::Charset;
use crate::cff::subset::sid::SidRemapper;
use crate::stream::{StringId, Writer};
use crate::Context;

pub(crate) fn subset_charset(
    charset: &Charset,
    ctx: &Context,
    sid_remapper: &SidRemapper,
) -> Option<Vec<u8>> {
    let mut w = Writer::new();

    // Format 0
    w.write::<u8>(0);

    for gid in 1..ctx.mapper.num_gids() {
        let old_gid = ctx.mapper.get_reverse(gid)?;
        let old_sid = charset.gid_to_sid(old_gid)?;
        let new_sid = if old_sid >= StringId(391) {
            sid_remapper.get(&old_sid).copied()?
        } else {
            old_sid
        };
        w.write::<StringId>(new_sid);
    }

    // TODO: What if empty?

    Some(w.finish())
}
