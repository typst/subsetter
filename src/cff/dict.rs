use crate::cff::types::{Number, StringId};
use crate::stream::Reader;
use std::fmt::Debug;
use std::ops::Range;

const TWO_BYTE_OPERATOR_MARK: u8 = 12;

#[derive(Clone, Copy, Debug)]
pub struct Operator(pub u16);

impl Operator {
    #[inline]
    pub fn get(self) -> u16 {
        self.0
    }
}

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
                let mut operator = u16::from(b);

                // Check that operator is two byte long.
                if b == TWO_BYTE_OPERATOR_MARK {
                    // Use a 1200 'prefix' to make two byte operators more readable.
                    // 12 3 => 1203
                    operator = 1200 + u16::from(r.read::<u8>()?);
                }

                self.offset = r.offset();
                return Some(Operator(operator));
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
pub fn is_dict_one_byte_op(b: u8) -> bool {
    match b {
        0..=27 => true,
        28..=30 => false,  // numbers
        31 => true,        // Reserved
        32..=254 => false, // numbers
        255 => true,       // Reserved
    }
}
