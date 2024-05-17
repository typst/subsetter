use crate::cff::charstring;
use crate::cff::charstring::{Instruction, Program, SharedCharString};
use crate::cff::index::create_index;
use crate::cff::remapper::SubroutineRemapper;
use crate::cff::types::Number;
use crate::Error::MalformedFont;

pub(crate) struct SubroutineCollection<'a> {
    subroutines: Vec<SubroutineContainer<'a>>,
}

impl<'a> SubroutineCollection<'a> {
    pub fn new(subroutines: Vec<Vec<SharedCharString<'a>>>) -> Self {
        debug_assert!(subroutines.len() <= 255);
        Self {
            subroutines: subroutines
                .into_iter()
                .map(|c| SubroutineContainer::new(c))
                .collect(),
        }
    }

    pub fn get_handler<'b>(&'b self, fd_index: u8) -> Option<SubroutineHandler<'a, 'b>> {
        self.subroutines.get(fd_index as usize).map(|s| s.get_handler())
    }

    pub fn num_entries(&self) -> u8 {
        self.subroutines.len() as u8
    }
}

pub(crate) struct SubroutineContainer<'a> {
    subroutines: Vec<SharedCharString<'a>>,
}

impl<'a> SubroutineContainer<'a> {
    pub fn new(subroutines: Vec<SharedCharString<'a>>) -> Self {
        Self { subroutines }
    }

    pub fn get_handler<'b>(&'b self) -> SubroutineHandler<'a, 'b> {
        SubroutineHandler::new(self.subroutines.as_ref())
    }

    pub fn num_subroutines(&self) -> u32 {
        self.subroutines.len() as u32
    }
}

pub(crate) struct ResolvedSubroutine<'a> {
    pub(crate) char_string: SharedCharString<'a>,
    pub(crate) biased_index: i32,
    pub(crate) unbiased_index: u32,
}

#[derive(Clone)]
pub(crate) struct SubroutineHandler<'a, 'b> {
    subroutines: &'b [SharedCharString<'a>],
    bias: u16,
}

impl<'a, 'b> SubroutineHandler<'a, 'b> {
    pub fn new(char_strings: &'b [SharedCharString<'a>]) -> Self {
        Self {
            subroutines: char_strings,
            bias: calc_subroutine_bias(char_strings.len() as u32),
        }
    }

    pub fn get_with_biased(&self, index: i32) -> Option<ResolvedSubroutine<'a>> {
        self.get_with_unbiased(unapply_bias(index, self.bias)?)
    }

    pub fn get_with_unbiased(&self, index: u32) -> Option<ResolvedSubroutine<'a>> {
        self.subroutines.get(index as usize).and_then(|s| {
            Some(ResolvedSubroutine {
                char_string: s.clone(),
                biased_index: apply_bias(index, self.bias)?,
                unbiased_index: index,
            })
        })
    }

    pub fn get_bias(&self) -> u16 {
        self.bias
    }
}

fn calc_subroutine_bias(len: u32) -> u16 {
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

    u32::try_from(index.checked_add(bias)?).ok()
}

pub fn apply_bias(index: u32, bias: u16) -> Option<i32> {
    let bias = i64::from(bias);
    let index = i64::from(index);

    i32::try_from(index.checked_sub(bias)?).ok()
}

pub(crate) fn write_gsubrs(
    gsubr_remapper: &SubroutineRemapper,
    gsubr_handler: SubroutineHandler,
) -> crate::Result<Vec<u8>> {
    let mut new_gsubrs = vec![];

    for old_subr in gsubr_remapper.sequential_iter() {
        let mut new_program = Program::default();
        let resolved_subroutine =
            gsubr_handler.get_with_unbiased(old_subr).ok_or(MalformedFont)?;
        let char_string = resolved_subroutine.char_string.borrow();
        let program = char_string.program();

        let mut iter = program.instructions().iter().peekable();

        while let Some(instruction) = iter.next() {
            match instruction {
                Instruction::HintMask(mask) => {
                    new_program.push(Instruction::HintMask(*mask))
                }
                Instruction::Operand(num) => {
                    if let Some(Instruction::Operator(op)) = iter.peek() {
                        if *op == charstring::operators::CALL_GLOBAL_SUBROUTINE {
                            let old_gsubr = unapply_bias(
                                num.as_i32().unwrap(),
                                gsubr_handler.get_bias(),
                            )
                            .unwrap();

                            let index = gsubr_remapper.get(old_gsubr).unwrap();

                            let new_gsubr = apply_bias(
                                index,
                                calc_subroutine_bias(gsubr_remapper.len()),
                            )
                            .unwrap();
                            new_program
                                .push(Instruction::Operand(Number::from_i32(new_gsubr)));
                            continue;
                        }
                    }

                    new_program.push(Instruction::Operand(num.clone()))
                }
                // TODO: What if two gsubr/lsubr next to each other>
                Instruction::Operator(op) => new_program.push(Instruction::Operator(*op)),
            }
        }

        new_gsubrs.push(new_program.compile());
    }

    create_index(new_gsubrs)
}
