use std::env;
use subsetter::subset;

fn main() {
    let args: Vec<String> = env::args().collect();
    // Read the raw font data.
    let data = std::fs::read(&args[1]).unwrap();

    // Keep only three glyphs and the OpenType tables
    // required for embedding the font in a PDF file.
    let mut glyphs = vec![];
    glyphs.extend(400..=420);
    let (sub, _) = subset(&data, 0, &glyphs).unwrap();

    // Write the resulting file.
    std::fs::write(&args[2], sub).unwrap();
}
