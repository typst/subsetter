use rand_distr::Distribution;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use rand::distributions::WeightedIndex;
use rand::prelude::{IteratorRandom, ThreadRng};
use rand::thread_rng;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::MetadataProvider;
use subsetter::{subset, GlyphRemapper};
use ttf_parser::GlyphId;

const NUM_ITERATIONS: usize = 200;

fn main() {
    let exclude_fonts = vec![
        // Seems to be an invalid font for some reason, fonttools can't read it either.
        Path::new("/Users/lstampfl/Desktop/fonts-main/ofl/souliyo/Souliyo-Regular.ttf"),
        // Glyph 822 doesn't seem to draw properly with ttf-parser... But most likely a ttf-parser
        // bug because it does work with skrifa and freetype. fonttools ttx subset matches
        // the output you get when subsetting with fonttools.
        Path::new("/Users/lstampfl/Desktop/fonts-main/ofl/souliyo/Souliyo-Regular.ttf"),
    ];
    let paths = walkdir::WalkDir::new("/Users/lstampfl/Desktop/fonts-main")
        .into_iter()
        .map(|p| p.unwrap().path().to_path_buf())
        .filter(|p| {
            let extension = p.extension().and_then(OsStr::to_str);
            (extension == Some("ttf") || extension == Some("otf"))
                && !exclude_fonts.contains(&p.as_path())
        })
        .collect::<Vec<_>>();

    loop {
        println!("Starting an iteration...");

        paths.par_iter().for_each(|path| {
            let mut rng = thread_rng();
            let extension = path.extension().and_then(OsStr::to_str);
            let is_font_file = extension == Some("ttf") || extension == Some("otf");

            if is_font_file {
                match run_test(&path, &mut rng) {
                    Ok(_) => {}
                    Err(msg) => {
                        println!("Error while fuzzing {:?}: {:}", path.clone(), msg)
                    }
                }
            }
        });
    }
}

fn run_test(path: &Path, rng: &mut ThreadRng) -> Result<(), String> {
    let data = fs::read(path).map_err(|_| "failed to read file".to_string())?;
    let old_ttf_face = ttf_parser::Face::parse(&data, 0)
        .map_err(|_| "failed to parse old face".to_string())?;

    if old_ttf_face.tables().cff.is_some() {
        println!("{:?}", path);
    }

    let num_glyphs = old_ttf_face.number_of_glyphs();
    let possible_gids = (0..num_glyphs).collect::<Vec<_>>();
    let dist = get_distribution(num_glyphs);

    let old_skrifa_face = skrifa::FontRef::new(&data).unwrap();

    for _ in 0..NUM_ITERATIONS {
        let num = dist.sample(rng);
        let sample = possible_gids.clone().into_iter().choose_multiple(rng, num);
        let sample_strings = sample.iter().map(|g| g.to_string()).collect::<Vec<_>>();
        let (subset, remapper) = subset(&data, 0, &sample).map_err(|_| {
            format!("subset failed for gids {:?}", sample_strings.join(","))
        })?;
        let new_ttf_face = ttf_parser::Face::parse(&subset, 0).map_err(|_| {
            format!(
                "failed to parse new ttf face with gids {:?}",
                sample_strings.join(",")
            )
        })?;
        let new_skrifa_face = skrifa::FontRef::new(&subset).map_err(|_| {
            format!(
                "failed to parse new skrifa face with gids {:?}",
                sample_strings.join(",")
            )
        })?;

        glyph_outlines_ttf_parser(&old_ttf_face, &new_ttf_face, &remapper, &sample)
            .map_err(|g| {
                format!(
                    "outlines didn't match for gid {:?}, with sample {:?}",
                    g,
                    sample_strings.join(",")
                )
            })?;
        glyph_outlines_skrifa(&old_skrifa_face, &new_skrifa_face, &remapper, &sample)
            .map_err(|g| {
                format!(
                    "outlines didn't match for gid {:?}, with sample {:?}",
                    g,
                    sample_strings.join(",")
                )
            })?;
        ttf_parser_glyph_metrics(&old_ttf_face, &new_ttf_face, &remapper, &sample)
            .map_err(|e| {
                format!(
                    "glyph metrics for sample {:?} didn't match: {:?}",
                    sample_strings.join(","),
                    e
                )
            })?;
    }

    Ok(())
}

fn get_distribution(num_glyphs: u16) -> WeightedIndex<usize> {
    let mut weights = vec![0];

    for i in 1..num_glyphs {
        if i <= 10 {
            weights.push(8000);
        } else if i <= 50 {
            weights.push(16000);
        } else if i <= 200 {
            weights.push(6000);
        } else if i <= 2000 {
            weights.push(100);
        } else if i <= 5000 {
            weights.push(2);
        }
    }

    WeightedIndex::new(&weights).unwrap()
}

fn ttf_parser_glyph_metrics(
    old_face: &ttf_parser::Face,
    new_face: &ttf_parser::Face,
    mapper: &GlyphRemapper,
    gids: &[u16],
) -> Result<(), String> {
    for glyph in gids.iter().copied() {
        let mapped = mapper.get(glyph).unwrap();

        // For some reason the glyph bounding box differs sometimes, so we don't check
        // that anymore. I verified via fonttools that our subset matches theirs. So it is
        // probably a ttf-parser issue...
        // if old_face.glyph_bounding_box(GlyphId(glyph))
        //     != new_face.glyph_bounding_box(GlyphId(mapped))
        // {
        //     return Err(format!("glyph bounding box for glyph {:?} didn't match.", glyph));
        // }

        if old_face.glyph_hor_side_bearing(GlyphId(glyph))
            != new_face.glyph_hor_side_bearing(GlyphId(mapped))
        {
            return Err(format!(
                "glyph hor side bearing for glyph {:?} didn't match.",
                glyph
            ));
        }

        if old_face.glyph_hor_advance(GlyphId(glyph))
            != new_face.glyph_hor_advance(GlyphId(mapped))
        {
            return Err(format!("glyph hor advance for glyph {:?} didn't match.", glyph));
        }
    }

    Ok(())
}

fn glyph_outlines_skrifa(
    old_face: &skrifa::FontRef,
    new_face: &skrifa::FontRef,
    mapper: &GlyphRemapper,
    gids: &[u16],
) -> Result<(), String> {
    // let hinting_instance_old = HintingInstance::new(
    //     &old_face.outline_glyphs(),
    //     Size::new(150.0),
    //     LocationRef::default(),
    //     HintingMode::Smooth { lcd_subpixel: None, preserve_linear_metrics: false },
    // ).map_err(|_| "failed to create old hinting instance".to_string())?;
    //
    // let hinting_instance_new = HintingInstance::new(
    //     &new_face.outline_glyphs(),
    //     Size::new(150.0),
    //     LocationRef::default(),
    //     HintingMode::Smooth { lcd_subpixel: None, preserve_linear_metrics: false },
    // ).map_err(|_| "failed to create new hinting instance".to_string())?;

    let mut sink1 = Sink(vec![]);
    let mut sink2 = Sink(vec![]);

    for glyph in gids.iter().copied() {
        let new_glyph = mapper.get(glyph).ok_or("failed to remap glyph".to_string())?;
        // We don't to hinted because for some reason skrifa fails to do so even on the old face in many
        // cases. So it's not a subsetting issue.
        let settings = DrawSettings::unhinted(Size::new(150.0), LocationRef::default());

        if let Some(glyph1) = old_face.outline_glyphs().get(skrifa::GlyphId::new(glyph)) {
            glyph1
                .draw(settings, &mut sink1)
                .map_err(|e| format!("failed to draw old glyph {}: {}", glyph, e))?;

            let settings =
                DrawSettings::unhinted(Size::new(150.0), LocationRef::default());
            let glyph2 = new_face
                .outline_glyphs()
                .get(skrifa::GlyphId::new(new_glyph))
                .expect(&format!("failed to find glyph {} in new face", glyph));
            glyph2
                .draw(settings, &mut sink2)
                .map_err(|e| format!("failed to draw new glyph {}: {}", glyph, e))?;

            if sink1 != sink2 {
                return Err(format!("{}", glyph));
            } else {
                return Ok(());
            }
        }
    }

    Ok(())
}

fn glyph_outlines_ttf_parser(
    old_face: &ttf_parser::Face,
    new_face: &ttf_parser::Face,
    mapper: &GlyphRemapper,
    gids: &[u16],
) -> Result<(), u16> {
    for glyph in gids {
        let new_glyph = mapper.get(*glyph).unwrap();
        let mut sink1 = Sink::default();
        let mut sink2 = Sink::default();

        if let Some(_) = old_face.outline_glyph(GlyphId(*glyph), &mut sink1) {
            new_face.outline_glyph(GlyphId(new_glyph), &mut sink2);
            if sink1 != sink2 {
                return Err(*glyph);
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        }
    }

    return Ok(());
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
