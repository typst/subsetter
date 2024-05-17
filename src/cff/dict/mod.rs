pub(crate) mod private_dict;
pub(crate) mod top_dict;

use crate::cff::operator::{Operator, TWO_BYTE_OPERATOR_MARK};
use crate::cff::types::{Number, StringId};
use crate::read::Reader;
use std::ops::Range;

pub struct DictionaryParser<'a> {
    data: &'a [u8],
    // The current offset.
    offset: usize,
    // Offset to the last operands start.
    operands_offset: usize,
    // Actual operands.
    //
    // While CFF can contain only i32 and f32 values, we have to store operands as f64
    // since f32 cannot represent the whole i32 range.
    // Meaning we have a choice of storing operands as f64 or as enum of i32/f32.
    // In both cases the type size would be 8 bytes, so it's easier to simply use f64.
    operands: &'a mut [Number<'a>],
    // An amount of operands in the `operands` array.
    operands_len: u16,
}

impl<'a> DictionaryParser<'a> {
    #[inline]
    pub fn new(data: &'a [u8], operands_buffer: &'a mut [Number<'a>]) -> Self {
        DictionaryParser {
            data,
            offset: 0,
            operands_offset: 0,
            operands: operands_buffer,
            operands_len: 0,
        }
    }

    #[inline(never)]
    pub fn parse_next(&mut self) -> Option<Operator> {
        let mut r = Reader::new_at(self.data, self.offset);
        self.operands_offset = self.offset;
        while !r.at_end() {
            // 0..=21 bytes are operators.
            if is_dict_one_byte_op(r.peak::<u8>()?) {
                let b = r.read::<u8>()?;
                let mut operator = Operator::from_one_byte(b);

                // Check that operator is two byte long.
                if b == TWO_BYTE_OPERATOR_MARK {
                    // Use a 1200 'prefix' to make two byte operators more readable.
                    // 12 3 => 1203
                    operator = Operator::from_two_byte(r.read::<u8>()?);
                }

                self.offset = r.offset();
                return Some(operator);
            } else {
                let _ = Number::parse_cff_number(&mut r)?;
            }
        }

        None
    }

    /// Parses operands of the current operator.
    ///
    /// In the DICT structure, operands are defined before an operator.
    /// So we are trying to find an operator first and the we can actually parse the operands.
    ///
    /// Since this methods is pretty expensive and we do not care about most of the operators,
    /// we can speed up parsing by parsing operands only for required operators.
    ///
    /// We still have to "skip" operands during operators search (see `skip_number()`),
    /// but it's still faster that a naive method.
    pub fn parse_operands(&mut self) -> Option<()> {
        let mut r = Reader::new_at(self.data, self.operands_offset);
        self.operands_len = 0;
        while !r.at_end() {
            let b = r.peak::<u8>()?;
            // 0..=21 bytes are operators.
            if is_dict_one_byte_op(b) {
                r.read::<u8>()?;
                break;
            } else {
                let op = Number::parse_cff_number(&mut r)?;
                self.operands[usize::from(self.operands_len)] = op;
                self.operands_len += 1;

                if usize::from(self.operands_len) >= self.operands.len() {
                    break;
                }
            }
        }

        Some(())
    }

    #[inline]
    pub fn operands(&self) -> &[Number] {
        &self.operands[..usize::from(self.operands_len)]
    }

    #[inline]
    pub fn parse_number(&mut self) -> Option<Number> {
        self.parse_operands()?;
        self.operands().get(0).cloned()
    }

    #[inline]
    pub fn parse_bool(&mut self) -> Option<bool> {
        self.parse_number().and_then(|n| n.as_i32()).and_then(|n| match n {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        })
    }

    #[inline]
    pub fn parse_sid(&mut self) -> Option<StringId> {
        self.parse_operands()?;
        let operands = self.operands();
        if operands.len() == 1 {
            Some(StringId(u16::try_from(operands[0].as_i32()?).ok()?))
        } else {
            None
        }
    }

    #[inline]
    pub fn parse_offset(&mut self) -> Option<usize> {
        self.parse_operands()?;
        let operands = self.operands();
        if operands.len() == 1 {
            usize::try_from(operands[0].as_u32()?).ok()
        } else {
            None
        }
    }

    #[inline]
    pub fn parse_range(&mut self) -> Option<Range<usize>> {
        self.parse_operands()?;
        let operands = self.operands();
        if operands.len() == 2 {
            let len = usize::try_from(operands[0].as_u32()?).ok()?;
            let start = usize::try_from(operands[1].as_u32()?).ok()?;
            let end = start.checked_add(len)?;
            Some(start..end)
        } else {
            None
        }
    }

    #[inline]
    pub fn parse_delta(&mut self) -> Option<Vec<Number>> {
        self.parse_operands()?;
        Some(self.operands().into())
    }
}

// One-byte CFF DICT Operators according to the
// Adobe Technical Note #5176, Appendix H CFF DICT Encoding.
fn is_dict_one_byte_op(b: u8) -> bool {
    match b {
        0..=27 => true,
        28..=30 => false,  // numbers
        31 => true,        // Reserved
        32..=254 => false, // numbers
        255 => true,       // Reserved
    }
}

#[allow(dead_code)]
// TODO: Use constructor
pub(crate) mod operators {
    use crate::cff::operator::{Operator, OperatorType, TWO_BYTE_OPERATOR_MARK};

    // TOP DICT OPERATORS
    pub const VERSION: Operator = Operator(OperatorType::OneByteOperator([0]));
    pub const NOTICE: Operator = Operator(OperatorType::OneByteOperator([1]));
    pub const COPYRIGHT: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 0]));
    pub const FULL_NAME: Operator = Operator(OperatorType::OneByteOperator([2]));
    pub const FAMILY_NAME: Operator = Operator(OperatorType::OneByteOperator([3]));
    pub const WEIGHT: Operator = Operator(OperatorType::OneByteOperator([4]));
    pub const IS_FIXED_PITCH: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 1]));
    pub const ITALIC_ANGLE: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 2]));
    pub const UNDERLINE_POSITION: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 3]));
    pub const UNDERLINE_THICKNESS: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 4]));
    pub const PAINT_TYPE: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 5]));
    pub const CHAR_STRING_TYPE: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 6]));
    pub const FONT_MATRIX: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 7]));
    pub const UNIQUE_ID: Operator = Operator(OperatorType::OneByteOperator([13]));
    pub const FONT_BBOX: Operator = Operator(OperatorType::OneByteOperator([5]));
    pub const STROKE_WIDTH: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 8]));
    pub const XUID: Operator = Operator(OperatorType::OneByteOperator([14]));
    pub const CHARSET: Operator = Operator(OperatorType::OneByteOperator([15]));
    pub const ENCODING: Operator = Operator(OperatorType::OneByteOperator([16]));
    pub const CHAR_STRINGS: Operator = Operator(OperatorType::OneByteOperator([17]));
    pub const PRIVATE: Operator = Operator(OperatorType::OneByteOperator([18]));
    pub const SYNTHETIC_BASE: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 20]));
    pub const POSTSCRIPT: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 21]));
    pub const BASE_FONT_NAME: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 22]));
    pub const BASE_FONT_BLEND: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 23]));
    // TOP DICT OPERATORS (CID FONTS)
    pub const ROS: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 30]));
    pub const CID_FONT_VERSION: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 31]));
    pub const CID_FONT_REVISION: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 32]));
    pub const CID_FONT_TYPE: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 33]));
    pub const CID_COUNT: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 34]));
    pub const UID_BASE: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 35]));
    pub const FD_ARRAY: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 36]));
    pub const FD_SELECT: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 37]));
    pub const FONT_NAME: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 38]));

    // PRIVATE DICT OPERATORS
    pub const BLUE_VALUES: Operator = Operator(OperatorType::OneByteOperator([6]));
    pub const OTHER_BLUES: Operator = Operator(OperatorType::OneByteOperator([7]));
    pub const FAMILY_BLUES: Operator = Operator(OperatorType::OneByteOperator([8]));
    pub const FAMILY_OTHER_BLUES: Operator = Operator(OperatorType::OneByteOperator([9]));
    pub const BLUE_SCALE: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 9]));
    pub const BLUE_SHIFT: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 10]));
    pub const BLUE_FUZZ: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 11]));
    pub const STD_HW: Operator = Operator(OperatorType::OneByteOperator([10]));
    pub const STD_VW: Operator = Operator(OperatorType::OneByteOperator([11]));
    pub const STEM_SNAP_H: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 12]));
    pub const STEM_SNAP_V: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 13]));
    pub const FORCE_BOLD: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 14]));
    pub const LANGUAGE_GROUP: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 17]));
    pub const EXPANSION_FACTOR: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 18]));
    pub const INITIAL_RANDOM_SEED: Operator =
        Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 19]));
    pub const SUBRS: Operator = Operator(OperatorType::OneByteOperator([19]));
    pub const DEFAULT_WIDTH_X: Operator = Operator(OperatorType::OneByteOperator([20]));
    pub const NOMINAL_WIDTH_X: Operator = Operator(OperatorType::OneByteOperator([21]));
}
