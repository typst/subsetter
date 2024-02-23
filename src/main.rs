use std::env;
use subsetter::{subset, Profile};

fn main() {
    let args: Vec<String> = env::args().collect();
    // Read the raw font data.
    let data = std::fs::read(&args[1]).unwrap();

    // Keep only three glyphs and the OpenType tables
    // required for embedding the font in a PDF file.
    let glyphs = &[1, 3, 4, 6, 38, 39, 40, 41, 42, 43, 56, 90, 91, 92, 93, 345];
    let profile = Profile::pdf(glyphs);
    let sub = subset(&data, 0, profile).unwrap();

    // Write the resulting file.
    std::fs::write(&args[2], sub).unwrap();
}
