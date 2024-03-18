use sha2::{Digest, Sha256};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use subsetter::{subset, Mapper};
use ttf_parser::GlyphId;

#[rustfmt::skip]
mod ttf;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const SAVE_SUBSETS: bool = false;

struct TestContext {
    font: Vec<u8>,
    subset: Vec<u8>,
    mapper: Mapper,
    gids: Vec<u16>,
}

fn save_font(font: &[u8]) {
    let mut font_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    font_path.push("tests");
    font_path.push("subsets");
    let _ = std::fs::create_dir_all(&font_path);

    let mut hasher = Sha256::new();
    hasher.update(font);
    let hash = hex::encode(&hasher.finalize()[..]);

    font_path.push(hash);
    fs::write(&font_path, font).unwrap();
}

fn get_test_context(font_file: &str, gids: &str) -> Result<TestContext> {
    let mut font_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    font_path.push("fonts");
    font_path.push(font_file);

    let data = std::fs::read(font_path)?;
    let gids: Vec<_> = parse_gids(gids);

    let (subset, mapper) = subset(&data, 0, &gids)?;

    if SAVE_SUBSETS {
        save_font(&subset);
    }

    Ok(TestContext { font: data, subset, mapper, gids })
}

fn parse_gids(gids: &str) -> Vec<u16> {
    if gids == "*" {
        return (0..u16::MAX).collect();
    }

    let split = gids.split(",").filter(|s| !s.is_empty()).collect::<Vec<_>>();
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
// fn cmap(font_file: &str, gids: &str) {
//     let ctx = get_test_context(font_file, gids).unwrap();
//     let old_face = ttf_parser::Face::parse(&ctx.font, 0).unwrap();
//     let new_face = ttf_parser::Face::parse(&ctx.subset, 0).unwrap();
//     let mut all_chars = vec![];
//
//     old_face.tables().cmap.unwrap().subtables.into_iter().for_each(|s| {
//         s.codepoints(|c| all_chars.push(c));
//     });
//
//     let relevant_chars = all_chars
//         .iter()
//         .map(|c| char::from_u32(*c).unwrap())
//         .filter_map(|c| match old_face.glyph_index(c) {
//             Some(g) if ctx.gids.contains(&g.0) => Some((c, g)),
//             _ => None,
//         })
//         .collect::<Vec<_>>();
//
//     for (c, gid) in relevant_chars {
//         let mapped_gid = ctx.mapper.get(gid.0);
//         let cur_gid = new_face.glyph_index(c).map(|g| g.0);
//         assert_eq!((c, mapped_gid), (c, cur_gid));
//     }
// }

fn face_metrics(font_file: &str, gids: &str) {
    let ctx = get_test_context(font_file, gids).unwrap();
    let old_face = ttf_parser::Face::parse(&ctx.font, 0).unwrap();
    let new_face = ttf_parser::Face::parse(&ctx.subset, 0).unwrap();

    assert_eq!(old_face.width(), new_face.width());
    assert_eq!(old_face.height(), new_face.height());
    assert_eq!(old_face.ascender(), new_face.ascender());
    assert_eq!(old_face.descender(), new_face.descender());
    assert_eq!(old_face.style(), new_face.style());
    assert_eq!(old_face.capital_height(), new_face.capital_height());
    assert_eq!(old_face.is_italic(), new_face.is_italic());
    assert_eq!(old_face.is_bold(), new_face.is_bold());
    assert_eq!(old_face.is_monospaced(), new_face.is_monospaced());
    assert_eq!(old_face.is_oblique(), new_face.is_oblique());
    assert_eq!(old_face.is_regular(), new_face.is_regular());
    assert_eq!(old_face.x_height(), new_face.x_height());
    assert_eq!(old_face.strikeout_metrics(), new_face.strikeout_metrics());
    assert_eq!(old_face.subscript_metrics(), new_face.subscript_metrics());
    assert_eq!(old_face.superscript_metrics(), new_face.superscript_metrics());
    assert_eq!(old_face.typographic_ascender(), new_face.typographic_ascender());
    assert_eq!(old_face.typographic_descender(), new_face.typographic_descender());
    assert_eq!(old_face.typographic_line_gap(), new_face.typographic_line_gap());
    assert_eq!(old_face.units_per_em(), new_face.units_per_em());
}

fn glyph_metrics(font_file: &str, gids: &str) {
    let ctx = get_test_context(font_file, gids).unwrap();
    let old_face = ttf_parser::Face::parse(&ctx.font, 0).unwrap();
    let new_face = ttf_parser::Face::parse(&ctx.subset, 0).unwrap();

    for glyph in ctx
        .gids
        .iter()
        .copied()
        .filter(|g| ctx.gids.contains(g) && *g < old_face.number_of_glyphs())
    {
        let mapped = ctx.mapper.get(glyph).unwrap();

        assert_eq!(
            old_face.glyph_bounding_box(GlyphId(glyph)),
            new_face.glyph_bounding_box(GlyphId(mapped)),
        );

        assert_eq!(
            old_face.glyph_hor_advance(GlyphId(glyph)),
            new_face.glyph_hor_advance(GlyphId(mapped)),
        );

        assert_eq!(
            old_face.glyph_hor_side_bearing(GlyphId(glyph)),
            new_face.glyph_hor_side_bearing(GlyphId(mapped)),
        );

        assert_eq!(
            old_face.glyph_hor_advance(GlyphId(glyph)),
            new_face.glyph_hor_advance(GlyphId(mapped)),
        );

        assert_eq!(
            old_face.glyph_name(GlyphId(glyph)),
            new_face.glyph_name(GlyphId(mapped)),
        );
    }
}

pub fn glyph_outlines(font_file: &str, gids: &str) {
    let ctx = get_test_context(font_file, gids).unwrap();
    let old_face = ttf_parser::Face::parse(&ctx.font, 0).unwrap();
    let new_face = ttf_parser::Face::parse(&ctx.subset, 0).unwrap();

    for glyph in (0..old_face.number_of_glyphs()).filter(|g| ctx.gids.contains(g)) {
        let mut sink1 = Sink::default();
        let mut sink2 = Sink::default();
        old_face.outline_glyph(GlyphId(glyph), &mut sink1);
        new_face.outline_glyph(GlyphId(ctx.mapper.get(glyph).unwrap()), &mut sink2);
        assert_eq!(sink1, sink2);
    }
}

#[derive(Debug, Default, PartialEq)]
struct Sink(Vec<Inst>);

#[derive(Debug, PartialEq)]
enum Inst {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    QuadTo(f32, f32, f32, f32),
    CurveTo(f32, f32, f32, f32, f32, f32),
    Close,
}

impl ttf_parser::OutlineBuilder for Sink {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(Inst::MoveTo(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(Inst::LineTo(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.push(Inst::QuadTo(x1, y1, x, y));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.push(Inst::CurveTo(x1, y1, x2, y2, x, y));
    }

    fn close(&mut self) {
        self.0.push(Inst::Close);
    }
}
