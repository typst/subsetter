use crate::cff::argstack::ArgumentsStack;
use crate::cff::charstring::Instruction::Operator;
use crate::cff::dict::Number;
use crate::cff::operator;
use crate::stream::Reader;
use crate::Error::MalformedFont;
use crate::{Error, Result};
use std::cell::RefCell;
use std::rc::Rc;

type SharedCharString<'a> = Rc<RefCell<CharString<'a>>>;

pub struct Decompiler<'a, 'b> {
    lsubrs: &'b [SharedCharString<'a>],
    lsubrs_bias: u16,
    gsubrs: &'b [SharedCharString<'a>],
    gsubrs_bias: u16,
    stack: ArgumentsStack<'a>,
    hint_count: u16,
    hint_mask_bytes: u16,
}

impl<'a, 'b> Decompiler<'a, 'b> {
    pub fn new(
        lsubrs: &'b [SharedCharString<'a>],
        gsubrs: &'b [SharedCharString<'a>],
    ) -> Self {
        Self {
            lsubrs,
            gsubrs,
            lsubrs_bias: calc_subroutine_bias(lsubrs.len() as u32),
            gsubrs_bias: calc_subroutine_bias(gsubrs.len() as u32),
            stack: ArgumentsStack::new(),
            hint_count: 0,
            hint_mask_bytes: 0,
        }
    }

    fn count_hints(&mut self) {
        let elements = self.stack.pop_all();
        self.hint_count += elements.len() as u16 / 2;
    }
}

#[derive(Debug)]
pub enum Instruction<'a> {
    Operand(Number<'a>),
    Operator(u8),
}

pub struct CharString<'a> {
    bytecode: &'a [u8],
    pub decompiled: Vec<Instruction<'a>>,
    used_lsubs: Vec<u32>,
    used_gsubs: Vec<u32>,
    referenced_glyphs: Vec<u16>,
}

impl<'a> CharString<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            bytecode: data,
            decompiled: vec![],
            used_gsubs: vec![],
            used_lsubs: vec![],
            referenced_glyphs: vec![],
        }
    }

    pub fn decompile(
        &mut self,
        decompiler: &mut Decompiler<'a, '_>,
    ) -> Result<&[Instruction]> {
        if self.decompiled.len() > 0 {
            return Ok(self.decompiled.as_ref());
        }

        let mut instructions = vec![];
        let mut r = Reader::new(self.bytecode);

        while !r.at_end() {
            // We need to peak instead of read because parsing a number requires
            // access to the whole buffer. This means that for each operator, we need
            // to add another read manually.
            let op = r.peak::<u8>().ok_or(Error::MalformedFont)?;

            match op {
                0 | 2 | 9 | 13 | 15 | 16 | 17 => {
                    // Reserved.
                    return Err(Error::MalformedFont);
                }
                operator::TWO_BYTE_OPERATOR_MARK => unimplemented!(),
                operator::HORIZONTAL_STEM
                | operator::VERTICAL_STEM
                | operator::HORIZONTAL_STEM_HINT_MASK
                | operator::VERTICAL_STEM_HINT_MASK => {
                    r.read::<u8>();
                    decompiler.count_hints();
                    instructions.push(Operator(op));
                }
                operator::VERTICAL_MOVE_TO
                | operator::HORIZONTAL_MOVE_TO
                | operator::LINE_TO
                | operator::VERTICAL_LINE_TO
                | operator::HORIZONTAL_LINE_TO
                | operator::MOVE_TO
                | operator::HORIZONTAL_MOVE_TO
                | operator::CURVE_LINE
                | operator::LINE_CURVE
                | operator::VV_CURVE_TO
                | operator::VH_CURVE_TO
                | operator::HH_CURVE_TO
                | operator::HV_CURVE_TO
                | operator::CURVE_TO
                | operator::RETURN => {
                    r.read::<u8>();
                    decompiler.stack.pop_all();
                    instructions.push(Instruction::Operator(op))
                }
                28 | 32..=254 => {
                    let number = Number::parse(&mut r).ok_or(MalformedFont)?;
                    decompiler.stack.push(number.clone())?;
                    instructions.push(Instruction::Operand(number));
                }
                operator::CALL_GLOBAL_SUBROUTINE => {
                    r.read::<u8>();
                    instructions.push(Operator(op));
                    // TODO: Add depth limit
                    // TODO: Recursion detector
                    let biased_index = decompiler
                        .stack
                        .pop()
                        .and_then(|n| n.as_i32())
                        .ok_or(MalformedFont)?;
                    let gsubr_index =
                        conv_subroutine_index(biased_index, decompiler.gsubrs_bias)
                            .ok_or(MalformedFont)?;
                    let gsubr = decompiler
                        .gsubrs
                        .get(gsubr_index as usize)
                        .ok_or(MalformedFont)?
                        .clone();
                    gsubr.borrow_mut().decompile(decompiler)?;
                    self.used_gsubs.push(gsubr_index);
                }
                operator::CALL_LOCAL_SUBROUTINE => {
                    r.read::<u8>();
                    instructions.push(Operator(op));
                    // TODO: Add depth limit
                    // TODO: Recursion detector
                    let biased_index = decompiler
                        .stack
                        .pop()
                        .and_then(|n| n.as_i32())
                        .ok_or(MalformedFont)?;
                    let lsubr_index =
                        conv_subroutine_index(biased_index, decompiler.lsubrs_bias)
                            .ok_or(MalformedFont)?;
                    let lsubr = decompiler
                        .lsubrs
                        .get(lsubr_index as usize)
                        .ok_or(MalformedFont)?
                        .clone();
                    lsubr.borrow_mut().decompile(decompiler)?;
                    self.used_lsubs.push(lsubr_index);
                }
                operator::HINT_MASK | operator::COUNTER_MASK => {
                    r.read::<u8>();
                    instructions.push(Operator(op));
                    if decompiler.hint_mask_bytes == 0 {
                        decompiler.count_hints();
                        decompiler.hint_mask_bytes = (decompiler.hint_count + 7) / 8;
                    }
                }
                operator::ENDCHAR => {
                    // TODO: Add seac
                    r.read::<u8>();
                    instructions.push(Operator(op));
                }
                operator::FIXED_16_16 => unimplemented!(),
            }
        }

        Ok(self.decompiled.as_ref())
    }
}

pub fn calc_subroutine_bias(len: u32) -> u16 {
    if len < 1240 {
        107
    } else if len < 33900 {
        1131
    } else {
        32768
    }
}

fn conv_subroutine_index(index: i32, bias: u16) -> Option<u32> {
    let bias = i32::from(bias);

    let index = index.checked_add(bias)?;
    u32::try_from(index).ok()
}
