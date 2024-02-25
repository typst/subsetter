use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use subsetter::{subset, Profile};
use ttf_parser::GlyphId;

mod ttf;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct TestContext<'a> {
    original_face: ttf_parser::Face<'a>,
    new_face: ttf_parser::Face<'a>,
    gid_map: HashMap<u16, u16>,
    gids: Vec<u16>,
}

fn run_ttf_test(font_file: &str, gids: &str) -> Result<()> {
    let mut font_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    font_path.push("fonts");
    font_path.push(font_file);

    let data = std::fs::read(font_path)?;
    let face = ttf_parser::Face::parse(&data, 0)?;
    let gids: Vec<_> = parse_gids(gids)
        .into_iter()
        .filter(|g| *g < face.number_of_glyphs())
        .collect();

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
    check_glyph_metrics(&test_context);

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
    assert_eq!(ctx.original_face.x_height(), ctx.new_face.x_height());
    assert_eq!(ctx.original_face.strikeout_metrics(), ctx.new_face.strikeout_metrics());
    assert_eq!(ctx.original_face.subscript_metrics(), ctx.new_face.subscript_metrics());
    assert_eq!(
        ctx.original_face.superscript_metrics(),
        ctx.new_face.superscript_metrics()
    );
    assert_eq!(
        ctx.original_face.typographic_ascender(),
        ctx.new_face.typographic_ascender()
    );
    assert_eq!(
        ctx.original_face.typographic_descender(),
        ctx.new_face.typographic_descender()
    );
    assert_eq!(
        ctx.original_face.typographic_line_gap(),
        ctx.new_face.typographic_line_gap()
    );
    assert_eq!(ctx.original_face.vertical_ascender(), ctx.new_face.vertical_ascender());
    assert_eq!(ctx.original_face.vertical_descender(), ctx.new_face.vertical_descender());
    assert_eq!(ctx.original_face.vertical_height(), ctx.new_face.vertical_height());
    assert_eq!(ctx.original_face.vertical_line_gap(), ctx.new_face.vertical_line_gap());
    assert_eq!(ctx.original_face.units_per_em(), ctx.new_face.units_per_em());
}

fn check_glyph_metrics(ctx: &TestContext) {
    for glyph in ctx
        .gids
        .iter()
        .copied()
        .filter(|g| ctx.gids.contains(g) && *g < ctx.original_face.number_of_glyphs())
    {
        let mapped = *ctx.gid_map.get(&glyph).unwrap();

        assert_eq!(
            ctx.original_face.glyph_bounding_box(GlyphId(glyph)),
            ctx.new_face.glyph_bounding_box(GlyphId(mapped)),
        );

        assert_eq!(
            ctx.original_face.glyph_hor_advance(GlyphId(glyph)),
            ctx.new_face.glyph_hor_advance(GlyphId(mapped)),
        );

        assert_eq!(
            ctx.original_face.glyph_ver_advance(GlyphId(glyph)),
            ctx.new_face.glyph_ver_advance(GlyphId(mapped)),
        );

        assert_eq!(
            ctx.original_face.glyph_y_origin(GlyphId(glyph)),
            ctx.new_face.glyph_y_origin(GlyphId(mapped)),
        );

        assert_eq!(
            ctx.original_face.glyph_hor_side_bearing(GlyphId(glyph)),
            ctx.new_face.glyph_hor_side_bearing(GlyphId(mapped)),
        );

        assert_eq!(
            ctx.original_face.glyph_ver_side_bearing(GlyphId(glyph)),
            ctx.new_face.glyph_ver_side_bearing(GlyphId(mapped)),
        );

        assert_eq!(
            ctx.original_face.glyph_hor_advance(GlyphId(glyph)),
            ctx.new_face.glyph_hor_advance(GlyphId(mapped)),
        );

        assert_eq!(
            ctx.original_face.glyph_ver_advance(GlyphId(glyph)),
            ctx.new_face.glyph_ver_advance(GlyphId(mapped)),
        );

        assert_eq!(
            ctx.original_face.glyph_name(GlyphId(glyph)),
            ctx.new_face.glyph_name(GlyphId(mapped)),
        );
    }
}
