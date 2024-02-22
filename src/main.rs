use std::env;
use subsetter::{Profile, subset};

fn main() {

    let args: Vec<String> = env::args().collect();
    // Read the raw font data.
    let data = std::fs::read(&args[1]).unwrap();

    // Keep only three glyphs and the OpenType tables
    // required for embedding the font in a PDF file.
    let glyphs = &[1, 4, 6, 56, 38, 39, 1, 90, 345, 3, 43534];
    let profile = Profile::pdf(glyphs);
    let sub = subset(&data, 0, profile).unwrap();

    // Write the resulting file.
    std::fs::write(&args[2], sub).unwrap();
}