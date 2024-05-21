use crate::cff::cid_font::CIDMetadata;
use crate::cff::dict::operators::*;
use crate::cff::dict::DictionaryParser;
use crate::cff::number::{IntegerNumber, Number};
use crate::cff::remapper::FontDictRemapper;
use crate::cff::sid_font::SIDMetadata;
use crate::cff::FontWriteContext;
use crate::write::Writer;
use crate::Error::SubsetError;
use crate::Result;
use std::array;

#[derive(Default, Clone, Debug)]
pub struct PrivateDict {
    pub subrs: Option<usize>,
}

pub fn parse_subr_offset(data: &[u8]) -> Option<usize> {
    let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
    let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);

    while let Some(operator) = dict_parser.parse_next() {
        if operator == SUBRS {
            return dict_parser.parse_offset();
        }
    }

    None
}

pub fn write_private_dicts(
    fd_remapper: &FontDictRemapper,
    font_write_context: &mut FontWriteContext,
    metadata: &CIDMetadata,
    w: &mut Writer,
) -> Result<()> {
    for (new_df, old_df) in fd_remapper.sorted_iter().enumerate() {
        let font_dict = metadata.font_dicts.get(old_df as usize).ok_or(SubsetError)?;

        let private_dict_offset = w.len();

        let private_dict_data = {
            let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
            let mut dict_parser =
                DictionaryParser::new(font_dict.private_dict, &mut operands_buffer);

            let mut sub_w = Writer::new();

            while let Some(operator) = dict_parser.parse_next() {
                match operator {
                    SUBRS => {
                        // We don't have any subroutines
                    }
                    _ => {
                        dict_parser.parse_operands().unwrap();
                        let operands = dict_parser.operands();

                        sub_w.write(operands);
                        sub_w.write(operator)
                    }
                }
            }

            sub_w.finish()
        };

        let private_dict_len = private_dict_data.len();

        let offsets = font_write_context
            .private_dicts_offsets
            .get_mut(new_df)
            .ok_or(SubsetError)?;
        offsets.0 = IntegerNumber(private_dict_len as i32);
        offsets.1 = IntegerNumber(private_dict_offset as i32);

        w.extend(&private_dict_data);
    }

    Ok(())
}

pub fn write_sid_private_dicts(
    font_write_context: &mut FontWriteContext,
    sid_metadata: &SIDMetadata,
    w: &mut Writer,
) -> Result<()> {
    let private_dict_offset = w.len();

    let private_dict_data = {
        let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
        let mut dict_parser =
            DictionaryParser::new(sid_metadata.private_dict_data, &mut operands_buffer);

        let mut sub_w = Writer::new();

        while let Some(operator) = dict_parser.parse_next() {
            match operator {
                SUBRS => {
                    // We don't have any subroutines
                }
                _ => {
                    dict_parser.parse_operands().unwrap();
                    let operands = dict_parser.operands();

                    sub_w.write(operands);
                    sub_w.write(operator);
                }
            }
        }

        sub_w.finish()
    };

    let private_dict_len = private_dict_data.len();

    let offsets = font_write_context
        .private_dicts_offsets
        .get_mut(0)
        .ok_or(SubsetError)?;
    offsets.0 = IntegerNumber(private_dict_len as i32);
    offsets.1 = IntegerNumber(private_dict_offset as i32);

    w.extend(&private_dict_data);

    Ok(())
}
