use crate::read::{Readable, Reader};
use crate::write::{Writeable, Writer};
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};

const FLOAT_STACK_LEN: usize = 64;
const END_OF_FLOAT_FLAG: u8 = 0xf;

#[derive(Clone)]
pub struct RealNumber<'a>(Cow<'a, [u8]>, f32);

impl Debug for RealNumber<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.1)
    }
}

#[derive(Clone)]
pub struct IntegerNumber<'a>(Cow<'a, [u8]>, i32);

impl Debug for IntegerNumber<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.1)
    }
}

#[derive(Clone, Copy)]
pub struct FixedNumber<'a>(i32, &'a [u8]);

impl Debug for FixedNumber<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> FixedNumber<'a> {
    pub fn as_f32(&self) -> f32 {
        self.0 as f32 / 65536.0
    }

    pub fn parse(r: &mut Reader<'a>) -> Option<Self> {
        let mut byte_reader = r.clone();
        let b0 = r.read::<u8>()?;

        if b0 == 255 {
            let num = r.read::<i32>()?;
            return Some(FixedNumber(num, byte_reader.read_bytes(5)?));
        }

        None
    }

    pub fn as_bytes(&self) -> &'a [u8] {
        self.1
    }
}

impl<'a> RealNumber<'a> {
    pub fn parse(r: &mut Reader<'a>) -> Option<RealNumber<'a>> {
        let mut bytes_reader = r.clone();
        let start = r.offset();

        let mut data = [0u8; FLOAT_STACK_LEN];
        let mut idx = 0;

        // Skip the prefix
        r.read::<u8>()?;

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
        let end = r.offset();

        Some(RealNumber(Cow::Borrowed(bytes_reader.read_bytes(end - start).unwrap()), n))
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

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
                r.read::<i32>()?,
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
        if (-107..=107).contains(&num) {
            let b0 = u8::try_from(num + 139).unwrap();
            Self(Cow::Owned(vec![b0]), num)
        } else if (108..=1131).contains(&num) {
            let temp = num - 108;
            let b0 = u8::try_from(temp / 256 + 247).unwrap();
            let b1 = u8::try_from(temp % 256).unwrap();
            Self(Cow::Owned(vec![b0, b1]), num)
        } else if (-1131..=-108).contains(&num) {
            let temp = -num - 108;
            let b0 = u8::try_from(temp / 256 + 251).unwrap();
            let b1 = u8::try_from(temp % 256).unwrap();
            Self(Cow::Owned(vec![b0, b1]), num)
        } else if (-32768..=32767).contains(&num) {
            let bytes = i16::try_from(num).unwrap().to_be_bytes();
            Self(Cow::Owned(vec![28, bytes[0], bytes[1]]), num)
        } else {
            IntegerNumber::from_i32_as_int5(num)
        }
    }

    pub fn from_i32_as_int5(num: i32) -> Self {
        let bytes = num.to_be_bytes();
        Self(Cow::Owned(vec![29, bytes[0], bytes[1], bytes[2], bytes[3]]), num)
    }
}

#[derive(Clone)]
pub enum Number<'a> {
    Real(RealNumber<'a>),
    Integer(IntegerNumber<'a>),
    Fixed(FixedNumber<'a>),
}

impl Default for Number<'_> {
    fn default() -> Self {
        Number::zero()
    }
}

impl Debug for Number<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_f64())
    }
}

impl<'a> Number<'a> {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Number::Real(real_num) => real_num.as_bytes(),
            Number::Integer(int_num) => int_num.as_bytes(),
            Number::Fixed(fixed_num) => fixed_num.as_bytes(),
        }
    }

    pub fn parse_cff_number(r: &mut Reader<'a>) -> Option<Number<'a>> {
        Self::parse_number(r, false)
    }

    pub fn parse_charstring_number(r: &mut Reader<'a>) -> Option<Number<'a>> {
        Self::parse_number(r, true)
    }

    fn parse_number(r: &mut Reader<'a>, charstring_num: bool) -> Option<Number<'a>> {
        match r.peak::<u8>()? {
            30 => Some(Number::Real(RealNumber::parse(r)?)),
            255 => {
                if charstring_num {
                    return Some(Number::Fixed(FixedNumber::parse(r)?));
                }

                None
            }
            _ => Some(Number::Integer(IntegerNumber::parse(r)?)),
        }
    }

    pub fn from_i32(num: i32) -> Self {
        Number::Integer(IntegerNumber::from_i32(num))
    }

    pub fn zero() -> Self {
        Number::Integer(IntegerNumber::from_i32(0))
    }

    pub fn as_f64(&self) -> f64 {
        match self {
            Number::Integer(int) => int.as_i32() as f64,
            Number::Real(real) => real.1 as f64,
            Number::Fixed(fixed) => fixed.as_f32() as f64,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Number::Integer(int) => Some(int.as_i32()),
            Number::Real(rn) => {
                if rn.1.fract() == 0.0 {
                    Some(rn.1 as i32)
                } else {
                    None
                }
            }
            Number::Fixed(fixn) => {
                let num = fixn.as_f32();
                if num.fract() == 0.0 {
                    Some(num as i32)
                } else {
                    None
                }
            }
        }
    }

    pub fn as_u32(&self) -> Option<u32> {
        u32::try_from(self.as_i32()?).ok()
    }
}

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

/// A type-safe wrapper for string ID.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Debug, Hash, Ord)]
pub struct StringId(pub u16);

impl StringId {
    pub const STANDARD_STRING_LEN: u16 = 391;

    pub fn is_standard_string(&self) -> bool {
        self.0 < Self::STANDARD_STRING_LEN
    }
}

impl Readable<'_> for StringId {
    const SIZE: usize = u16::SIZE;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Self(r.read::<u16>()?))
    }
}

impl Writeable for StringId {
    fn write(&self, w: &mut Writer) {
        w.write::<u16>(self.0)
    }
}

impl From<u16> for StringId {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct U24(pub u32);

impl U24 {
    pub const MAX: u32 = 16777215;
}

impl Readable<'_> for U24 {
    const SIZE: usize = 3;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        let data = r.read::<[u8; 3]>()?;
        Some(U24(u32::from_be_bytes([0, data[0], data[1], data[2]])))
    }
}

impl Writeable for U24 {
    fn write(&self, w: &mut Writer) {
        let data = self.0.to_be_bytes();
        w.write::<[u8; 3]>([data[1], data[2], data[3]]);
    }
}

#[cfg(test)]
mod tests {
    use crate::cff::number::*;
    use crate::read::Reader;

    #[test]
    fn u24() {
        let nums = [0u32, 45, 345, 54045, 32849324, 16777215];

        for num in nums {
            let wrapped = U24(num);

            let mut w = Writer::new();
            w.write(wrapped);
            let first = w.finish();

            let mut r = Reader::new(&first);
            let rewritten = r.read::<U24>().unwrap();
            let mut w = Writer::new();
            w.write(rewritten);
            let second = w.finish();

            assert_eq!(first, second);
        }
    }

    #[test]
    fn size1_roundtrip() {
        let nums = [0, 1, -1, 93, 107, -107];

        for num in nums {
            let integer = IntegerNumber::from_i32(num);
            let bytes = integer.as_bytes();
            let reader = Reader::new(bytes);

            let reparsed = IntegerNumber::parse(&mut reader.clone()).unwrap();
            assert_eq!(reparsed.as_bytes().len(), 1);
            assert_eq!(reparsed.as_i32(), num);
        }

        for num in nums {
            let integer = Number::from_i32(num);
            let bytes = integer.as_bytes();
            let reader = Reader::new(bytes);

            let reparsed = Number::parse_cff_number(&mut reader.clone()).unwrap();
            assert_eq!(reparsed.as_bytes().len(), 1);
            assert_eq!(reparsed.as_i32(), Some(num));
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

        for num in nums {
            let integer = Number::from_i32(num);
            let bytes = integer.as_bytes();
            let mut reader = Reader::new(bytes);
            let reparsed = Number::parse_cff_number(&mut reader).unwrap();
            assert_eq!(reparsed.as_bytes().len(), 2);
            assert_eq!(reparsed.as_i32(), Some(num));
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

        for num in nums {
            let integer = Number::from_i32(num);
            let bytes = integer.as_bytes();
            let mut reader = Reader::new(bytes);
            let reparsed = Number::parse_cff_number(&mut reader).unwrap();
            assert_eq!(reparsed.as_bytes().len(), 3);
            assert_eq!(reparsed.as_i32(), Some(num));
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

        for num in nums {
            let integer = Number::from_i32(num);
            let bytes = integer.as_bytes();
            let mut reader = Reader::new(bytes);
            let reparsed = Number::parse_cff_number(&mut reader).unwrap();
            assert_eq!(reparsed.as_bytes().len(), 5);
            assert_eq!(reparsed.as_i32(), Some(num));
        }
    }

    #[test]
    fn parse_float() {
        let num = [0x1E, 0xE2, 0x49, 0x32, 0xA1, 0x2C, 0x2F];
        let mut r = Reader::new(&num);
        let real = RealNumber::parse(&mut r).unwrap();
        assert_eq!(-249.3212, real.1);
    }

    // TODO: Add fixed number tests
}
