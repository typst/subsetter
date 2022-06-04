use std::path::Path;

use subsetter::{parse, subset, Profile};

#[test]
fn test_subset() {
    test("NotoSans-Regular.ttf", "Hello");
    test("NewCMMath-Regular.otf", "1 + 2 = π?");
}

fn test(path: &str, text: &str) {
    let data = std::fs::read(Path::new("fonts").join(path)).unwrap();
    let ttf = ttf_parser::Face::from_slice(&data, 0).unwrap();
    let glyphs: Vec<_> =
        text.chars().filter_map(|c| Some(ttf.glyph_index(c)?.0)).collect();

    let face = parse(&data, 0).unwrap();
    let profile = Profile::pdf(&glyphs);
    let subs = subset(&face, profile).unwrap();
    let out = Path::new("target").join(Path::new(path).with_extension("ttf"));
    std::fs::write(out, &subs).unwrap();

    let ttfs = ttf_parser::Face::from_slice(&subs, 0).unwrap();
    for c in text.chars() {
        macro_rules! same {
            ($method:ident, $($args:tt)*) => {
                assert_eq!(
                    ttf.$method($($args)*),
                    ttfs.$method($($args)*),
                );
            };
        }
        let id = ttf.glyph_index(c).unwrap();
        same!(glyph_index, c);
        same!(glyph_hor_advance, id);
        same!(glyph_hor_side_bearing, id);
        same!(glyph_bounding_box, id);
    }
}
