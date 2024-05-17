pub const TWO_BYTE_OPERATOR_MARK: u8 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatorType {
    OneByteOperator([u8; 1]),
    TwoByteOperator([u8; 2]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operator(pub OperatorType);

impl Operator {
    pub fn from_one_byte(b: u8) -> Self {
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
