use crate::cff::cid_font::CIDMetadata;
use crate::cff::dict::operators::*;
use crate::cff::dict::DictionaryParser;
use crate::cff::remapper::{FontDictRemapper, SidRemapper};
use crate::cff::types::{IntegerNumber, Number};
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
        match operator {
            SUBRS => {
                return Some(dict_parser.parse_offset()?);
            }
            _ => {}
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
    for (new_df, old_df) in fd_remapper.sequential_iter().enumerate() {
        let font_dict = metadata.font_dicts.get(old_df as usize).ok_or(SubsetError)?;

        let offsets = font_write_context
            .private_dicts_offsets
            .get_mut(new_df)
            .ok_or(SubsetError)?;

        let private_dict_data = {
            let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
            let mut dict_parser =
                DictionaryParser::new(font_dict.private_dict, &mut operands_buffer);

            let mut sub_w = Writer::new();

            let mut write = |operands: &[u8], operator: &[u8]| {
                for operand in operands {
                    sub_w.write(*operand);
                }
                sub_w.write(operator);
            };

            while let Some(operator) = dict_parser.parse_next() {
                match operator {
                    SUBRS => {
                        let mut w = Writer::new();
                        w.write(offsets.0.as_bytes());
                        w.write(offsets.1.as_bytes());

                        write(&w.finish(), SUBRS.as_bytes());
                    }
                    _ => {
                        dict_parser.parse_operands().unwrap();
                        let operands = dict_parser.operands();

                        let mut w = Writer::new();

                        for operand in operands {
                            w.write(operand.as_bytes());
                        }

                        write(&w.finish(), operator.as_bytes());
                    }
                }
            }

            sub_w.finish()
        };

        offsets.0 = IntegerNumber::from_i32(private_dict_data.len() as i32);
        offsets.1 = IntegerNumber::from_i32(
            font_write_context.lsubrs_offsets.as_i32() - w.len() as i32,
        );

        w.extend(&private_dict_data);
    }

    Ok(())
}
