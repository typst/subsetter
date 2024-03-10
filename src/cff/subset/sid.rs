use crate::cff::Table;
use crate::stream::StringId;
use std::collections::{HashMap, HashSet};

pub type SidRemapper = HashMap<StringId, StringId>;

// Collects all custom string ids that appear as part of the font.
fn collect_custom_sids(
    table: &Table,
    requested_glyphs: &HashSet<u16>,
) -> HashSet<StringId> {
    let mut sid_set = HashSet::new();

    let mut collect = |sid: StringId| {
        if sid.0 > 390 {
            sid_set.insert(sid);
        }
    };

    // TOP DICT
    table.top_dict.version.map(&mut collect);
    table.top_dict.notice.map(&mut collect);
    table.top_dict.copyright.map(&mut collect);
    table.top_dict.full_name.map(&mut collect);
    table.top_dict.family_name.map(&mut collect);
    table.top_dict.weight.map(&mut collect);
    table.top_dict.postscript.map(&mut collect);
    table.top_dict.base_font_name.map(&mut collect);

    table.top_dict.ros.map(|ros| {
        collect(ros.0);
        collect(ros.1);
    });
    table.top_dict.font_name.map(&mut collect);

    // CHARSET
    for gid in requested_glyphs {
        if let Some(sid) = table.charset.gid_to_sid(*gid) {
            collect(sid);
        }
    }

    sid_set
}

pub(crate) fn get_sid_remapper(
    table: &Table,
    requested_glyphs: &HashSet<u16>,
) -> Option<SidRemapper> {
    let sids = collect_custom_sids(table, requested_glyphs);

    let res = sids
        .into_iter()
        .enumerate()
        .map(|(counter, sid)| (sid, StringId(u16::try_from(counter + 391).unwrap())))
        .collect::<HashMap<_, _>>();

    Some(res)
}
