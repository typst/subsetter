# subsetter
[![Crates.io](https://img.shields.io/crates/v/subsetter.svg)](https://crates.io/crates/subsetter)
[![Documentation](https://docs.rs/subsetter/badge.svg)](https://docs.rs/subsetter)

Reduces the size and coverage of OpenType fonts with TrueType or CFF outlines.

```toml
[dependencies]
subsetter = "0.1"
```

## Example
In the example below, we remove all glyphs except the ones with IDs 68, 69, 70.
Those correspond to the letters 'a', 'b' and 'c'.

```rust
use subsetter::{subset, Profile};

// Read the raw font data.
let data = std::fs::read("fonts/NotoSans-Regular.ttf")?;

// Keep only three glyphs and the OpenType tables
// required for embedding the font in a PDF file.
let glyphs = &[68, 69, 70];
let profile = Profile::pdf(glyphs);
let sub = subset(&data, 0, profile)?;

// Write the resulting file.
std::fs::write("target/Noto-Small.ttf", sub)?;
```

Notably, this subsetter does not really remove glyphs, just their outlines. This
means that you don't have to worry about changed glyphs IDs. However, it also
means that the resulting font won't always be as small as possible. To somewhat
remedy this, this crate sometimes at least zeroes out unused data that it cannot
fully remove. This helps if the font gets compressed, for example when embedding
it in a PDF file.

In the above example, the original font was 375 KB (188 KB zipped) while the
resulting font is 36 KB (5 KB zipped).

## Limitations
Currently, the library only subsets static outline fonts. Furthermore, it is
designed for use cases where text was already mapped to glyphs. Possible future
work includes:

- The option to pass variation coordinates which would make the subsetter create
  a static instance of a variable font.
- Subsetting of bitmap, color and SVG tables.
- A profile which takes a char set instead of a glyph set and subsets the
  layout tables.

## Safety and Dependencies
This crate forbids unsafe code and has zero dependencies.

## License
This crate is dual-licensed under the MIT and Apache 2.0 licenses.
