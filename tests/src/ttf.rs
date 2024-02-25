use crate::{check_cmap, check_face_metrics, check_glyph_metrics, check_glyph_outlines};

#[test]
fn noto_sans_regular_glyph_outlines() {
    check_glyph_outlines("NotoSans-Regular.ttf", "*");
}
#[test]
fn noto_sans_regular_cmap() {
    check_cmap("NotoSans-Regular.ttf", "*");
}
#[test]
fn noto_sans_regular_face_metrics() {
    check_face_metrics("NotoSans-Regular.ttf", "*");
}
#[test]
fn noto_sans_regular_glyph_metrics() {
    check_glyph_metrics("NotoSans-Regular.ttf", "*");
}
