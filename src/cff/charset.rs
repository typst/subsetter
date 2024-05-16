use crate::stream::{Readable, Reader, StringId};
use crate::util::LazyArray16;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Charset<'a> {
    ISOAdobe,
    Expert,
    ExpertSubset,
    Format0(LazyArray16<'a, StringId>),
    Format1(LazyArray16<'a, Format1Range>),
    Format2(LazyArray16<'a, Format2Range>),
}

impl Charset<'_> {
    // NOTE: Unlike the ttf-parser equivalent, this only returns SIDs for
    // custom charsets
    pub fn gid_to_sid(&self, gid: u16) -> Option<StringId> {
        match self {
            Charset::ISOAdobe => None,
            Charset::Expert => None,
            Charset::ExpertSubset => None,
            Charset::Format0(ref array) => {
                if gid == 0 {
                    Some(StringId(0))
                } else {
                    array.get(gid - 1)
                }
            }
            Charset::Format1(array) => {
                if gid == 0 {
                    Some(StringId(0))
                } else {
                    let mut sid = gid - 1;
                    for range in *array {
                        if sid <= u16::from(range.left) {
                            sid = sid.checked_add(range.first.0)?;
                            return Some(StringId(sid));
                        }

                        sid = sid.checked_sub(u16::from(range.left) + 1)?;
                    }

                    None
                }
            }
            Charset::Format2(array) => {
                if gid == 0 {
                    Some(StringId(0))
                } else {
                    let mut sid = gid - 1;
                    for range in *array {
                        if sid <= range.left {
                            sid = sid.checked_add(range.first.0)?;
                            return Some(StringId(sid));
                        }

                        sid = sid.checked_sub(range.left.checked_add(1)?)?;
                    }

                    None
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Format1Range {
    first: StringId,
    left: u8,
}

impl Readable<'_> for Format1Range {
    const SIZE: usize = 3;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Format1Range {
            first: r.read::<StringId>()?,
            left: r.read::<u8>()?,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Format2Range {
    first: StringId,
    left: u16,
}

impl Readable<'_> for Format2Range {
    const SIZE: usize = 4;

    fn read(r: &mut Reader<'_>) -> Option<Self> {
        Some(Format2Range {
            first: r.read::<StringId>()?,
            left: r.read::<u16>()?,
        })
    }
}

pub(crate) fn parse_charset<'a>(
    number_of_glyphs: u16,
    r: &mut Reader<'a>,
) -> Option<Charset<'a>> {
    if number_of_glyphs < 2 {
        return None;
    }

    // -1 everywhere, since `.notdef` is omitted.
    let format = r.read::<u8>()?;
    match format {
        0 => Some(Charset::Format0(r.read_array16::<StringId>(number_of_glyphs - 1)?)),
        1 => {
            // The number of ranges is not defined, so we have to
            // read until no glyphs are left.
            let mut count = 0;
            {
                let mut s = r.clone();
                let mut total_left = number_of_glyphs - 1;
                while total_left > 0 {
                    s.skip::<StringId>(); // first
                    let left = s.read::<u8>()?;
                    total_left = total_left.checked_sub(u16::from(left) + 1)?;
                    count += 1;
                }
            }

            r.read_array16::<Format1Range>(count).map(Charset::Format1)
        }
        2 => {
            // The same as format 1, but Range::left is u16.
            let mut count = 0;
            {
                let mut s = r.clone();
                let mut total_left = number_of_glyphs - 1;
                while total_left > 0 {
                    let first = s.read::<StringId>(); // first
                    let left = s.read::<u16>()?.checked_add(1)?;
                    total_left = total_left.checked_sub(left)?;
                    count += 1;
                }
            }

            r.read_array16::<Format2Range>(count).map(Charset::Format2)
        }
        _ => None,
    }
}
