use crate::cff::dict::{DictionaryParser, Operator};
use crate::cff::index::parse_index;
use crate::cff::MAX_OPERANDS_LEN;
use crate::stream::Reader;
use std::collections::HashMap;

pub fn parse_top_dict(r: &mut Reader) -> Option<HashMap<Operator, Vec<f64>>> {
    let mut map = HashMap::new();
    let index = parse_index::<u16>(r).ok()?;

    // The Top DICT INDEX should have only one dictionary.
    let data = index.get(0)?;

    let mut operands_buffer = [0.0; MAX_OPERANDS_LEN];
    let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);
    while let Some(operator) = dict_parser.parse_next() {
        dict_parser.parse_operands()?;
        map.insert(operator, Vec::from(dict_parser.operands()));
    }

    Some(map)
}
