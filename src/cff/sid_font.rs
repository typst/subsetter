use crate::cff::dict::private_dict::parse_subr_offset;
use crate::cff::dict::top_dict::TopDictData;
use crate::cff::index::{parse_index, Index};
use crate::read::Reader;

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct SIDMetadata<'a> {
    pub(crate) local_subrs: Index<'a>,
    pub(crate) private_dict_data: &'a [u8],
}

pub(crate) fn parse_sid_metadata<'a>(
    data: &'a [u8],
    top_dict: &TopDictData,
) -> SIDMetadata<'a> {
    top_dict
        .private
        .clone()
        .and_then(|private_dict_range| {
            let mut metadata = SIDMetadata::default();
            let private_dict_data = data.get(private_dict_range.clone())?;

            let subrs_offset = parse_subr_offset(private_dict_data)?;
            let start = private_dict_range.start.checked_add(subrs_offset)?;
            let subrs_data = data.get(start..)?;
            let mut r = Reader::new(subrs_data);

            metadata.local_subrs = parse_index::<u16>(&mut r)?;
            metadata.private_dict_data = private_dict_data;
            Some(metadata)
        })
        .unwrap_or_default()
}
