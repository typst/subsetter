use crate::cff::TopDict;
use crate::stream::StringId;
use std::collections::HashMap;

pub(crate) fn update_top_dict(
    top_dict: &TopDict,
    sid_remapper: HashMap<StringId, StringId>,
) -> Option<TopDict> {
    Some(TopDict {
        version: top_dict.version.and_then(|s| sid_remapper.get(&s).copied()),
        notice: top_dict.notice.and_then(|s| sid_remapper.get(&s).copied()),
        copyright: top_dict.copyright.and_then(|s| sid_remapper.get(&s).copied()),
        full_name: top_dict.full_name.and_then(|s| sid_remapper.get(&s).copied()),
        family_name: top_dict.family_name.and_then(|s| sid_remapper.get(&s).copied()),
        weight: top_dict.weight.and_then(|s| sid_remapper.get(&s).copied()),

        //charset: Option<usize>,
        //encoding: Option<usize>,
        //char_strings: Option<usize>,
        // private: Option<(usize, usize)>,
        postscript: top_dict.postscript.and_then(|s| sid_remapper.get(&s).copied()),
        base_font_name: top_dict
            .base_font_name
            .and_then(|v| sid_remapper.get(&v).copied()),
        ros: top_dict.ros.and_then(|(s1, s2, f)| {
            Some((sid_remapper.get(&s1).copied()?, sid_remapper.get(&s2).copied()?, f))
        }),
        // fd_array: Option<usize>,
        // fd_select: Option<usize>,
        font_name: top_dict.font_name.and_then(|s| sid_remapper.get(&s).copied()),
        ..top_dict.clone()
    })
}
