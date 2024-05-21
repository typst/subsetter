use crate::read::{Readable, Reader};
use crate::write::{Writeable, Writer};
use std::fmt::{Debug, Formatter};

const FLOAT_STACK_LEN: usize = 64;
const END_OF_FLOAT_FLAG: u8 = 0xf;

#[derive(Clone)]
pub struct RealNumber<'a>(&'a [u8], f32);

impl Debug for RealNumber<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.1)
    }
}

#[derive(Clone)]
pub struct IntegerNumber(pub i32);

impl Debug for IntegerNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy)]
pub struct FixedNumber(i32);

impl Debug for FixedNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FixedNumber {
    pub fn as_f32(&self) -> f32 {
        self.0 as f32 / 65536.0
    }

    pub fn parse(r: &mut Reader<'_>) -> Option<Self> {
        let b0 = r.read::<u8>()?;

        if b0 != 255 {
            return None;
        }

        let num = r.read::<i32>()?;
        Some(FixedNumber(num))
    }
}

impl Writeable for FixedNumber {
    fn write(&self, w: &mut Writer) {
        w.write(255);
        w.write(self.0);
    }
}

impl<'a> RealNumber<'a> {
    pub fn parse(r: &mut Reader<'a>) -> Option<RealNumber<'a>> {
        let mut bytes_reader = r.clone();
        let start = r.offset();

        let mut data = [0u8; FLOAT_STACK_LEN];
        let mut idx = 0;

        let b0 = r.read::<u8>()?;

        if b0 != 30 {
            return None;
        }

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

        Some(RealNumber(bytes_reader.read_bytes(end - start)?, n))
    }
}

impl Writeable for RealNumber<'_> {
    fn write(&self, w: &mut Writer) {
        w.write(self.0);
    }
}

impl IntegerNumber {
    pub fn parse(r: &mut Reader<'_>) -> Option<IntegerNumber> {
        let b0 = r.read::<u8>()?;
        match b0 {
            28 => Some(IntegerNumber(i32::from(r.read::<i16>()?))),
            29 => Some(IntegerNumber(r.read::<i32>()?)),
            32..=246 => {
                let n = i32::from(b0) - 139;
                Some(IntegerNumber(n))
            }
            247..=250 => {
                let b1 = i32::from(r.read::<u8>()?);
                let n = (i32::from(b0) - 247) * 256 + b1 + 108;
                Some(IntegerNumber(n))
            }
            251..=254 => {
                let b1 = i32::from(r.read::<u8>()?);
                let n = -(i32::from(b0) - 251) * 256 - b1 - 108;
                Some(IntegerNumber(n))
            }
            _ => None,
        }
    }

    pub fn write_as_5_bytes(&self, w: &mut Writer) {
        let bytes = self.0.to_be_bytes();
        w.write([29, bytes[0], bytes[1], bytes[2], bytes[3]]);
    }
}

impl Writeable for IntegerNumber {
    fn write(&self, w: &mut Writer) {
        if (-107..=107).contains(&self.0) {
            let b0 = u8::try_from(self.0 + 139).unwrap();
            w.write(b0);
        } else if (108..=1131).contains(&self.0) {
            let temp = self.0 - 108;
            let b0 = u8::try_from(temp / 256 + 247).unwrap();
            let b1 = u8::try_from(temp % 256).unwrap();
            w.write([b0, b1]);
        } else if (-1131..=-108).contains(&self.0) {
            let temp = -self.0 - 108;
            let b0 = u8::try_from(temp / 256 + 251).unwrap();
            let b1 = u8::try_from(temp % 256).unwrap();
            w.write([b0, b1])
        } else if (-32768..=32767).contains(&self.0) {
            let bytes = i16::try_from(self.0).unwrap().to_be_bytes();
            w.write([28, bytes[0], bytes[1]])
        } else {
            self.write_as_5_bytes(w)
        }
    }
}

#[derive(Clone)]
pub enum Number<'a> {
    Real(RealNumber<'a>),
    Integer(IntegerNumber),
    Fixed(FixedNumber),
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

impl Writeable for Number<'_> {
    fn write(&self, w: &mut Writer) {
        match self {
            Number::Real(real_num) => real_num.write(w),
            Number::Integer(int_num) => int_num.write(w),
            Number::Fixed(fixed_num) => fixed_num.write(w),
        }
    }
}

impl<'a> Number<'a> {
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
        Number::Integer(IntegerNumber(num))
    }

    pub fn zero() -> Self {
        Number::Integer(IntegerNumber(0))
    }

    pub fn as_f64(&self) -> f64 {
        match self {
            Number::Integer(int) => int.0 as f64,
            Number::Real(real) => real.1 as f64,
            Number::Fixed(fixed) => fixed.as_f32() as f64,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Number::Integer(int) => Some(int.0),
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
            w.write(&rewritten);
            let second = w.finish();

            assert_eq!(first, second);
        }
    }

    #[test]
    fn size1_roundtrip() {
        let nums = [0, 1, -1, 93, 107, -107];

        for num in nums {
            let integer = IntegerNumber(num);
            let mut w = Writer::new();
            w.write(integer);
            let buffer = w.finish();
            let mut reader = Reader::new(&buffer);

            let reparsed = IntegerNumber::parse(&mut reader).unwrap();
            let mut w = Writer::new();
            w.write(&reparsed);
            let bytes = w.finish();
            assert_eq!(bytes.len(), 1);
            assert_eq!(reparsed.0, num);
        }
    }

    #[test]
    fn size2_roundtrip() {
        let nums = [108, -108, 255, -255, 349, -349, 845, -845, 1131, -1131];

        for num in nums {
            let integer = IntegerNumber(num);
            let mut w = Writer::new();
            w.write(integer);
            let buffer = w.finish();
            let mut reader = Reader::new(&buffer);

            let reparsed = IntegerNumber::parse(&mut reader).unwrap();
            let mut w = Writer::new();
            w.write(&reparsed);
            let bytes = w.finish();
            assert_eq!(bytes.len(), 2);
            assert_eq!(reparsed.0, num);
        }
    }

    #[test]
    fn size3_roundtrip() {
        let nums = [1132, -1132, 2450, -2450, 4096, -4096, 8965, -8965, 32767, -32768];

        for num in nums {
            let integer = IntegerNumber(num);
            let mut w = Writer::new();
            w.write(integer);
            let buffer = w.finish();
            let mut reader = Reader::new(&buffer);

            let reparsed = IntegerNumber::parse(&mut reader).unwrap();
            let mut w = Writer::new();
            w.write(&reparsed);
            let bytes = w.finish();
            assert_eq!(bytes.len(), 3);
            assert_eq!(reparsed.0, num);
        }
    }

    #[test]
    fn size5_roundtrip() {
        let nums = [32768, -32769, i32::MAX, i32::MIN];

        for num in nums {
            let integer = IntegerNumber(num);
            let mut w = Writer::new();
            w.write(integer);
            let buffer = w.finish();
            let mut reader = Reader::new(&buffer);

            let reparsed = IntegerNumber::parse(&mut reader).unwrap();
            let mut w = Writer::new();
            w.write(&reparsed);
            let bytes = w.finish();
            assert_eq!(bytes.len(), 5);
            assert_eq!(reparsed.0, num);
        }
    }

    #[test]
    fn parse_float() {
        let num = [0x1E, 0xE2, 0x49, 0x32, 0xA1, 0x2C, 0x2F];
        let mut r = Reader::new(&num);
        let real = RealNumber::parse(&mut r).unwrap();
        assert_eq!(-249.3212, real.1);
    }
}
