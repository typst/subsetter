use std::borrow::Cow;

type HmtxInterjector = Option<Box<dyn FnMut(u16) -> Option<(u16, u16)>>>;
type GlyfInterjector<'a> = Option<Box<dyn FnMut(u16) -> Option<Cow<'a, [u8]>>>>;

pub(crate) trait Interjector<'a> {
    fn horizontal_metrics(&self) -> HmtxInterjector;
    fn glyph_data(&self) -> GlyfInterjector<'a>;
}

pub(crate) struct DummyInterjector;

impl<'a> Interjector<'a> for DummyInterjector {
    fn horizontal_metrics(&self) -> HmtxInterjector {
        None
    }

    fn glyph_data(&self) -> GlyfInterjector<'a> {
        None
    }
}
