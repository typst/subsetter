use std::env;
use subsetter::{subset, Profile};

fn main() {
    let args: Vec<String> = env::args().collect();
    // Read the raw font data.
    let data = std::fs::read(&args[1]).unwrap();

    // Keep only three glyphs and the OpenType tables
    // required for embedding the font in a PDF file.
    let mut glyphs = vec![];
    // glyphs.extend(80..=100);
    glyphs.extend([3,4]);
    let profile = Profile::pdf(&glyphs);
    let sub = subset(&data, 0, profile).unwrap();

    // Write the resulting file.
    std::fs::write(&args[2], sub).unwrap();
}
