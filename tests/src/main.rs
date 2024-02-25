use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;
use subsetter::{subset, Profile};

mod ttf;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct TestContext<'a> {
    original_face: ttf_parser::Face<'a>,
    new_face: ttf_parser::Face<'a>,
    gid_map: HashMap<u16, u16>,
    gids: Vec<u16>,
}

fn run_ttf_test(font_path: &Path, gids: &str) -> Result<()> {
    let data = std::fs::read(font_path)?;
    let face = ttf_parser::Face::parse(&data, 0)?;
    let gids = parse_gids(gids);

    let (subset, gid_map) = subset(
        &data,
        0,
        Profile::pdf(gids.iter().copied().collect::<Vec<_>>().as_ref()),
    )?;
    let subsetted_face = ttf_parser::Face::parse(&subset, 0)?;

    let test_context = TestContext {
        original_face: face,
        new_face: subsetted_face,
        gid_map,
        gids,
    };

    check_cmap(&test_context);
    check_face_metrics(&test_context);

    Ok(())
}

fn parse_gids(gids: &str) -> Vec<u16> {
    if gids == "*" {
        return (0..u16::MAX).collect();
    }

    let split = gids.split(",").collect::<Vec<_>>();
    let mut gids = vec![];

    for el in &split {
        if el.contains("-") {
            let range = el.split("-").collect::<Vec<_>>();
            let first = range[0].parse::<u16>().unwrap();
            let second = range[1].parse::<u16>().unwrap();

            gids.extend(first..second);
        } else {
            gids.push(el.parse::<u16>().unwrap());
        }
    }

    gids
}

/// Check that for each character that was mapped to a gid that is in the subset,
/// a corresponding map also exists in the new face.
fn check_cmap(ctx: &TestContext) {
    let mut all_chars = vec![];

    ctx.original_face
        .tables()
        .cmap
        .unwrap()
        .subtables
        .into_iter()
        .for_each(|s| {
            s.codepoints(|c| all_chars.push(c));
        });

    let relevant_chars = all_chars
        .iter()
        .map(|c| char::from_u32(*c).unwrap())
        .filter_map(|c| match ctx.original_face.glyph_index(c) {
            Some(g) if ctx.gids.contains(&g.0) => Some((c, g)),
            _ => None,
        })
        .collect::<Vec<_>>();

    for (c, gid) in relevant_chars {
        let mapped_gid = ctx.gid_map.get(&gid.0).copied();
        let cur_gid = ctx.new_face.glyph_index(c).map(|g| g.0);
        assert_eq!((c, mapped_gid), (c, cur_gid));
    }
}

fn check_face_metrics(ctx: &TestContext) {
    assert_eq!(ctx.original_face.width(), ctx.new_face.width());
    assert_eq!(ctx.original_face.height(), ctx.new_face.height());
    assert_eq!(ctx.original_face.ascender(), ctx.new_face.ascender());
    assert_eq!(ctx.original_face.descender(), ctx.new_face.descender());
    assert_eq!(ctx.original_face.style(), ctx.new_face.style());
    assert_eq!(ctx.original_face.capital_height(), ctx.new_face.capital_height());
    assert_eq!(ctx.original_face.is_italic(), ctx.new_face.is_italic());
    assert_eq!(ctx.original_face.is_bold(), ctx.new_face.is_bold());
    assert_eq!(ctx.original_face.is_monospaced(), ctx.new_face.is_monospaced());
    assert_eq!(ctx.original_face.is_oblique(), ctx.new_face.is_oblique());
    assert_eq!(ctx.original_face.is_regular(), ctx.new_face.is_regular());
}
