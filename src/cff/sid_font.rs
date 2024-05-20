use crate::cff::dict::private_dict::parse_subr_offset;
use crate::cff::dict::top_dict::TopDictData;
use crate::cff::encoding::Encoding;
use crate::cff::index::{Index, parse_index};
use crate::read::Reader;

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct SIDMetadata<'a> {
    pub(crate) local_subrs: Index<'a>,
    pub(crate) encoding: Encoding<'a>,
}

fn parse_sid_metadata<'a>(
    data: &'a [u8],
    top_dict: TopDictData,
    encoding: Encoding<'a>,
) -> Option<SIDMetadata<'a>> {
    let mut metadata = SIDMetadata::default();
    metadata.encoding = encoding;

    if let Some(private_dict_range) = top_dict.private.clone() {
        let private_dict_data = data.get(private_dict_range.clone())?;

        let subrs_offset = parse_subr_offset(private_dict_data)?;
        let start = private_dict_range.start.checked_add(subrs_offset)?;
        let subrs_data = data.get(start..)?;
        let mut r = Reader::new(subrs_data);

        metadata.local_subrs = parse_index::<u16>(&mut r)?;
    }

    Some(metadata)
}