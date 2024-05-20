use sha2::{Digest, Sha256};
use skrifa::outline::{DrawSettings, HintingInstance, HintingMode, OutlinePen};
use skrifa::prelude::{LocationRef, Size};
use skrifa::raw::TableProvider;
use skrifa::MetadataProvider;
use std::error::Error;
use std::fs;
use std::panic::Location;
use std::path::PathBuf;
use subsetter::{subset, GidMapper};
use ttf_parser::GlyphId;

#[rustfmt::skip]
mod ttf;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const SAVE_SUBSETS: bool = false;

struct TestContext {
    font: Vec<u8>,
    subset: Vec<u8>,
    mapper: GidMapper,
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

    let face = ttf_parser::Face::parse(&data, 0).unwrap();

    let gids: Vec<_> = parse_gids(gids, face.number_of_glyphs());

    let (subset, mapper) = subset(&data, 0, &gids)?;

    if SAVE_SUBSETS {
        save_font(&subset);
    }

    Ok(TestContext { font: data, subset, mapper, gids })
}

fn parse_gids(gids: &str, max: u16) -> Vec<u16> {
    if gids == "*" {
        return (0..max).collect();
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

fn face_metrics(font_file: &str, gids: &str) {
    let ctx = get_test_context(font_file, gids).unwrap();
    let old_face = ttf_parser::Face::parse(&ctx.font, 0).unwrap();
    let new_face = ttf_parser::Face::parse(&ctx.subset, 0).unwrap();

    assert_eq!(old_face.width(), new_face.width(), "face width didn't match");
    assert_eq!(old_face.height(), new_face.height(), "face height didn't match");
    assert_eq!(old_face.ascender(), new_face.ascender(), "face ascender didn't match");
    assert_eq!(old_face.descender(), new_face.descender(), "face descender didn't match");
    assert_eq!(old_face.style(), new_face.style(), "face style didn't match");
    assert_eq!(
        old_face.capital_height(),
        new_face.capital_height(),
        "face capital didn't match"
    );
    assert_eq!(old_face.is_italic(), new_face.is_italic(), "face italic didn't match");
    assert_eq!(old_face.is_bold(), new_face.is_bold(), "face bold didn't match");
    assert_eq!(
        old_face.is_monospaced(),
        new_face.is_monospaced(),
        "face monospaced didn't match"
    );
    assert_eq!(old_face.is_oblique(), new_face.is_oblique(), "face oblique didn't match");
    assert_eq!(old_face.is_regular(), new_face.is_regular(), "face regular didn't match");
    assert_eq!(old_face.x_height(), new_face.x_height(), "face x_height didn't match");
    assert_eq!(
        old_face.strikeout_metrics(),
        new_face.strikeout_metrics(),
        "face strikeout metrics didn't match"
    );
    assert_eq!(
        old_face.subscript_metrics(),
        new_face.subscript_metrics(),
        "face subscript metrics didn't match"
    );
    assert_eq!(
        old_face.superscript_metrics(),
        new_face.superscript_metrics(),
        "face superscript matrics didn't match"
    );
    assert_eq!(
        old_face.typographic_ascender(),
        new_face.typographic_ascender(),
        "face typographic ascender didn't match"
    );
    assert_eq!(
        old_face.typographic_descender(),
        new_face.typographic_descender(),
        "face typographic descender didn't match"
    );
    assert_eq!(
        old_face.typographic_line_gap(),
        new_face.typographic_line_gap(),
        "face typographic line gap didn't match"
    );
    assert_eq!(
        old_face.units_per_em(),
        new_face.units_per_em(),
        "face units per em didn't match"
    );
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
            "{:?}",
            format!("metric glyph bounding box didn't match for glyph {}.", glyph)
        );

        assert_eq!(
            old_face.glyph_hor_side_bearing(GlyphId(glyph)),
            new_face.glyph_hor_side_bearing(GlyphId(mapped)),
            "{:?}",
            format!(
                "metric glyph horizontal side bearing didn't match for glyph {}.",
                glyph
            )
        );

        assert_eq!(
            old_face.glyph_hor_advance(GlyphId(glyph)),
            new_face.glyph_hor_advance(GlyphId(mapped)),
            "{:?}",
            format!("metric glyph horizontal advance didn't match for glyph {}.", glyph)
        );

        assert_eq!(
            old_face.glyph_name(GlyphId(glyph)),
            new_face.glyph_name(GlyphId(mapped)),
            "{:?}",
            format!("metric glyph name didn't match for glyph {}.", glyph)
        );

        if let Some(old_cff) = old_face.tables().cff {
            let new_cff = new_face.tables().cff.unwrap();

            assert_eq!(
                old_cff.glyph_cid(GlyphId(glyph)),
                new_cff.glyph_cid(GlyphId(mapped)),
                "{:?}",
                format!("metric glyph cid didn't match for glyph {}.", glyph)
            );
        }
    }
}

pub fn glyph_outlines_skrifa(font_file: &str, gids: &str) {
    let ctx = get_test_context(font_file, gids).unwrap();
    let old_face = skrifa::FontRef::new(&ctx.font).unwrap();
    let new_face = skrifa::FontRef::new(&ctx.subset).unwrap();
    let hinting_instance = HintingInstance::new(
        &old_face.outline_glyphs(),
        Size::new(150.0),
        LocationRef::default(),
        HintingMode::Smooth { lcd_subpixel: None, preserve_linear_metrics: false },
    )
    .unwrap();

    let mut sink1 = Sink(vec![]);
    let mut sink2 = Sink(vec![]);

    let num_glyphs = old_face.maxp().unwrap().num_glyphs();

    for glyph in (0..num_glyphs).filter(|g| ctx.gids.contains(g)) {
        let new_glyph = ctx.mapper.get(glyph).unwrap();
        let settings = DrawSettings::hinted(&hinting_instance, false);
        let glyph1 = old_face
            .outline_glyphs()
            .get(skrifa::GlyphId::new(glyph))
            .expect(&format!("failed to find glyph {} in old face", glyph));
        glyph1.draw(settings, &mut sink1).unwrap();

        let settings = DrawSettings::hinted(&hinting_instance, false);
        let glyph2 = new_face
            .outline_glyphs()
            .get(skrifa::GlyphId::new(new_glyph))
            .expect(&format!("failed to find glyph {} in new face", glyph));
        glyph2.draw(settings, &mut sink2).unwrap();

        assert_eq!(sink1, sink2, "glyph {} drawn didn't match.", glyph);
    }
}

pub fn glyph_outlines_ttf_parser(font_file: &str, gids: &str) {
    let ctx = get_test_context(font_file, gids).unwrap();
    let old_face = ttf_parser::Face::parse(&ctx.font, 0).unwrap();
    let new_face = ttf_parser::Face::parse(&ctx.subset, 0).unwrap();

    for glyph in (0..old_face.number_of_glyphs()).filter(|g| ctx.gids.contains(g)) {
        let new_glyph = ctx.mapper.get(glyph).unwrap();
        let mut sink1 = Sink::default();
        let mut sink2 = Sink::default();
        old_face.outline_glyph(GlyphId(glyph), &mut sink1);
        new_face.outline_glyph(GlyphId(new_glyph), &mut sink2);
        assert_eq!(sink1, sink2, "glyph {} drawn with ttf-parser didn't match.", glyph);
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

impl OutlinePen for Sink {
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
