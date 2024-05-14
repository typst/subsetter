use crate::stream::{Reader, StringId};
use std::borrow::Cow;
use std::ops::Range;

// Limits according to the Adobe Technical Note #5176, chapter 4 DICT Data.
const TWO_BYTE_OPERATOR_MARK: u8 = 12;
const FLOAT_STACK_LEN: usize = 64;
const END_OF_FLOAT_FLAG: u8 = 0xf;

/// Represents a real number. The underlying buffer is guaranteed to be a valid number.
pub struct RealNumber<'a>(Cow<'a, [u8]>);
/// Represents an integer number. The underlying buffer is guaranteed to be a valid number.
pub struct IntegerNumber<'a>(Cow<'a, [u8]>, i32);

impl<'a> IntegerNumber<'a> {
    pub fn parse(r: &mut Reader<'a>) -> Option<IntegerNumber<'a>> {
        let mut byte_reader = r.clone();
        let b0 = r.read::<u8>()?;
        match b0 {
            28 => Some(IntegerNumber(
                Cow::Borrowed(byte_reader.read_bytes(3)?),
                i32::from(r.read::<i16>()?),
            )),
            29 => Some(IntegerNumber(
                Cow::Borrowed(byte_reader.read_bytes(5)?),
                i32::from(r.read::<i32>()?),
            )),
            32..=246 => {
                let n = i32::from(b0) - 139;
                Some(IntegerNumber(Cow::Borrowed(byte_reader.read_bytes(1)?), n))
            }
            247..=250 => {
                let b1 = i32::from(r.read::<u8>()?);
                let n = (i32::from(b0) - 247) * 256 + b1 + 108;
                Some(IntegerNumber(Cow::Borrowed(byte_reader.read_bytes(2)?), n))
            }
            251..=254 => {
                let b1 = i32::from(r.read::<u8>()?);
                let n = -(i32::from(b0) - 251) * 256 - b1 - 108;
                Some(IntegerNumber(Cow::Borrowed(byte_reader.read_bytes(2)?), n))
            }
            _ => None,
        }
    }

    pub fn as_i32(&self) -> i32 {
        self.1
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }

    pub fn from_i32(num: i32) -> Self {
        if num >= -107 && num <= 107 {
            let b0 = u8::try_from(num + 139).unwrap();
            Self(Cow::Owned(vec![b0]), num)
        } else if num >= 108 && num <= 1131 {
            let temp = num - 108;
            let b0 = u8::try_from(temp / 256 + 247).unwrap();
            let b1 = u8::try_from(temp % 256).unwrap();
            Self(Cow::Owned(vec![b0, b1]), num)
        } else if num >= -1131 && num <= -108 {
            let temp = -num - 108;
            let b0 = u8::try_from(temp / 256 + 251).unwrap();
            let b1 = u8::try_from(temp % 256).unwrap();
            Self(Cow::Owned(vec![b0, b1]), num)
        } else if num >= -32768 && num <= 32767 {
            let bytes = i16::try_from(num).unwrap().to_be_bytes();
            Self(Cow::Owned(vec![28, bytes[0], bytes[1]]), num)
        } else {
            let bytes = num.to_be_bytes();
            Self(Cow::Owned(vec![29, bytes[0], bytes[1], bytes[2], bytes[3]]), num)
        }
    }
}

mod tests {
    use crate::cff::dict::IntegerNumber;
    use crate::stream::Reader;

    #[test]
    fn size1_roundtrip() {
        let nums = [0, 1, -1, 93, 107, -107];

        for num in nums {
            let integer = IntegerNumber::from_i32(num);
            let bytes = integer.as_bytes();
            let mut reader = Reader::new(bytes);
            let reparsed = IntegerNumber::parse(&mut reader).unwrap();
            assert_eq!(reparsed.as_bytes().len(), 1);
            assert_eq!(reparsed.as_i32(), num);
        }
    }

    #[test]
    fn size2_roundtrip() {
        let nums = [108, -108, 255, -255, 349, -349, 845, -845, 1131, -1131];

        for num in nums {
            let integer = IntegerNumber::from_i32(num);
            let bytes = integer.as_bytes();
            let mut reader = Reader::new(bytes);
            let reparsed = IntegerNumber::parse(&mut reader).unwrap();
            assert_eq!(reparsed.as_bytes().len(), 2);
            assert_eq!(reparsed.as_i32(), num);
        }
    }

    #[test]
    fn size3_roundtrip() {
        let nums = [1132, -1132, 2450, -2450, 4096, -4096, 8965, -8965, 32767, -32768];

        for num in nums {
            let integer = IntegerNumber::from_i32(num);
            let bytes = integer.as_bytes();
            let mut reader = Reader::new(bytes);
            let reparsed = IntegerNumber::parse(&mut reader).unwrap();
            assert_eq!(reparsed.as_bytes().len(), 3);
            assert_eq!(reparsed.as_i32(), num);
        }
    }

    #[test]
    fn size5_roundtrip() {
        let nums = [32768, -32769, i32::MAX, i32::MIN];

        for num in nums {
            let integer = IntegerNumber::from_i32(num);
            let bytes = integer.as_bytes();
            let mut reader = Reader::new(bytes);
            let reparsed = IntegerNumber::parse(&mut reader).unwrap();
            assert_eq!(reparsed.as_bytes().len(), 5);
            assert_eq!(reparsed.as_i32(), num);
        }
    }
}

pub enum Number<'a> {
    RealNumber(RealNumber<'a>),
    IntegerNumber(IntegerNumber<'a>),
}

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
    operands: &'a mut [f64],
    // An amount of operands in the `operands` array.
    operands_len: u16,
}

impl<'a> DictionaryParser<'a> {
    #[inline]
    pub fn new(data: &'a [u8], operands_buffer: &'a mut [f64]) -> Self {
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
            let b = r.read::<u8>()?;
            // 0..=21 bytes are operators.
            if is_dict_one_byte_op(b) {
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
                skip_number(b, &mut r)?;
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
            let b = r.read::<u8>()?;
            // 0..=21 bytes are operators.
            if is_dict_one_byte_op(b) {
                break;
            } else {
                let op = parse_number(b, &mut r)?;
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
    pub fn operands(&self) -> &[f64] {
        &self.operands[..usize::from(self.operands_len)]
    }

    #[inline]
    pub fn parse_number(&mut self) -> Option<f64> {
        self.parse_operands()?;
        self.operands().get(0).cloned()
    }

    #[inline]
    pub fn parse_bool(&mut self) -> Option<bool> {
        self.parse_number().map(|n| n as u64).and_then(|n| match n {
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
            Some(StringId(u16::try_from(operands[0] as i64).ok()?))
        } else {
            None
        }
    }

    #[inline]
    pub fn parse_offset(&mut self) -> Option<usize> {
        self.parse_operands()?;
        let operands = self.operands();
        if operands.len() == 1 {
            usize::try_from(operands[0] as i32).ok()
        } else {
            None
        }
    }

    #[inline]
    pub fn parse_range(&mut self) -> Option<Range<usize>> {
        self.parse_operands()?;
        let operands = self.operands();
        if operands.len() == 2 {
            let len = usize::try_from(operands[0] as i32).ok()?;
            let start = usize::try_from(operands[1] as i32).ok()?;
            let end = start.checked_add(len)?;
            Some(start..end)
        } else {
            None
        }
    }

    #[inline]
    pub fn parse_delta(&mut self) -> Option<Vec<f64>> {
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

// Adobe Technical Note #5177, Table 3 Operand Encoding
pub fn parse_number(b0: u8, r: &mut Reader) -> Option<f64> {
    match b0 {
        28 => {
            let n = i32::from(r.read::<i16>()?);
            Some(f64::from(n))
        }
        29 => {
            let n = r.read::<i32>()?;
            Some(f64::from(n))
        }
        30 => parse_float(r),
        32..=246 => {
            let n = i32::from(b0) - 139;
            Some(f64::from(n))
        }
        247..=250 => {
            let b1 = i32::from(r.read::<u8>()?);
            let n = (i32::from(b0) - 247) * 256 + b1 + 108;
            Some(f64::from(n))
        }
        251..=254 => {
            let b1 = i32::from(r.read::<u8>()?);
            let n = -(i32::from(b0) - 251) * 256 - b1 - 108;
            Some(f64::from(n))
        }
        _ => None,
    }
}

fn parse_float(r: &mut Reader) -> Option<f64> {
    let mut data = [0u8; FLOAT_STACK_LEN];
    let mut idx = 0;

    loop {
        let b1: u8 = r.read()?;
        let nibble1 = b1 >> 4;
        let nibble2 = b1 & 15;

        if nibble1 == END_OF_FLOAT_FLAG {
            break;
        }

        idx = parse_float_nibble(nibble1, idx, &mut data)?;

        if nibble2 == END_OF_FLOAT_FLAG {
            break;
        }

        idx = parse_float_nibble(nibble2, idx, &mut data)?;
    }

    let s = core::str::from_utf8(&data[..idx]).ok()?;
    let n = s.parse().ok()?;
    Some(n)
}

// Adobe Technical Note #5176, Table 5 Nibble Definitions
fn parse_float_nibble(nibble: u8, mut idx: usize, data: &mut [u8]) -> Option<usize> {
    if idx == FLOAT_STACK_LEN {
        return None;
    }

    match nibble {
        0..=9 => {
            data[idx] = b'0' + nibble;
        }
        10 => {
            data[idx] = b'.';
        }
        11 => {
            data[idx] = b'E';
        }
        12 => {
            if idx + 1 == FLOAT_STACK_LEN {
                return None;
            }

            data[idx] = b'E';
            idx += 1;
            data[idx] = b'-';
        }
        13 => {
            return None;
        }
        14 => {
            data[idx] = b'-';
        }
        _ => {
            return None;
        }
    }

    idx += 1;
    Some(idx)
}

// Just like `parse_number`, but doesn't actually parses the data.
pub fn skip_number(b0: u8, r: &mut Reader) -> Option<()> {
    match b0 {
        28 => r.skip::<u16>(),
        29 => r.skip::<u32>(),
        30 => {
            while !r.at_end() {
                let b1 = r.read::<u8>()?;
                let nibble1 = b1 >> 4;
                let nibble2 = b1 & 15;
                if nibble1 == END_OF_FLOAT_FLAG || nibble2 == END_OF_FLOAT_FLAG {
                    break;
                }
            }
        }
        32..=246 => {}
        247..=250 => r.skip::<u8>(),
        251..=254 => r.skip::<u8>(),
        _ => return None,
    }

    Some(())
}
