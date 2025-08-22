use crate::Error::MalformedFont;
use crate::{FontFlavor, GlyphRemapper, Tag};
use kurbo::{BezPath, CubicBez, Rect, Shape};
use skrifa::instance::Size;
use skrifa::outline::{DrawSettings, OutlineGlyphFormat, OutlinePen};
use skrifa::MetadataProvider;
use skrifa::{FontRef, GlyphId, OutlineGlyphCollection};

pub(crate) fn get_font_data(
    data: &[u8],
    index: u32,
    variation_coordinates: &[(Tag, f32)],
    font_flavor: FontFlavor,
    remapper: &GlyphRemapper,
) -> crate::Result<FontMetrics> {
    let font_ref =
        FontRef::from_index(data, index).map_err(|_| MalformedFont)?;
    let location = font_ref.axes().location(
        variation_coordinates
            .iter()
            .map(|i| (skrifa::Tag::new(i.0.get()), i.1)),
    );

    let outlines = match font_flavor {
        FontFlavor::TrueType => {
            OutlineGlyphCollection::with_format(&font_ref, OutlineGlyphFormat::Glyf)
        }
        FontFlavor::Cff => {
            OutlineGlyphCollection::with_format(&font_ref, OutlineGlyphFormat::Cff)
        }
        FontFlavor::Cff2 => {
            OutlineGlyphCollection::with_format(&font_ref, OutlineGlyphFormat::Cff2)
        }
    }
    .ok_or(MalformedFont)?;
    let metrics = font_ref.glyph_metrics(Size::unscaled(), &location);

    let mut glyph_data = Vec::new();
    let mut global_bbox: Option<Rect> = None;

    for glyph in remapper.remapped_gids() {
        let mut outline_builder = OutlineBuilder::new(font_flavor);
        let glyph = GlyphId::new(glyph as u32);

        if let Some(outline_glyph) = outlines.get(glyph) {
            outline_glyph
                .draw(
                    DrawSettings::unhinted(Size::unscaled(), &location),
                    &mut outline_builder,
                )
                .map_err(|_| MalformedFont)?;
        }

        let path = outline_builder.path;

        let advance_width =
            metrics.advance_width(glyph).ok_or(MalformedFont)?.round() as u16;
        let lsb = metrics.left_side_bearing(glyph).ok_or(MalformedFont)?.round() as i16;

        let bbox = path.bounding_box().expand();

        global_bbox = Some(global_bbox.map(|g| g.union(bbox)).unwrap_or(bbox));

        glyph_data.push(GlyphData {
            path,
            advance_width,
            lsb,
            bbox: Bbox::from_rect(&bbox),
        });
    }

    Ok(FontMetrics {
        glyph_data,
        global_bbox: Bbox::from_rect(
            &global_bbox.unwrap_or(Rect::new(0.0, 0.0, 1.0, 1.0)),
        ),
    })
}

pub(crate) struct Bbox {
    pub(crate) left: i16,
    pub(crate) top: i16,
    pub(crate) right: i16,
    pub(crate) bottom: i16,
}

impl Bbox {
    fn from_rect(rect: &Rect) -> Self {
        Self {
            left: rect.min_x() as i16,
            top: rect.min_y() as i16,
            right: rect.max_x() as i16,
            bottom: rect.max_y() as i16,
        }
    }
}

pub(crate) struct FontMetrics {
    pub(crate) glyph_data: Vec<GlyphData>,
    pub(crate) global_bbox: Bbox,
}

pub(crate) struct GlyphData {
    pub(crate) path: BezPath,
    pub(crate) advance_width: u16,
    pub(crate) lsb: i16,
    pub(crate) bbox: Bbox,
}

struct OutlineBuilder {
    last_move_to: (f32, f32),
    last_point: (f32, f32),
    path: BezPath,
    convert_cubics: bool,
}

impl OutlineBuilder {
    fn new(font_flavor: FontFlavor) -> Self {
        Self {
            last_move_to: (0.0, 0.0),
            last_point: (0.0, 0.0),
            path: BezPath::new(),
            convert_cubics: font_flavor == FontFlavor::Cff2,
        }
    }
}

impl OutlinePen for OutlineBuilder {
    #[inline]
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to((x, y));
        self.last_move_to = (x, y);
        self.last_point = (x, y);
    }

    #[inline]
    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to((x, y));
        self.last_point = (x, y);
    }

    #[inline]
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.path.quad_to((cx, cy), (x, y));
        self.last_point = (x, y);
    }

    #[inline]
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        if self.convert_cubics {
            // Only called by CFF2 fonts.
            let cubic = CubicBez::new(
                (self.last_point.0 as f64, self.last_point.1 as f64),
                (cx0 as f64, cy0 as f64),
                (cx1 as f64, cy1 as f64),
                (x as f64, y as f64),
            );

            // It is not entirely clear how small the `accuracy` parameter needs to be
            // to produce sensible results. In `vello_cpu`, a value of around 0.025 is used
            // (0.25 for the flattening accuracy and * 0.1 in the flattening code, so we choose
            // the same value here.
            for (_, _, quad) in cubic.to_quads(0.025) {
                // Note that `quad.p2` is the same as `quad.p0` of the next point in the iterator.
                self.quad_to(
                    quad.p1.x as f32,
                    quad.p1.y as f32,
                    quad.p2.x as f32,
                    quad.p2.y as f32,
                );
            }
        } else {
            self.path.curve_to((cx0, cy0), (cx1, cy1), (x, y));
            self.last_point = (x, y);
        }
    }

    #[inline]
    fn close(&mut self) {
        self.path.close_path();
        self.last_point = self.last_move_to;
    }
}
