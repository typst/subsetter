use kurbo::BezPath;
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::prelude::{LocationRef, Size};
use skrifa::raw::TableProvider;
use skrifa::MetadataProvider;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;
use subsetter::{subset, subset_with_variations, GlyphRemapper, Tag};
use ttf_parser::GlyphId;

#[rustfmt::skip]
mod subsets;

#[rustfmt::skip]
mod font_tools;
#[rustfmt::skip]
mod cff;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const FONT_TOOLS_REF: bool = false;
const OVERWRITE_REFS: bool = false;

struct TestContext {
    font: Vec<u8>,
    subset: Vec<u8>,
    mapper: GlyphRemapper,
    gids: Vec<u16>,
}

fn test_cff_dump(font_file: &str, gids: &str, num: u16) {
    let mut cff_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cff_path.push("tests/cff");
    let _ = std::fs::create_dir_all(&cff_path);

    let name = Path::new(font_file);
    let stem = name.file_stem().unwrap().to_string_lossy().to_string();
    let dump_name = format!("{}_{}.txt", stem, num);
    let dump_path: PathBuf = [cff_path.to_string_lossy().to_string(), dump_name.clone()]
        .iter()
        .collect();
    let otf_name = format!("{}_{}.otf", stem, num);
    let otf_path: PathBuf = [cff_path.to_string_lossy().to_string(), otf_name.clone()]
        .iter()
        .collect();

    let data = read_file(font_file);
    let face = ttf_parser::Face::parse(&data, 0).unwrap();
    let gids_vec: Vec<_> = parse_gids(gids, face.number_of_glyphs());
    let remapper = GlyphRemapper::new_from_glyphs(gids_vec.as_slice());
    let subset = subset(&data, 0, &remapper).unwrap();

    std::fs::write(otf_path.clone(), subset).unwrap();

    let cff_dump_util = env!("CFF_DUMP_BIN");

    let output = Command::new("java")
        .args([
            "-jar",
            cff_dump_util,
            "-otf",
            "-c",
            "-long",
            "-offsets",
            otf_path.clone().to_str().unwrap(),
        ])
        .output()
        .unwrap()
        .stdout;

    if !dump_path.exists() || OVERWRITE_REFS {
        std::fs::write(dump_path, &output).unwrap();
        panic!("reference file was created/overwritten.");
    } else {
        let reference = std::fs::read(dump_path).unwrap();

        assert!(
            reference.iter().zip(output.iter()).all(|(a, b)| a == b),
            "CFF dump output didn't match."
        );
    }
}

fn test_font_tools(font_file: &str, gids: &str, variations: &str, num: u16) {
    let mut ttx_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    ttx_path.push("tests/ttx");
    let _ = std::fs::create_dir_all(&ttx_path);

    let name = Path::new(font_file);
    let stem = name.file_stem().unwrap().to_string_lossy().to_string();
    let ttx_name = format!("{}_{}.ttx", stem, num);
    let ttx_ref_name = format!("{}_{}_ref.ttx", stem, num);
    let otf_name = format!("{}_{}.otf", stem, num);
    let otf_ref_name = format!("{}_{}_ref.otf", stem, num);
    let otf_path: PathBuf = [ttx_path.to_string_lossy().to_string(), otf_name.clone()]
        .iter()
        .collect();
    let otf_ref_path: PathBuf =
        [ttx_path.to_string_lossy().to_string(), otf_ref_name.clone()]
            .iter()
            .collect();
    let ttx_path: PathBuf = [ttx_path.to_string_lossy().to_string(), ttx_name.clone()]
        .iter()
        .collect();
    let ttx_ref_path: PathBuf =
        [ttx_path.to_string_lossy().to_string(), ttx_ref_name.clone()]
            .iter()
            .collect();

    let data = read_file(font_file);
    let face = ttf_parser::Face::parse(&data, 0).unwrap();
    let gids_vec: Vec<_> = parse_gids(gids, face.number_of_glyphs());
    let remapper = GlyphRemapper::new_from_glyphs(gids_vec.as_slice());
    let variations = parse_variations(variations);
    let subset = subset_with_variations(&data, 0, &variations, &remapper).unwrap();

    std::fs::write(otf_path.clone(), subset).unwrap();

    // Optionally create the subset via fonttools, so that we can compare it to our subset.
    if FONT_TOOLS_REF {
        let font_path = get_font_path(font_file);
        let mut input_path = font_path.to_str().unwrap();
        let output_path = otf_ref_path.to_str().unwrap();

        if !variations.is_empty() {
            let mut args = vec!["varLib.instancer".to_string(), input_path.to_string()];

            args.extend(variations.iter().map(|(name, value)| format!("{name}={value}")));
            args.extend(["-o".to_string(), output_path.to_string()]);

            Command::new("fonttools").args(args).output().unwrap();

            input_path = output_path;
        }

        Command::new("fonttools")
            .args([
                "subset",
                input_path,
                "--drop-tables=GSUB,GPOS,GDEF,FFTM,vhea,vmtx,DSIG,VORG,hdmx,cmap,MATH,HVAR,MVAR,STAT,avar,fvar,gvar",
                &format!("--gids={}", gids),
                "--glyph-names",
                "--desubroutinize",
                "--notdef-outline",
                "--no-prune-unicode-ranges",
                "--no-prune-codepage-ranges",
                &format!("--output-file={output_path}", ),
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
        panic!("reference file was created/overwritten.");
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
    let glyph_remapper = GlyphRemapper::new_from_glyphs(gids.as_slice());
    let subset = subset(&data, 0, &glyph_remapper)?;

    Ok(TestContext { font: data, subset, mapper: glyph_remapper, gids })
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

fn parse_variations(input: &str) -> Vec<(Tag, f32)> {
    input
        .split(',')
        .filter_map(|pair| {
            let parts: Vec<&str> = pair.split('=').collect();
            if parts.len() == 2 {
                Some((
                    Tag::from_str(parts[0].trim()).unwrap(),
                    parts[1].trim().parse().ok()?,
                ))
            } else {
                None
            }
        })
        .collect()
}

fn glyph_metrics(font_file: &str, gids: &str) {
    let ctx = get_test_context(font_file, gids).unwrap();
    let old_face = skrifa::FontRef::from_index(&ctx.font, 0).unwrap();
    let new_face = skrifa::FontRef::from_index(&ctx.subset, 0).unwrap();

    let old_metrics = old_face.glyph_metrics(Size::unscaled(), LocationRef::default());
    let new_metrics = new_face.glyph_metrics(Size::unscaled(), LocationRef::default());

    let old_face_num_glyphs = old_face.maxp().unwrap().num_glyphs();

    for glyph in ctx
        .gids
        .iter()
        .copied()
        .filter(|g| ctx.gids.contains(g) && *g < old_face_num_glyphs)
    {
        let mapped = ctx.mapper.get(glyph).unwrap();

        assert_eq!(
            old_metrics.advance_width(skrifa::GlyphId::new(glyph as u32)).unwrap(),
            new_metrics
                .advance_width(skrifa::GlyphId::new(mapped as u32))
                .unwrap(),
            "{:?}",
            format!("metric advance width didn't match for glyph {}.", glyph)
        );
    }
}

fn glyph_outlines_skrifa(font_file: &str, gids: &str) {
    let ctx = get_test_context(font_file, gids).unwrap();
    let old_face = skrifa::FontRef::from_index(&ctx.font, 0).unwrap();
    std::fs::write("out.otf", &ctx.subset);
    let new_face = skrifa::FontRef::from_index(&ctx.subset, 0).unwrap();

    let num_glyphs = old_face.maxp().unwrap().num_glyphs();

    for glyph in (0..num_glyphs).filter(|g| ctx.gids.contains(g)) {
        let mut sink1 = Sink(vec![]);
        let mut sink2 = Sink(vec![]);

        let new_glyph = ctx.mapper.get(glyph).unwrap();
        let settings = DrawSettings::unhinted(Size::unscaled(), LocationRef::default());

        if let Some(glyph1) =
            old_face.outline_glyphs().get(skrifa::GlyphId::new(glyph as u32))
        {
            glyph1.draw(settings, &mut sink1).unwrap();

            let settings =
                DrawSettings::unhinted(Size::unscaled(), LocationRef::default());
            let glyph2 = new_face
                .outline_glyphs()
                .get(skrifa::GlyphId::new(new_glyph as u32))
                .unwrap_or_else(|| panic!("failed to find glyph {} in new face", glyph));
            glyph2.draw(settings, &mut sink2).unwrap();

            if !compare_instructions(&sink1.0, &sink2.0) {
                assert_eq!(
                    sink1.0, sink2.0,
                    "glyph {} drawn with skrifa didn't match.",
                    glyph
                );
            }
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

        if old_face.outline_glyph(GlyphId(glyph), &mut sink1).is_some() {
            new_face.outline_glyph(GlyphId(new_glyph), &mut sink2);

            // Don't directly check that the outlines are equivalent to each other, because they are
            // most likely not going to be. We are rewriting the outlines by loading them with skrifa
            // and then dumping them again. It seems like skrifa and ttf-parser produce different
            // outlines (although the visual result will be the same), so the outlines produced by
            // ttf-parser might be different.

            // We just outline to make sure that _something_ happened.

            if !sink1.0.is_empty() {
                assert!(
                    !sink2.0.is_empty(),
                    "ttf-parser failed to outline a subsetted glyph."
                );
            }
        }
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

impl Inst {
    fn approx_eq(&self, other: &Inst) -> bool {
        const EPSILON: f32 = 2.0;

        match (self, other) {
            (Inst::MoveTo(x1, y1), Inst::MoveTo(x2, y2)) => {
                (x1 - x2).abs() < EPSILON && (y1 - y2).abs() < EPSILON
            }
            (Inst::LineTo(x1, y1), Inst::LineTo(x2, y2)) => {
                (x1 - x2).abs() < EPSILON && (y1 - y2).abs() < EPSILON
            }
            (Inst::QuadTo(x1, y1, x2, y2), Inst::QuadTo(x3, y3, x4, y4)) => {
                (x1 - x3).abs() < EPSILON
                    && (y1 - y3).abs() < EPSILON
                    && (x2 - x4).abs() < EPSILON
                    && (y2 - y4).abs() < EPSILON
            }
            (Inst::Close, Inst::Close) => true,
            _ => false,
        }
    }
}

fn compare_instructions(a: &[Inst], b: &[Inst]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    for (i1, i2) in a.iter().zip(b.iter()) {
        if !i1.approx_eq(i2) {
            return false;
        }
    }

    true
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
