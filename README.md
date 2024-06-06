# subsetter
[![Crates.io](https://img.shields.io/crates/v/subsetter.svg)](https://crates.io/crates/subsetter)
[![Documentation](https://docs.rs/subsetter/badge.svg)](https://docs.rs/subsetter)

Reduces the size and coverage of OpenType fonts with TrueType or CFF outlines for embedding
in PDFs. You can in general expect very good results in terms of font size, as most of the things
that can be subsetted are also subsetted.

# Scope
**Note that the resulting font subsets will most likely be unusable in any other contexts than PDF writing,
since a lot of information will be removed from the font which is not necessary in PDFs, but is
necessary in other contexts.** This is on purpose, and for now, there are no plans to expand the
scope of this crate to become a general purpose subsetter, as this is a massive undertaking and
will make the already complex codebase even more complex.

In the future,
[klippa](https://github.com/googlefonts/fontations/tree/main/klippa) will hopefully fill this gap.

For an example on how to use this crate, have a look at the 
[documentation](https://docs.rs/subsetter/latest/subsetter/).

## Limitations
As mentioned above, this crate is specifically aimed at subsetting a font with the purpose of 
including it in a PDF file. For any other purposes, this crate will most likely not be very useful.

Potential future work could include allowing to define variation coordinates for which to generate
the subset for. However, apart from that there are no plans to increase the scope of this crate, apart from
fixing bugs and adding new APIs to the existing interface.

## Safety and Dependencies
This crate forbids unsafe code and has zero dependencies.

## License
This crate is dual-licensed under the MIT and Apache 2.0 licenses.
