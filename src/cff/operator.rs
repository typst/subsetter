use std::fmt::{Display, Formatter};

pub const TWO_BYTE_OPERATOR_MARK: u8 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatorType {
    OneByteOperator([u8; 1]),
    TwoByteOperator([u8; 2]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operator(pub OperatorType);

impl Display for Operator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            OperatorType::OneByteOperator(b) => write!(f, "{}", b[0]),
            OperatorType::TwoByteOperator(b) => write!(f, "{}{}", b[0], b[1]),
        }
    }
}

impl Operator {
    pub const fn from_one_byte(b: u8) -> Self {
        Self(OperatorType::OneByteOperator([b]))
    }

    pub fn from_two_byte(b: u8) -> Self {
        Self(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, b]))
    }

    pub fn as_bytes(&self) -> &[u8] {
        match &self.0 {
            OperatorType::OneByteOperator(b) => b,
            OperatorType::TwoByteOperator(b) => b,
        }
    }
}
