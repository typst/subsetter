use crate::cff::charstring::SharedCharString;

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
