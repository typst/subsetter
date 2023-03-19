use std::fmt::{self, Debug, Formatter};
use std::ops::Range;

use crate::{Error, Reader, Result, Structure, Writer};

/// A DICT data structure.
#[derive(Clone)]
pub struct Dict<'a>(Vec<Pair<'a>>);

impl<'a> Dict<'a> {
    pub fn get(&self, op: Op) -> Option<&[Operand<'a>]> {
        self.0
            .iter()
            .find(|pair| pair.op == op)
            .map(|pair| pair.operands.as_slice())
    }

    pub fn get_offset(&self, op: Op) -> Option<usize> {
        match self.get(op)? {
            &[Operand::Int(offset)] if offset > 0 => Some(offset as usize),
            _ => None,
        }
    }

    pub fn get_range(&self, op: Op) -> Option<Range<usize>> {
        match self.get(op)? {
            &[Operand::Int(len), Operand::Int(offset)] if offset > 0 => {
                let offset = usize::try_from(offset).ok()?;
                let len = usize::try_from(len).ok()?;
                Some(offset..offset + len)
            }
            _ => None,
        }
    }

    pub fn retain(&mut self, ops: &[Op]) {
        self.0.retain(|pair| ops.contains(&pair.op));
    }

    pub fn set(&mut self, op: Op, operands: Vec<Operand<'a>>) {
        if let Some(pair) = self.0.iter_mut().find(|pair| pair.op == op) {
            pair.operands = operands;
        } else {
            self.0.push(Pair { operands, op });
        }
    }

    pub fn set_offset(&mut self, op: Op, offset: usize) {
        self.set(op, vec![Operand::Offset(offset)]);
    }

    pub fn set_range(&mut self, op: Op, range: &Range<usize>) {
        self.set(
            op,
            vec![Operand::Offset(range.end - range.start), Operand::Offset(range.start)],
        );
    }
}

impl<'a> Structure<'a> for Dict<'a> {
    fn read(r: &mut Reader<'a>) -> Result<Self> {
        let mut pairs = vec![];
        while !r.eof() {
            pairs.push(r.read::<Pair>()?);
        }
        Ok(Self(pairs))
    }

    fn write(&self, w: &mut Writer) {
        for pair in &self.0 {
            w.write_ref::<Pair>(pair);
        }
    }
}

impl Debug for Dict<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_list().entries(self.0.iter()).finish()
    }
}

/// An operand-operator pair in a DICT.
#[derive(Clone)]
struct Pair<'a> {
    operands: Vec<Operand<'a>>,
    op: Op,
}

impl<'a> Structure<'a> for Pair<'a> {
    fn read(r: &mut Reader<'a>) -> Result<Self> {
        let mut operands = vec![];
        loop {
            match r.data().first().ok_or(Error::MissingData)? {
                0..=21 => break,
                28..=30 | 32..=254 => operands.push(r.read::<Operand>()?),
                _ => r.skip(1)?,
            }
        }
        Ok(Self { operands, op: r.read::<Op>()? })
    }

    fn write(&self, w: &mut Writer) {
        for operand in &self.operands {
            w.write_ref::<Operand>(operand);
        }
        w.write::<Op>(self.op);
    }
}

impl Debug for Pair<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}: {:?}", self.op, self.operands)
    }
}

/// An operator in a DICT.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Op(u8, u8);

impl Structure<'_> for Op {
    fn read(r: &mut Reader) -> Result<Self> {
        let b0 = r.read::<u8>()?;
        match b0 {
            12 => Ok(Self(b0, r.read::<u8>()?)),
            0..=21 => Ok(Self(b0, 0)),
            _ => panic!("cannot read operator here"),
        }
    }

    fn write(&self, w: &mut Writer) {
        w.write::<u8>(self.0);
        if self.0 == 12 {
            w.write::<u8>(self.1);
        }
    }
}

/// An operand in a DICT.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Operand<'a> {
    Int(i32),
    Offset(usize),
    Real(&'a [u8]),
}

impl<'a> Structure<'a> for Operand<'a> {
    fn read(r: &mut Reader<'a>) -> Result<Self> {
        let b0 = i32::from(r.read::<u8>()?);
        Ok(match b0 {
            28 => Self::Int(i32::from(r.read::<i16>()?)),
            29 => Self::Int(r.read::<i32>()?),
            30 => {
                let mut len = 0;
                for &byte in r.data() {
                    len += 1;
                    if byte & 0x0f == 0x0f {
                        break;
                    }
                }
                Self::Real(r.take(len)?)
            }
            32..=246 => Self::Int(b0 - 139),
            247..=250 => {
                let b1 = i32::from(r.read::<u8>()?);
                Self::Int((b0 - 247) * 256 + b1 + 108)
            }
            251..=254 => {
                let b1 = i32::from(r.read::<u8>()?);
                Self::Int(-(b0 - 251) * 256 - b1 - 108)
            }
            _ => panic!("cannot read operand here"),
        })
    }

    fn write(&self, w: &mut Writer) {
        match self {
            Self::Int(int) => {
                // TODO: Select most compact encoding.
                w.write::<u8>(29);
                w.write::<i32>(*int);
            }
            Self::Offset(offset) => {
                w.write::<u8>(29);
                w.write::<i32>(*offset as i32);
            }
            Self::Real(real) => {
                w.write::<u8>(30);
                w.give(&real);
            }
        }
    }
}

/// Top DICT operators.
pub mod top {
    use super::Op;

    pub const KEEP: &[Op] = &[
        ROS,
        CID_FONT_VERSION,
        CID_FONT_REVISION,
        CID_FONT_TYPE,
        CID_COUNT,
        FONT_NAME,
        VERSION,
        NOTICE,
        COPYRIGHT,
        FULL_NAME,
        FAMILY_NAME,
        WEIGHT,
        IS_FIXED_PITCH,
        ITALIC_ANGLE,
        UNDERLINE_POSITION,
        UNDERLINE_THICKNESS,
        PAINT_TYPE,
        CHARSTRING_TYPE,
        FONT_MATRIX,
        FONT_BBOX,
        STROKE_WIDTH,
        POST_SCRIPT,
    ];

    pub const VERSION: Op = Op(0, 0);
    pub const NOTICE: Op = Op(1, 0);
    pub const COPYRIGHT: Op = Op(12, 0);
    pub const FULL_NAME: Op = Op(2, 0);
    pub const FAMILY_NAME: Op = Op(3, 0);
    pub const WEIGHT: Op = Op(4, 0);
    pub const IS_FIXED_PITCH: Op = Op(12, 1);
    pub const ITALIC_ANGLE: Op = Op(12, 2);
    pub const UNDERLINE_POSITION: Op = Op(12, 3);
    pub const UNDERLINE_THICKNESS: Op = Op(12, 4);
    pub const PAINT_TYPE: Op = Op(12, 5);
    pub const CHARSTRING_TYPE: Op = Op(12, 6);
    pub const FONT_MATRIX: Op = Op(12, 7);
    pub const FONT_BBOX: Op = Op(5, 0);
    pub const STROKE_WIDTH: Op = Op(12, 8);
    pub const CHARSET: Op = Op(15, 0);
    pub const ENCODING: Op = Op(16, 0);
    pub const CHAR_STRINGS: Op = Op(17, 0);
    pub const PRIVATE: Op = Op(18, 0);
    pub const POST_SCRIPT: Op = Op(12, 21);

    // CID-keyed fonts.
    pub const ROS: Op = Op(12, 30);
    pub const CID_FONT_VERSION: Op = Op(12, 31);
    pub const CID_FONT_REVISION: Op = Op(12, 32);
    pub const CID_FONT_TYPE: Op = Op(12, 33);
    pub const CID_COUNT: Op = Op(12, 34);
    pub const FD_ARRAY: Op = Op(12, 36);
    pub const FD_SELECT: Op = Op(12, 37);
    pub const FONT_NAME: Op = Op(12, 38);
}

/// Private DICT operators.
pub mod private {
    use super::Op;

    pub const KEEP: &[Op] = &[
        BLUE_VALUES,
        OTHER_BLUES,
        FAMILY_BLUES,
        FAMILY_OTHER_BLUES,
        BLUE_SCALE,
        BLUE_SHIFT,
        BLUE_FUZZ,
        STD_HW,
        STD_VW,
        STEM_SNAP_H,
        STEM_SNAP_V,
        FORCE_BOLD,
        LANGUAGE_GROUP,
        EXPANSION_FACTOR,
        INITIAL_RANDOM_SEED,
        DEFAULT_WIDTH_X,
        NOMINAL_WIDTH_X,
    ];

    pub const BLUE_VALUES: Op = Op(6, 0);
    pub const OTHER_BLUES: Op = Op(7, 0);
    pub const FAMILY_BLUES: Op = Op(8, 0);
    pub const FAMILY_OTHER_BLUES: Op = Op(9, 0);
    pub const BLUE_SCALE: Op = Op(12, 9);
    pub const BLUE_SHIFT: Op = Op(12, 10);
    pub const BLUE_FUZZ: Op = Op(12, 11);
    pub const STD_HW: Op = Op(10, 0);
    pub const STD_VW: Op = Op(11, 0);
    pub const STEM_SNAP_H: Op = Op(12, 12);
    pub const STEM_SNAP_V: Op = Op(12, 13);
    pub const FORCE_BOLD: Op = Op(12, 14);
    pub const LANGUAGE_GROUP: Op = Op(12, 17);
    pub const EXPANSION_FACTOR: Op = Op(12, 18);
    pub const INITIAL_RANDOM_SEED: Op = Op(12, 19);
    pub const SUBRS: Op = Op(19, 0);
    pub const DEFAULT_WIDTH_X: Op = Op(20, 0);
    pub const NOMINAL_WIDTH_X: Op = Op(21, 0);
}
