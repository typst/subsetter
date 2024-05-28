use crate::Inst::{CurveTo, LineTo, QuadTo};
use freetype::face::LoadFlag;
use skrifa::outline::{DrawSettings, HintingInstance, HintingMode, OutlinePen};
use skrifa::prelude::{LocationRef, Size};
use skrifa::raw::TableProvider;
use skrifa::MetadataProvider;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;
use subsetter::{subset, GlyphRemapper};
use ttf_parser::GlyphId;

#[rustfmt::skip]
mod subsets;

#[rustfmt::skip]
mod font_tools;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const FONT_TOOLS_REF: bool = false;
const OVERWRITE_REFS: bool = true;

struct TestContext {
    font: Vec<u8>,
    subset: Vec<u8>,
    mapper: GlyphRemapper,
    gids: Vec<u16>,
}

fn test_font_tools(font_file: &str, gids: &str, num: u16) {
    let mut font_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    font_path.push("tests/ttx");
    let _ = std::fs::create_dir_all(&font_path);

    let name = Path::new(font_file);
    let stem = name.file_stem().unwrap().to_string_lossy().to_string();
    let ttx_name = format!("{}_{}.ttx", stem, num);
    let ttx_ref_name = format!("{}_{}_ref.ttx", stem, num);
    let otf_name = format!("{}_{}.otf", stem, num);
    let otf_ref_name = format!("{}_{}_ref.otf", stem, num);
    let otf_path: PathBuf = [font_path.to_string_lossy().to_string(), otf_name.clone()]
        .iter()
        .collect();
    let otf_ref_path: PathBuf =
        [font_path.to_string_lossy().to_string(), otf_ref_name.clone()]
            .iter()
            .collect();
    let ttx_path: PathBuf = [font_path.to_string_lossy().to_string(), ttx_name.clone()]
        .iter()
        .collect();
    let ttx_ref_path: PathBuf =
        [font_path.to_string_lossy().to_string(), ttx_ref_name.clone()]
            .iter()
            .collect();

    let data = read_file(font_file);
    let face = ttf_parser::Face::parse(&data, 0).unwrap();
    let gids_vec: Vec<_> = parse_gids(gids, face.number_of_glyphs());
    let remapper = GlyphRemapper::new_from_glyphs(gids_vec.as_slice());
    let subset = subset(&data, 0, &remapper).unwrap();

    std::fs::write(otf_path.clone(), subset).unwrap();

    if FONT_TOOLS_REF {
        let font_path = get_font_path(font_file);
        Command::new("fonttools")
            .args([
                "subset",
                font_path.to_str().unwrap(),
                "--drop-tables=GSUB,GPOS,GDEF,FFTM,vhea,vmtx,DSIG,VORG,hdmx,cmap,MATH",
                &format!("--gids={}", gids),
                "--glyph-names",
                "--desubroutinize",
                "--notdef-outline",
                "--no-prune-unicode-ranges",
                "--no-prune-codepage-ranges",
                &format!("--output-file={}", otf_ref_path.to_str().unwrap()),
            ])
            .output()
            .unwrap();

        Command::new("fonttools")
            .args([
                "ttx",
                "-f",
                "-o",
                ttx_ref_path.clone().to_str().unwrap(),
                otf_ref_path.clone().to_str().unwrap(),
            ])
            .output()
            .unwrap();
    }

    if !ttx_path.exists() || OVERWRITE_REFS {
        Command::new("fonttools")
            .args([
                "ttx",
                "-f",
                "-o",
                ttx_path.clone().to_str().unwrap(),
                otf_path.clone().to_str().unwrap(),
            ])
            .output()
            .unwrap();
    } else {
        let output = Command::new("fonttools")
            .args(["ttx", "-f", "-o", "-", otf_path.clone().to_str().unwrap()])
            .output()
            .unwrap()
            .stdout;

        let reference = std::fs::read(ttx_path).unwrap();
        assert_eq!(
            reference.len(),
            output.len(),
            "fonttools output didn't match in length."
        );
        assert!(
            reference.iter().zip(output.iter()).all(|(a, b)| a == b),
            "fonttools output didn't match."
        );
    }
}

fn get_font_path(font_file: &str) -> PathBuf {
    let mut font_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    font_path.push("fonts");
    font_path.push(font_file);
    font_path
}

fn read_file(font_file: &str) -> Vec<u8> {
    let font_path = get_font_path(font_file);
    std::fs::read(font_path).unwrap()
}

fn get_test_context(font_file: &str, gids: &str) -> Result<TestContext> {
    let data = read_file(font_file);
    let face = ttf_parser::Face::parse(&data, 0).unwrap();
    let gids: Vec<_> = parse_gids(gids, face.number_of_glyphs());
    let glyph_remapepr = GlyphRemapper::new_from_glyphs(gids.as_slice());
    let subset = subset(&data, 0, &glyph_remapepr)?;

    Ok(TestContext { font: data, subset, mapper: glyph_remapepr, gids })
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

            gids.extend(first..=second);
        } else {
            gids.push(el.parse::<u16>().unwrap());
        }
    }

    gids
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

        // Assert that each glyph has an identity CID-to-GID mapping.
        if let Some(cff) = new_face.tables().cff {
            assert_eq!(cff.glyph_cid(GlyphId(mapped)), Some(mapped))
        }
    }
}

fn glyph_outlines_skrifa(font_file: &str, gids: &str) {
    let ctx = get_test_context(font_file, gids).unwrap();
    let old_face = skrifa::FontRef::new(&ctx.font).unwrap();
    let new_face = skrifa::FontRef::new(&ctx.subset).unwrap();
    let hinting_instance_old = HintingInstance::new(
        &old_face.outline_glyphs(),
        Size::new(150.0),
        LocationRef::default(),
        HintingMode::Smooth { lcd_subpixel: None, preserve_linear_metrics: false },
    )
    .unwrap();

    let hinting_instance_new = HintingInstance::new(
        &new_face.outline_glyphs(),
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
        let settings = DrawSettings::hinted(&hinting_instance_old, true);

        if let Some(glyph1) = old_face.outline_glyphs().get(skrifa::GlyphId::new(glyph)) {
            glyph1.draw(settings, &mut sink1).unwrap();

            let settings = DrawSettings::hinted(&hinting_instance_new, true);
            let glyph2 = new_face
                .outline_glyphs()
                .get(skrifa::GlyphId::new(new_glyph))
                .expect(&format!("failed to find glyph {} in new face", glyph));
            glyph2.draw(settings, &mut sink2).unwrap();
        }
    }
}

fn glyph_outlines_ttf_parser(font_file: &str, gids: &str) {
    let ctx = get_test_context(font_file, gids).unwrap();
    let old_face = ttf_parser::Face::parse(&ctx.font, 0).unwrap();
    let new_face = ttf_parser::Face::parse(&ctx.subset, 0).unwrap();

    for glyph in (0..old_face.number_of_glyphs()).filter(|g| ctx.gids.contains(g)) {
        let new_glyph = ctx.mapper.get(glyph).unwrap();
        let mut sink1 = Sink::default();
        let mut sink2 = Sink::default();

        if let Some(_) = old_face.outline_glyph(GlyphId(glyph), &mut sink1) {
            new_face.outline_glyph(GlyphId(new_glyph), &mut sink2);
            assert_eq!(
                sink1, sink2,
                "glyph {} drawn with ttf-parser didn't match.",
                glyph
            );
        }
    }
}

fn glyph_outlines_freetype(font_file: &str, gids: &str) {
    let ctx = get_test_context(font_file, gids).unwrap();
    let library = freetype::Library::init().unwrap();
    let old_face = library.new_memory_face2(ctx.font, 0).unwrap();
    let new_face = library.new_memory_face2(ctx.subset, 0).unwrap();
    let num_glyphs = old_face.num_glyphs() as u16;

    for glyph in (0..num_glyphs).filter(|g| ctx.gids.contains(g)) {
        let new_glyph = ctx.mapper.get(glyph).unwrap();

        old_face.load_glyph(glyph as u32, LoadFlag::DEFAULT).unwrap();
        let old_outline = old_face.glyph().outline().unwrap();

        new_face.load_glyph(new_glyph as u32, LoadFlag::DEFAULT).unwrap();
        let new_outline = new_face.glyph().outline().unwrap();

        let sink1 = Sink::from_freetype(&old_outline);
        let sink2 = Sink::from_freetype(&new_outline);

        assert_eq!(sink1, sink2, "glyph {} drawn with freetype didn't match.", glyph);
    }
}

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
            freetype::outline::Curve::Line(pt) => LineTo(pt.x as f32, pt.y as f32),
            freetype::outline::Curve::Bezier2(pt1, pt2) => {
                QuadTo(pt1.x as f32, pt1.y as f32, pt2.x as f32, pt2.y as f32)
            }
            freetype::outline::Curve::Bezier3(pt1, pt2, pt3) => CurveTo(
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
