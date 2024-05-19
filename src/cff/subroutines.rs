use crate::cff::charstring::CharString;

pub(crate) struct SubroutineCollection<'a> {
    subroutines: Vec<SubroutineContainer<'a>>,
}

impl<'a> SubroutineCollection<'a> {
    pub fn new(subroutines: Vec<Vec<CharString<'a>>>) -> Self {
        debug_assert!(subroutines.len() <= 255);
        Self {
            subroutines: subroutines
                .into_iter()
                .map(|c| SubroutineContainer::new(c))
                .collect(),
        }
    }

    pub fn get_handler(&self, fd_index: u8) -> Option<SubroutineHandler> {
        self.subroutines.get(fd_index as usize).map(|s| s.get_handler())
    }
}

pub(crate) struct SubroutineContainer<'a> {
    subroutines: Vec<CharString<'a>>,
}

impl<'a> SubroutineContainer<'a> {
    pub fn new(subroutines: Vec<CharString<'a>>) -> Self {
        Self { subroutines }
    }

    pub fn get_handler(&self) -> SubroutineHandler {
        SubroutineHandler::new(self.subroutines.as_ref())
    }
}

#[derive(Clone)]
pub(crate) struct SubroutineHandler<'a> {
    subroutines: &'a [CharString<'a>],
    bias: u16,
}

impl<'a> SubroutineHandler<'a> {
    pub fn new(char_strings: &'a [CharString<'a>]) -> Self {
        Self {
            subroutines: char_strings,
            bias: calc_subroutine_bias(char_strings.len() as u32),
        }
    }

    pub fn get_with_biased(&self, index: i32) -> Option<CharString<'a>> {
        self.get_with_unbiased(unapply_bias(index, self.bias)?)
    }

    pub fn get_with_unbiased(&self, index: u32) -> Option<CharString<'a>> {
        self.subroutines.get(index as usize).copied()
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
