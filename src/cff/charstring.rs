use crate::cff::argstack::ArgumentsStack;
use crate::cff::charstring::Instruction::{
    DoubleByteOperator, HintMask, SingleByteOperator,
};
use crate::cff::dict::{Number, RealNumber};
use crate::cff::operator;
use crate::stream::{Readable, Reader, Writer};
use crate::Error::MalformedFont;
use crate::{Error, Result};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;

type SharedCharString<'a> = RefCell<CharString<'a>>;

#[derive(Clone, Copy)]
pub struct Fixed<'a>(i32, &'a [u8]);

impl Debug for Fixed<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> Fixed<'a> {
    pub fn as_f32(&self) -> f32 {
        self.0 as f32 / 65536.0
    }

    pub fn as_bytes(&self) -> &'a [u8] {
        self.1
    }
}

impl<'a> Readable<'a> for Fixed<'a> {
    const SIZE: usize = 5;

    fn read(r: &mut Reader<'a>) -> Option<Self> {
        // TODO: Improve
        let bytes = r.read_bytes(5)?;
        let mut r = Reader::new(bytes);
        // Skip 255
        r.read::<u8>();
        let num = r.read::<i32>()?;
        Some(Fixed(num, bytes))
    }
}

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
    SingleByteOperator(u8),
    // Needs to be encoded with 12 in the beginning when serializing.
    DoubleByteOperator(u8),
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
                Instruction::SingleByteOperator(op) => {
                    str_buffer.push(format!("op({})", op));

                    if *op != operator::HINT_MASK && *op != operator::COUNTER_MASK {
                        formatted_strings.push(str_buffer.join(" "));
                        str_buffer.clear();
                    }
                }
                Instruction::DoubleByteOperator(op) => {
                    str_buffer.push(format!("op({})", 1200 + *op as u16));
                    formatted_strings.push(str_buffer.join(" "));
                    str_buffer.clear();
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

    pub fn compile(&self, writer: &mut Writer) {
        for instr in &self.0 {
            match instr {
                Instruction::Operand(op) => {
                    writer.extend(op.as_bytes());
                }
                SingleByteOperator(sbo) => {
                    writer.write(*sbo);
                }
                DoubleByteOperator(dbo) => {
                    writer.write::<u8>(12);
                    writer.write(*dbo);
                }
                HintMask(hm) => {
                    writer.extend(*hm);
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

pub struct CharString<'a> {
    bytecode: &'a [u8],
    pub program: Program<'a>,
    pub used_lsubs: BTreeSet<u32>,
    pub used_gsubs: BTreeSet<u32>,
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
            // access to the whole buffer. This means that for each operator, we need
            // to add another read manually.
            let op = r.peak::<u8>().ok_or(Error::MalformedFont)?;

            match op {
                0 | 2 | 9 | 13 | 15 | 16 | 17 => {
                    // Reserved.
                    return Err(Error::MalformedFont);
                }
                operator::TWO_BYTE_OPERATOR_MARK => {
                    r.read::<u8>();
                    let op2 = r.read::<u8>().ok_or(MalformedFont)?;

                    match op2 {
                        operator::HFLEX
                        | operator::FLEX
                        | operator::HFLEX1
                        | operator::FLEX1 => {
                            decompiler.stack.pop_all();
                            push_instr(DoubleByteOperator(op2));
                        }
                        _ => return Err(MalformedFont),
                    }
                }
                operator::HORIZONTAL_STEM
                | operator::VERTICAL_STEM
                | operator::HORIZONTAL_STEM_HINT_MASK
                | operator::VERTICAL_STEM_HINT_MASK => {
                    r.read::<u8>();
                    decompiler.count_hints();
                    push_instr(SingleByteOperator(op));
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
                | operator::CURVE_TO => {
                    r.read::<u8>();
                    decompiler.stack.pop_all();
                    push_instr(Instruction::SingleByteOperator(op))
                }
                operator::RETURN => {
                    r.read::<u8>();
                    push_instr(Instruction::SingleByteOperator(op))
                }
                28 | 32..=254 => {
                    let number = Number::parse(&mut r).ok_or(MalformedFont)?;
                    decompiler.stack.push(number.clone())?;
                    push_instr(Instruction::Operand(number));
                }
                operator::CALL_GLOBAL_SUBROUTINE => {
                    r.read::<u8>();
                    push_instr(SingleByteOperator(op));
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
                        .ok_or(MalformedFont)?;
                    gsubr.borrow_mut().decompile(decompiler)?;
                    self.used_gsubs.insert(gsubr_index);
                }
                operator::CALL_LOCAL_SUBROUTINE => {
                    r.read::<u8>();
                    push_instr(SingleByteOperator(op));
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
                        .ok_or(MalformedFont)?;
                    lsubr.borrow_mut().decompile(decompiler)?;
                    self.used_lsubs.insert(lsubr_index);
                }
                operator::HINT_MASK | operator::COUNTER_MASK => {
                    r.read::<u8>();
                    push_instr(SingleByteOperator(op));
                    if decompiler.hint_mask_bytes == 0 {
                        decompiler.count_hints();
                        decompiler.hint_mask_bytes = (decompiler.hint_count + 7) / 8;
                        // TODO: Continue
                    }

                    let hint_bytes = r
                        .read_bytes(decompiler.hint_mask_bytes as usize)
                        .ok_or(MalformedFont)?;
                    push_instr(HintMask(hint_bytes));
                }
                operator::ENDCHAR => {
                    // TODO: Add seac
                    r.read::<u8>();
                    push_instr(SingleByteOperator(op));
                }
                operator::FIXED_16_16 => {
                    let num =
                        Number::FixedNumber(r.read::<Fixed>().ok_or(MalformedFont)?);
                    decompiler.stack.push(num.clone())?;
                    push_instr(Instruction::Operand(num));
                }
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

fn conv_subroutine_index(index: i32, bias: u16) -> Option<u32> {
    let bias = i32::from(bias);

    let index = index.checked_add(bias)?;
    u32::try_from(index).ok()
}
