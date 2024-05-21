use crate::cff::number::Number;
use crate::Error::MalformedFont;
use crate::Result;

const MAX_OPERANDS_LEN: usize = 48;

pub struct ArgumentsStack<'a> {
    pub data: Vec<Number<'a>>,
}

impl<'a> ArgumentsStack<'a> {
    pub fn new() -> Self {
        Self { data: vec![] }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn push(&mut self, n: Number<'a>) -> Result<()> {
        if self.len() == MAX_OPERANDS_LEN {
            Err(MalformedFont)
        } else {
            self.data.push(n);
            Ok(())
        }
    }

    #[inline]
    pub fn pop(&mut self) -> Option<Number> {
        self.data.pop()
    }

    #[inline]
    pub fn pop_all(&mut self) -> Vec<Number> {
        let mut ret_vec = vec![];
        std::mem::swap(&mut self.data, &mut ret_vec);
        ret_vec
    }
}
