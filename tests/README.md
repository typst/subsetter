# Testing

## Requirements
You need to have `fonttools 4.50` installed on your system and in your PATH. Note that you need to have that
exact version, otherwise the tests will fail.

## Generating tests
In order to create new fonttools tests, you can edit `data/fonttools.tests`. For subset tests, you can edit 
`data/subsets.tests`

In order to generate the tests, run `scripts/gen-tests.py`.

## Description
Testing is very important, as having errors in the subsetting logic could have fatal consequences.
Because of this, we have three different testing approaches that cover 4 different 
font readers and 7 different PDF readers in total.

### Subset tests
We use `fontations`, `ttf-parser` and `freetype` to ensure that the outlines in the new font are the same as in the 
old font. By checking 3 different implementations, we can assert with relatively high confidence that there are
no issues in this regard. For each font, we test a selected subset of glyphs (see `data/subsets.tests`), but
for each font we also make one subset where all glyphs are included, to make sure that "rewriting the whole font"
also works as expected.

Using `ttf-parser`, we also check that the metrics for a glyph still match. This is especially useful for testing
`hmtx` subsetting.

### fonttools tests
`fonttools` has a feature that allows us to dump a font's internal structure as an XML file. This is
_incredibly_ useful, because it allows us to easily inspect the structure of a font file. We use this to
dump small subsets of fonts and compare the output to how fonttools would subset the font. This allows us
to identify other kinds of potential issues in the implementation. And it conveniently also allows us to
have a fourth implementation to test against.

### Fuzzing tests
In `examples`, we have a binary that takes an environment variable `FONT_DIR` and recursively iterates over all fonts
in that directory and basically performs the same test as in #1, but on a randomly selected sets of glyphs. We currently
try to run the fuzzer every once in a while on a set of 1000+ fonts to make sure it that the subsetter also works with
other fonts than the one included in this repository.

### PDF tests
Occasionally, we will also use a subset of fonts and use `typst` to create a PDF file with it and check
that the output looks correct in Adobe Acrobat, mupdf, xpdf, Firefox, Chrome, Apple Preview and pdfbox. These tests
only happen manually though.