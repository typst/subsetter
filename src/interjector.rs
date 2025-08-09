type HmtxInterjector<'a> = Option<Box<dyn FnMut(u16) -> Option<(u16, i16)> + 'a>>;
type GlyfInterjector<'a> = Option<Box<dyn FnMut(u16) -> Option<Vec<u8>> + 'a>>;

pub(crate) trait Interjector {
    fn horizontal_metrics(&self) -> HmtxInterjector;
    fn glyph_data(&self) -> GlyfInterjector;
}

pub(crate) struct DummyInterjector;

impl Interjector for DummyInterjector {
    fn horizontal_metrics(&self) -> HmtxInterjector {
        None
    }

    fn glyph_data(&self) -> GlyfInterjector {
        None
    }
}

#[cfg(feature = "variable_fonts")]
pub(crate) mod skrifa {
    use crate::interjector::{GlyfInterjector, HmtxInterjector, Interjector};
    use kurbo::BezPath;
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
            let location = font_ref.axes().location(location.iter().map(|i| (i.0.as_str(), i.1)));

            Some(Self { font_ref, location })
        }
    }

    impl<'a> Interjector for SkrifaInterjector<'a> {
        fn horizontal_metrics(&self) -> HmtxInterjector {
            let metrics = self.font_ref.glyph_metrics(Size::unscaled(), &self.location);

            Some(Box::new(move |glyph| {
                // TODO: Is this the right thing to do? This might lead to a mismatch in PDF,
                // where advance widths are stored as integers.

                let adv = metrics.advance_width(GlyphId::new(glyph as u32))?;
                let lsb = metrics.left_side_bearing(GlyphId::new(glyph as u32))?;

                Some((adv.round() as u16, lsb.round() as i16))
            }))
        }

        fn glyph_data(&self) -> GlyfInterjector {
            let outlines = self.font_ref.outline_glyphs();

            Some(Box::new(move |glyph| {
                let mut outline_builder = OutlinePath(BezPath::new());
                let glyph = GlyphId::new(glyph as u32);

                if let Some(outline_glyph) = outlines.get(glyph) {
                    outline_glyph
                        .draw(
                            DrawSettings::unhinted(Size::unscaled(), &self.location),
                            &mut outline_builder,
                        )
                        .ok()?;
                }

                let path = outline_builder.0;

                if path.is_empty() {
                    return Some(vec![]);
                }

                let simple_glyph = SimpleGlyph::from_bezpath(&path).ok()?;
                let mut writer = TableWriter::default();
                simple_glyph.write_into(&mut writer);

                dump_table(&simple_glyph).ok()
            }))
        }
    }

    pub(crate) struct OutlinePath(pub(crate) BezPath);

    impl OutlinePen for OutlinePath {
        #[inline]
        fn move_to(&mut self, x: f32, y: f32) {
            self.0.move_to((x, y));
        }

        #[inline]
        fn line_to(&mut self, x: f32, y: f32) {
            self.0.line_to((x, y));
        }

        #[inline]
        fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
            self.0.quad_to((cx, cy), (x, y));
        }

        #[inline]
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {
            // TrueType glyphs cannot have cubic curves.
            unreachable!()
        }

        #[inline]
        fn close(&mut self) {
            self.0.close_path();
        }
    }
}
