use crate::cff::charset::Charset;
use crate::stream::Writer;
use crate::Context;

pub(crate) fn subset_char_strings(
    char_strings: &Charset,
    ctx: &Context,
) -> Option<Vec<u8>> {
    let mut w = Writer::new();

    Some(w.finish())
}
