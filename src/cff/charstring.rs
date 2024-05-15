use ttf_parser::GlyphId;
use crate::cff::dict::Number;

enum Instruction<'a> {
    Operand(Number<'a>),
    Operator(u8)
}

struct CharString<'a> {
    bytecode: &'a [u8],
    decompiled: Vec<u8>,
    used_lsubs: Vec<u16>,
    used_gsubs: Vec<u16>,
    referenced_glyphs: Vec<GlyphId>
}

// impl CharString {
//     fn decompile(&mut self) -> Result<()> {
//
//     }
// }
