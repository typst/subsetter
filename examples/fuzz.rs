use std::fs;
use rand_distr::Distribution;

use rand::{Rng, SeedableRng, thread_rng};
use rand::prelude::IteratorRandom;
use skrifa::outline::{DrawSettings, HintingInstance, HintingMode, OutlinePen};
use ttf_parser::GlyphId;
use subsetter::{GlyphRemapper, subset};

fn main() {
    let mut rng = thread_rng();
    let paths = fs::read_dir("/Users/lstampfl/Programming/Playground/python-playground/fonts").unwrap();

    for path in paths {
        let path = path.unwrap();
        let data = fs::read(path.path()).unwrap();
        let old_face = ttf_parser::Face::parse(&data, 0).unwrap();
        let num_glyphs = old_face.number_of_glyphs();
        let possible_gids = (0..num_glyphs).collect::<Vec<_>>();

        for _ in 0..500 {
            let sample = possible_gids.clone().into_iter().choose_multiple(&mut rng, 15);
            let (subset, remapper) = subset(&data, 0, &sample).unwrap();
            let new_face = ttf_parser::Face::parse(&subset, 0).unwrap();
            glyph_outlines_ttf_parser(&old_face, &new_face, &remapper, &sample).unwrap();
        }
    }
}

// fn face_metrics(font_file: &str, gids: &str) {
//     let ctx = get_test_context(font_file, gids).unwrap();
//     let old_face = ttf_parser::Face::parse(&ctx.font, 0).unwrap();
//     let new_face = ttf_parser::Face::parse(&ctx.subset, 0).unwrap();
//
//     assert_eq!(old_face.width(), new_face.width(), "face width didn't match");
//     assert_eq!(old_face.height(), new_face.height(), "face height didn't match");
//     assert_eq!(old_face.ascender(), new_face.ascender(), "face ascender didn't match");
//     assert_eq!(old_face.descender(), new_face.descender(), "face descender didn't match");
//     assert_eq!(old_face.style(), new_face.style(), "face style didn't match");
//     assert_eq!(
//         old_face.capital_height(),
//         new_face.capital_height(),
//         "face capital didn't match"
//     );
//     assert_eq!(old_face.is_italic(), new_face.is_italic(), "face italic didn't match");
//     assert_eq!(old_face.is_bold(), new_face.is_bold(), "face bold didn't match");
//     assert_eq!(
//         old_face.is_monospaced(),
//         new_face.is_monospaced(),
//         "face monospaced didn't match"
//     );
//     assert_eq!(old_face.is_oblique(), new_face.is_oblique(), "face oblique didn't match");
//     assert_eq!(old_face.is_regular(), new_face.is_regular(), "face regular didn't match");
//     assert_eq!(old_face.x_height(), new_face.x_height(), "face x_height didn't match");
//     assert_eq!(
//         old_face.strikeout_metrics(),
//         new_face.strikeout_metrics(),
//         "face strikeout metrics didn't match"
//     );
//     assert_eq!(
//         old_face.subscript_metrics(),
//         new_face.subscript_metrics(),
//         "face subscript metrics didn't match"
//     );
//     assert_eq!(
//         old_face.superscript_metrics(),
//         new_face.superscript_metrics(),
//         "face superscript matrics didn't match"
//     );
//     assert_eq!(
//         old_face.typographic_ascender(),
//         new_face.typographic_ascender(),
//         "face typographic ascender didn't match"
//     );
//     assert_eq!(
//         old_face.typographic_descender(),
//         new_face.typographic_descender(),
//         "face typographic descender didn't match"
//     );
//     assert_eq!(
//         old_face.typographic_line_gap(),
//         new_face.typographic_line_gap(),
//         "face typographic line gap didn't match"
//     );
//     assert_eq!(
//         old_face.units_per_em(),
//         new_face.units_per_em(),
//         "face units per em didn't match"
//     );
// }

// fn glyph_metrics(font_file: &str, gids: &str) {
//     let ctx = get_test_context(font_file, gids).unwrap();
//     let old_face = ttf_parser::Face::parse(&ctx.font, 0).unwrap();
//     let new_face = ttf_parser::Face::parse(&ctx.subset, 0).unwrap();
//
//     for glyph in ctx
//         .gids
//         .iter()
//         .copied()
//         .filter(|g| ctx.gids.contains(g) && *g < old_face.number_of_glyphs())
//     {
//         let mapped = ctx.mapper.get(glyph).unwrap();
//
//         assert_eq!(
//             old_face.glyph_bounding_box(GlyphId(glyph)),
//             new_face.glyph_bounding_box(GlyphId(mapped)),
//             "{:?}",
//             format!("metric glyph bounding box didn't match for glyph {}.", glyph)
//         );
//
//         assert_eq!(
//             old_face.glyph_hor_side_bearing(GlyphId(glyph)),
//             new_face.glyph_hor_side_bearing(GlyphId(mapped)),
//             "{:?}",
//             format!(
//                 "metric glyph horizontal side bearing didn't match for glyph {}.",
//                 glyph
//             )
//         );
//
//         assert_eq!(
//             old_face.glyph_hor_advance(GlyphId(glyph)),
//             new_face.glyph_hor_advance(GlyphId(mapped)),
//             "{:?}",
//             format!("metric glyph horizontal advance didn't match for glyph {}.", glyph)
//         );
//
//         assert_eq!(
//             old_face.glyph_name(GlyphId(glyph)),
//             new_face.glyph_name(GlyphId(mapped)),
//             "{:?}",
//             format!("metric glyph name didn't match for glyph {}.", glyph)
//         );
//
//         if let Some(old_cff) = old_face.tables().cff {
//             let new_cff = new_face.tables().cff.unwrap();
//
//             assert_eq!(
//                 old_cff.glyph_cid(GlyphId(glyph)),
//                 new_cff.glyph_cid(GlyphId(mapped)),
//                 "{:?}",
//                 format!("metric glyph cid didn't match for glyph {}.", glyph)
//             );
//         }
//     }
// }

// fn glyph_outlines_skrifa(font_file: &str, gids: &str) {
//     let ctx = get_test_context(font_file, gids).unwrap();
//     let old_face = skrifa::FontRef::new(&ctx.font).unwrap();
//     let new_face = skrifa::FontRef::new(&ctx.subset).unwrap();
//     let hinting_instance_old = HintingInstance::new(
//         &old_face.outline_glyphs(),
//         Size::new(150.0),
//         LocationRef::default(),
//         HintingMode::Smooth { lcd_subpixel: None, preserve_linear_metrics: false },
//     )
//         .unwrap();
//
//     let hinting_instance_new = HintingInstance::new(
//         &new_face.outline_glyphs(),
//         Size::new(150.0),
//         LocationRef::default(),
//         HintingMode::Smooth { lcd_subpixel: None, preserve_linear_metrics: false },
//     )
//         .unwrap();
//
//     let mut sink1 = Sink(vec![]);
//     let mut sink2 = Sink(vec![]);
//
//     let num_glyphs = old_face.maxp().unwrap().num_glyphs();
//
//     for glyph in (0..num_glyphs).filter(|g| ctx.gids.contains(g)) {
//         let new_glyph = ctx.mapper.get(glyph).unwrap();
//         let settings = DrawSettings::hinted(&hinting_instance_old, true);
//
//         if let Some(glyph1) = old_face.outline_glyphs().get(skrifa::GlyphId::new(glyph)) {
//             glyph1.draw(settings, &mut sink1).unwrap();
//
//             let settings = DrawSettings::hinted(&hinting_instance_new, true);
//             let glyph2 = new_face
//                 .outline_glyphs()
//                 .get(skrifa::GlyphId::new(new_glyph))
//                 .expect(&format!("failed to find glyph {} in new face", glyph));
//             glyph2.draw(settings, &mut sink2).unwrap();
//         }
//     }
// }

fn glyph_outlines_ttf_parser(
    old_face: &ttf_parser::Face,
    new_face: &ttf_parser::Face,
    mapper: &GlyphRemapper,
    gids: &[u16]
) -> Result<(), u16> {

    for glyph in gids {
        let new_glyph = mapper.get(*glyph).unwrap();
        let mut sink1 = Sink::default();
        let mut sink2 = Sink::default();

        if let Some(_) = old_face.outline_glyph(GlyphId(*glyph), &mut sink1) {
            new_face.outline_glyph(GlyphId(new_glyph), &mut sink2);
            if sink1 != sink2 {
                return Err(*glyph);
            }   else {
                return Ok(());
            }
        }   else {
            return Ok(())
        }
    }

    return Ok(())
}

// fn glyph_outlines_freetype(font_file: &str, gids: &str) {
//     let ctx = get_test_context(font_file, gids).unwrap();
//     let library = freetype::Library::init().unwrap();
//     let old_face = library.new_memory_face2(ctx.font, 0).unwrap();
//     let new_face = library.new_memory_face2(ctx.subset, 0).unwrap();
//     let num_glyphs = old_face.num_glyphs() as u16;
//
//     for glyph in (0..num_glyphs).filter(|g| ctx.gids.contains(g)) {
//         let new_glyph = ctx.mapper.get(glyph).unwrap();
//
//         old_face.load_glyph(glyph as u32, LoadFlag::DEFAULT).unwrap();
//         let old_outline = old_face.glyph().outline().unwrap();
//
//         new_face.load_glyph(new_glyph as u32, LoadFlag::DEFAULT).unwrap();
//         let new_outline = new_face.glyph().outline().unwrap();
//
//         let sink1 = Sink::from_freetype(&old_outline);
//         let sink2 = Sink::from_freetype(&new_outline);
//
//         assert_eq!(sink1, sink2, "glyph {} drawn with freetype didn't match.", glyph);
//     }
// }

#[derive(Debug, Default, PartialEq)]
struct Sink(Vec<Inst>);

impl Sink {
    fn from_freetype(outline: &freetype::Outline) -> Self {
        let mut insts = vec![];

        for contour in outline.contours_iter() {
            for curve in contour {
                insts.push(Inst::from_freetype_curve(curve))
            }
        }

        Self(insts)
    }
}

#[derive(Debug, PartialEq)]
enum Inst {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    QuadTo(f32, f32, f32, f32),
    CurveTo(f32, f32, f32, f32, f32, f32),
    Close,
}

impl Inst {
    fn from_freetype_curve(curve: freetype::outline::Curve) -> Self {
        match curve {
            freetype::outline::Curve::Line(pt) => Inst::LineTo(pt.x as f32, pt.y as f32),
            freetype::outline::Curve::Bezier2(pt1, pt2) => {
                Inst::QuadTo(pt1.x as f32, pt1.y as f32, pt2.x as f32, pt2.y as f32)
            }
            freetype::outline::Curve::Bezier3(pt1, pt2, pt3) => Inst::CurveTo(
                pt1.x as f32,
                pt1.y as f32,
                pt2.x as f32,
                pt2.y as f32,
                pt3.x as f32,
                pt3.y as f32,
            ),
        }
    }
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