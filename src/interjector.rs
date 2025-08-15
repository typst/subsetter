pub(crate) enum Interjector<'a> {
    Dummy,
    #[cfg(feature = "variable-fonts")]
    Skrifa(skrifa::SkrifaInterjector<'a>),
}

#[cfg(feature = "variable-fonts")]
pub(crate) mod skrifa {
    use crate::MaxpData;
    use kurbo::{BezPath, CubicBez};
    use skrifa::instance::Location;
    use skrifa::outline::{DrawSettings, OutlinePen};
    use skrifa::prelude::Size;
    use skrifa::{FontRef, GlyphId, MetadataProvider};
    use write_fonts::tables::glyf::SimpleGlyph;
    use write_fonts::{dump_table, FontWrite, TableWriter};

    pub(crate) struct SkrifaInterjector<'a> {
        font_ref: FontRef<'a>,
        location: Location,
    }

    impl<'a> SkrifaInterjector<'a> {
        pub(crate) fn new(
            data: &'a [u8],
            index: u32,
            location: &[(String, f32)],
        ) -> Option<Self> {
            let font_ref = FontRef::from_index(data, index).ok()?;
            let location =
                font_ref.axes().location(location.iter().map(|i| (i.0.as_str(), i.1)));

            Some(Self { font_ref, location })
        }
    }

    impl<'a> SkrifaInterjector<'a> {
        pub(crate) fn horizontal_metrics(&self, glyph: u16) -> Option<(u16, i16)> {
            let metrics = self.font_ref.glyph_metrics(Size::unscaled(), &self.location);

            let adv = metrics.advance_width(GlyphId::new(glyph as u32))?;
            // Note that for variable fonts, our left side bearing points don't seem to
            // match the ones from fonttools (they use some different technique for deriving
            // it which isn't reflected in skrifa's API), but I _think_ that this shouldn't
            // really be relevant in the context of PDF.
            let lsb = metrics.left_side_bearing(GlyphId::new(glyph as u32))?;

            Some((adv.round() as u16, lsb.round() as i16))
        }

        pub(crate) fn glyph_data<'b>(
            &'b self,
            maxp_data: &'b mut MaxpData,
            glyph: u16,
        ) -> Option<Vec<u8>> {
            let outlines = self.font_ref.outline_glyphs();

            let mut outline_builder = OutlinePath::new();
            let glyph = GlyphId::new(glyph as u32);

            if let Some(outline_glyph) = outlines.get(glyph) {
                outline_glyph
                    .draw(
                        DrawSettings::unhinted(Size::unscaled(), &self.location),
                        &mut outline_builder,
                    )
                    .ok()?;
            }

            let path = outline_builder.path;

            if path.is_empty() {
                return Some(vec![]);
            }

            let simple_glyph = SimpleGlyph::from_bezpath(&path).ok()?;

            maxp_data.max_points = maxp_data
                .max_points
                .max(simple_glyph.contours.iter().map(|c| c.len() as u16).sum());
            maxp_data.max_contours =
                maxp_data.max_contours.max(simple_glyph.contours.len() as u16);

            let mut writer = TableWriter::default();
            simple_glyph.write_into(&mut writer);

            dump_table(&simple_glyph).ok()
        }
    }

    pub(crate) struct OutlinePath {
        last_move_to: (f32, f32),
        last_point: (f32, f32),
        path: BezPath,
    }

    impl OutlinePath {
        fn new() -> Self {
            Self {
                last_move_to: (0.0, 0.0),
                last_point: (0.0, 0.0),
                path: BezPath::new(),
            }
        }
    }

    impl OutlinePen for OutlinePath {
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
            // Only called by TrueType fonts.
            self.path.quad_to((cx, cy), (x, y));
            self.last_point = (x, y);
        }

        #[inline]
        fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
            // Only called by CFF2 fonts.
            let cubic = CubicBez::new(
                (self.last_point.0 as f64, self.last_point.1 as f64),
                (cx0 as f64, cy0 as f64),
                (cx1 as f64, cy1 as f64),
                (x as f64, y as f64),
            );

            for (_, _, quad) in cubic.to_quads(1e-2) {
                // Note that `quad.p2` is the same as `quad.p0` of the next point in the iterator.
                self.quad_to(
                    quad.p1.x as f32,
                    quad.p1.y as f32,
                    quad.p2.x as f32,
                    quad.p2.y as f32,
                );
            }
        }

        #[inline]
        fn close(&mut self) {
            self.path.close_path();
            self.last_point = self.last_move_to;
        }
    }
}
