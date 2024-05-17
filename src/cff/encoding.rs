use crate::cff::types::StringId;
use crate::stream::{Readable, Reader};
use crate::util::LazyArray16;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Format1Range {
    first: u8,
    left: u8,
}

impl Readable<'_> for Format1Range {
    const SIZE: usize = 2;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Format1Range { first: r.read::<u8>()?, left: r.read::<u8>()? })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Supplement {
    code: u8,
    name: StringId,
}

impl Readable<'_> for Supplement {
    const SIZE: usize = 3;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Supplement { code: r.read::<u8>()?, name: r.read::<StringId>()? })
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Encoding<'a> {
    kind: EncodingKind<'a>,
    supplemental: LazyArray16<'a, Supplement>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum EncodingKind<'a> {
    Standard,
    Expert,
    Format0(LazyArray16<'a, u8>),
    Format1(LazyArray16<'a, Format1Range>),
}

impl Default for EncodingKind<'_> {
    fn default() -> Self {
        Self::Standard
    }
}

pub(crate) fn parse_encoding<'a>(r: &mut Reader<'a>) -> Option<Encoding<'a>> {
    let format = r.read::<u8>()?;
    // The first high-bit in format indicates that a Supplemental encoding is present.
    // Check it and clear.
    let has_supplemental = format & 0x80 != 0;
    let format = format & 0x7f;

    let count = u16::from(r.read::<u8>()?);
    let kind = match format {
        // TODO: read_array8?
        0 => r.read_array16::<u8>(count).map(EncodingKind::Format0)?,
        1 => r.read_array16::<Format1Range>(count).map(EncodingKind::Format1)?,
        _ => return None,
    };

    let supplemental = if has_supplemental {
        let count = u16::from(r.read::<u8>()?);
        r.read_array16::<Supplement>(count)?
    } else {
        LazyArray16::default()
    };

    Some(Encoding { kind, supplemental })
}
