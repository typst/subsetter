use crate::cff::number::Number;
use crate::Error::CFFError;
use crate::Result;

/// The maximum number of operands allowed during parsing.
const MAX_OPERANDS_LEN: usize = 48;

/// An arguments stack for interpreting CFF DICTs and charstrings.
pub struct ArgumentsStack {
    pub data: Vec<Number>,
}

impl ArgumentsStack {
    /// Create a new argument stack.
    pub fn new() -> Self {
        Self { data: vec![] }
    }

    /// The current length of the arguments stack.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Push a new number onto the stack.
    #[inline]
    pub fn push(&mut self, n: Number) -> Result<()> {
        if self.len() == MAX_OPERANDS_LEN {
            Err(CFFError)
        } else {
            self.data.push(n);
            Ok(())
        }
    }

    /// Pop a number from the stack.
    #[inline]
    pub fn pop(&mut self) -> Option<Number> {
        self.data.pop()
    }

    /// Pop all numbers from the stack.
    #[inline]
    pub fn pop_all(&mut self) -> Vec<Number> {
        let mut ret_vec = vec![];
        std::mem::swap(&mut self.data, &mut ret_vec);
        ret_vec
    }
}
