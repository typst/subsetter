use crate::cff::subset::sid::SidRemapper;
use crate::cff::TopDict;
use crate::stream::StringId;
use std::collections::HashMap;

pub(crate) fn update_top_dict(
    top_dict: &TopDict,
    sid_remapper: &mut SidRemapper,
) -> Option<TopDict> {
    Some(TopDict {
        version: top_dict.version.map(|s| sid_remapper.remap(s)),
        notice: top_dict.notice.map(|s| sid_remapper.remap(s)),
        copyright: top_dict.copyright.map(|s| sid_remapper.remap(s)),
        full_name: top_dict.full_name.map(|s| sid_remapper.remap(s)),
        family_name: top_dict.family_name.map(|s| sid_remapper.remap(s)),
        weight: top_dict.weight.map(|s| sid_remapper.remap(s)),

        //charset: Option<usize>,
        //encoding: Option<usize>,
        //char_strings: Option<usize>,
        // private: Option<(usize, usize)>,
        postscript: top_dict.postscript.map(|s| sid_remapper.remap(s)),
        base_font_name: top_dict.base_font_name.map(|s| sid_remapper.remap(s)),
        ros: top_dict.ros.and_then(|(s1, s2, f)| {
            Some((sid_remapper.remap(s1), sid_remapper.remap(s2), f))
        }),
        // fd_array: Option<usize>,
        // fd_select: Option<usize>,
        font_name: top_dict.font_name.map(|s| sid_remapper.remap(s)),
        ..top_dict.clone()
    })
}
