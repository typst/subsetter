use crate::cff::argstack::ArgumentsStack;
use crate::cff::operator::Operator;
use crate::cff::subroutines::SubroutineHandler;
use crate::cff::types::Number;
use crate::read::{Readable, Reader};
use crate::write::Writer;
use crate::Error::MalformedFont;
use crate::{Error, Result};
use operators::*;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;

pub type SharedCharString<'a> = Rc<RefCell<CharString<'a>>>;

pub struct Decompiler<'a, 'b> {
    gsubr_handler: SubroutineHandler<'a, 'b>,
    lsubr_handler: SubroutineHandler<'a, 'b>,
    stack: ArgumentsStack<'a>,
    hint_count: u16,
    hint_mask_bytes: u16,
}

impl<'a, 'b> Decompiler<'a, 'b> {
    pub fn new(
        gsubr_handler: SubroutineHandler<'a, 'b>,
        lsubr_handler: SubroutineHandler<'a, 'b>,
    ) -> Self {
        Self {
            gsubr_handler,
            lsubr_handler,
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
    Operator(Operator),
    HintMask(&'a [u8]),
}

#[derive(Default)]
pub struct Program<'a>(Vec<Instruction<'a>>);

impl Debug for Program<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut formatted_strings = vec![];
        let mut str_buffer = vec![];

        for instr in &self.0 {
            match instr {
                Instruction::Operand(op) => str_buffer.push(format!("{}", op.as_f64())),
                Instruction::Operator(op) => {
                    str_buffer.push(format!("op({})", op));

                    if *op != HINT_MASK && *op != COUNTER_MASK {
                        formatted_strings.push(str_buffer.join(" "));
                        str_buffer.clear();
                    }
                }
                Instruction::HintMask(bytes) => {
                    let mut byte_string = String::new();

                    for byte in *bytes {
                        byte_string.push_str(&format!("{:08b}", *byte));
                    }

                    str_buffer.push(byte_string);
                    formatted_strings.push(str_buffer.join(" "));
                    str_buffer.clear();
                }
            }
        }

        write!(f, "{}", formatted_strings.join("\n"))
    }
}

impl<'a> Program<'a> {
    pub fn instructions(&self) -> &[Instruction<'a>] {
        self.0.as_ref()
    }

    pub fn push(&mut self, instruction: Instruction<'a>) {
        self.0.push(instruction);
    }

    pub fn compile(&self) -> Vec<u8> {
        let mut w = Writer::new();

        for instr in &self.0 {
            match instr {
                Instruction::Operand(op) => {
                    w.extend(op.as_bytes());
                }
                Instruction::Operator(op) => {
                    w.write(op.as_bytes());
                }
                Instruction::HintMask(hm) => {
                    w.extend(*hm);
                }
            }
        }

        w.finish()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

pub struct CharString<'a> {
    bytecode: &'a [u8],
    pub program: Program<'a>,
    used_lsubs: BTreeSet<u32>,
    used_gsubs: BTreeSet<u32>,
    referenced_glyphs: Vec<u16>,
}

impl<'a> CharString<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            bytecode: data,
            program: Program::default(),
            used_gsubs: BTreeSet::new(),
            used_lsubs: BTreeSet::new(),
            referenced_glyphs: vec![],
        }
    }

    pub fn used_lsubs(&self) -> Option<&BTreeSet<u32>> {
        if self.program.len() == 0 {
            None
        } else {
            Some(&self.used_lsubs)
        }
    }

    pub fn used_gsubs(&self) -> Option<&BTreeSet<u32>> {
        if self.program.len() == 0 {
            None
        } else {
            Some(&self.used_gsubs)
        }
    }

    pub fn decompile(
        &mut self,
        decompiler: &mut Decompiler<'a, '_>,
    ) -> Result<&[Instruction]> {
        let mut r = Reader::new(self.bytecode);
        let needs_decompilation = self.program.len() == 0;

        while !r.at_end() {
            // We always need to execute the subroutine, because a subroutine
            // has an effect on the state of the stack, hinting, etc., meaning
            // that if we don't execute it, the result will be wrong. However, we
            // only need to decompile the program (= push instructions to the program)
            // if it's empty.
            let mut push_instr: Box<dyn FnMut(_) -> ()> = if needs_decompilation {
                Box::new(|instr| self.program.push(instr))
            } else {
                Box::new(|_| {})
            };

            // We need to peak instead of read because parsing a number requires
            // access to the whole buffer.
            let op = r.peak::<u8>().ok_or(Error::MalformedFont)?;

            // Numbers
            if matches!(op, 28 | 32..=255) {
                let number =
                    Number::parse_charstring_number(&mut r).ok_or(MalformedFont)?;
                decompiler.stack.push(number.clone())?;
                push_instr(Instruction::Operand(number));
                continue;
            }

            let op = r.read::<u8>().ok_or(Error::MalformedFont)?;
            let operator = if op == 12 {
                Operator::from_two_byte(r.read::<u8>().ok_or(Error::MalformedFont)?)
            } else {
                Operator::from_one_byte(op)
            };

            match operator {
                HFLEX | FLEX | HFLEX1 | FLEX1 => {
                    decompiler.stack.pop_all();
                    push_instr(Instruction::Operator(operator));
                }
                HORIZONTAL_STEM
                | VERTICAL_STEM
                | HORIZONTAL_STEM_HINT_MASK
                | VERTICAL_STEM_HINT_MASK => {
                    decompiler.count_hints();
                    push_instr(Instruction::Operator(operator));
                }
                VERTICAL_MOVE_TO | HORIZONTAL_MOVE_TO | LINE_TO | VERTICAL_LINE_TO
                | HORIZONTAL_LINE_TO | MOVE_TO | CURVE_LINE | LINE_CURVE
                | VV_CURVE_TO | VH_CURVE_TO | HH_CURVE_TO | HV_CURVE_TO | CURVE_TO => {
                    decompiler.stack.pop_all();
                    push_instr(Instruction::Operator(operator));
                }
                RETURN => {
                    push_instr(Instruction::Operator(operator));
                }
                CALL_GLOBAL_SUBROUTINE => {
                    push_instr(Instruction::Operator(operator));

                    // TODO: Add depth limit
                    // TODO: Recursion detector
                    let biased_index = decompiler
                        .stack
                        .pop()
                        .and_then(|n| n.as_i32())
                        .ok_or(MalformedFont)?;
                    let gsubr = decompiler
                        .gsubr_handler
                        .get_with_biased(biased_index)
                        .ok_or(MalformedFont)?;
                    gsubr.char_string.borrow_mut().decompile(decompiler)?;
                    self.used_gsubs.insert(gsubr.unbiased_index);
                    // Make sure used lsubs and gsubs are propagated transitively.
                    // TODO Maybe don't do this?
                    self.used_lsubs.extend(&gsubr.char_string.borrow().used_lsubs);
                    self.used_gsubs.extend(&gsubr.char_string.borrow().used_gsubs);
                }
                CALL_LOCAL_SUBROUTINE => {
                    push_instr(Instruction::Operator(operator));
                    // TODO: Add depth limit
                    // TODO: Recursion detector
                    let biased_index = decompiler
                        .stack
                        .pop()
                        .and_then(|n| n.as_i32())
                        .ok_or(MalformedFont)?;
                    let lsubr = decompiler
                        .lsubr_handler
                        .get_with_biased(biased_index)
                        .ok_or(MalformedFont)?;
                    lsubr.char_string.borrow_mut().decompile(decompiler)?;
                    self.used_lsubs.insert(lsubr.unbiased_index);
                    // Make sure used lsubs and gsubs are propagated transitively.
                    self.used_lsubs.extend(&lsubr.char_string.borrow().used_lsubs);
                    self.used_gsubs.extend(&lsubr.char_string.borrow().used_gsubs);
                }
                HINT_MASK | COUNTER_MASK => {
                    push_instr(Instruction::Operator(operator));
                    if decompiler.hint_mask_bytes == 0 {
                        decompiler.count_hints();
                        decompiler.hint_mask_bytes = (decompiler.hint_count + 7) / 8;
                    }

                    let hint_bytes = r
                        .read_bytes(decompiler.hint_mask_bytes as usize)
                        .ok_or(MalformedFont)?;
                    push_instr(Instruction::HintMask(hint_bytes));
                }
                ENDCHAR => {
                    // TODO: Add seac!
                    push_instr(Instruction::Operator(operator));
                }
                _ => return Err(MalformedFont),
            }
        }

        Ok(self.program.instructions())
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

pub fn unapply_bias(index: i32, bias: u16) -> Option<u32> {
    let bias = i32::from(bias);

    let index = index.checked_add(bias)?;
    u32::try_from(index).ok()
}

pub fn apply_bias(index: i32, bias: u16) -> Option<i32> {
    let bias = i32::from(bias);

    index.checked_sub(bias)
}

#[allow(dead_code)]
mod operators {
    use crate::cff::operator::Operator;

    pub const HORIZONTAL_STEM: Operator = Operator::from_one_byte(1);
    pub const VERTICAL_STEM: Operator = Operator::from_one_byte(3);
    pub const VERTICAL_MOVE_TO: Operator = Operator::from_one_byte(4);
    pub const LINE_TO: Operator = Operator::from_one_byte(5);
    pub const HORIZONTAL_LINE_TO: Operator = Operator::from_one_byte(6);
    pub const VERTICAL_LINE_TO: Operator = Operator::from_one_byte(7);
    pub const CURVE_TO: Operator = Operator::from_one_byte(8);
    pub const CALL_LOCAL_SUBROUTINE: Operator = Operator::from_one_byte(10);
    pub const RETURN: Operator = Operator::from_one_byte(11);
    pub const ENDCHAR: Operator = Operator::from_one_byte(14);
    pub const HORIZONTAL_STEM_HINT_MASK: Operator = Operator::from_one_byte(18);
    pub const HINT_MASK: Operator = Operator::from_one_byte(19);
    pub const COUNTER_MASK: Operator = Operator::from_one_byte(20);
    pub const MOVE_TO: Operator = Operator::from_one_byte(21);
    pub const HORIZONTAL_MOVE_TO: Operator = Operator::from_one_byte(22);
    pub const VERTICAL_STEM_HINT_MASK: Operator = Operator::from_one_byte(23);
    pub const CURVE_LINE: Operator = Operator::from_one_byte(24);
    pub const LINE_CURVE: Operator = Operator::from_one_byte(25);
    pub const VV_CURVE_TO: Operator = Operator::from_one_byte(26);
    pub const HH_CURVE_TO: Operator = Operator::from_one_byte(27);
    pub const SHORT_INT: Operator = Operator::from_one_byte(28);
    pub const CALL_GLOBAL_SUBROUTINE: Operator = Operator::from_one_byte(29);
    pub const VH_CURVE_TO: Operator = Operator::from_one_byte(30);
    pub const HV_CURVE_TO: Operator = Operator::from_one_byte(31);
    pub const HFLEX: Operator = Operator::from_one_byte(34);
    pub const FLEX: Operator = Operator::from_one_byte(35);
    pub const HFLEX1: Operator = Operator::from_one_byte(36);
    pub const FLEX1: Operator = Operator::from_one_byte(37);
    pub const FIXED_16_16: Operator = Operator::from_one_byte(255);
}
