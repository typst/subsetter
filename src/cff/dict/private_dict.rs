use crate::cff::dict::operators::*;
use crate::cff::dict::DictionaryParser;
use crate::cff::types::Number;
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
