use crate::cff::dict::DictionaryParser;
use crate::cff::{private_dict_operator, MAX_OPERANDS_LEN};

#[derive(Default, Clone, Debug)]
pub struct PrivateDict {
    pub blue_values: Option<Vec<f64>>,
    pub other_blues: Option<Vec<f64>>,
    pub family_blues: Option<Vec<f64>>,
    pub family_other_blues: Option<Vec<f64>>,
    pub blue_scale: Option<f64>,
    pub blue_shift: Option<f64>,
    pub blue_fuzz: Option<f64>,
    pub std_hw: Option<f64>,
    pub std_vw: Option<f64>,
    pub stem_snap_h: Option<Vec<f64>>,
    pub stem_snap_v: Option<Vec<f64>>,
    pub force_bold: Option<bool>,
    pub language_group: Option<f64>,
    pub expansion_factor: Option<f64>,
    pub initial_random_seed: Option<f64>,
    pub subrs: Option<usize>,
    pub default_width_x: Option<f64>,
    pub nominal_width_x: Option<f64>,
}

pub fn parse_private_dict(data: &[u8]) -> Option<PrivateDict> {
    let mut dict = PrivateDict::default();
    let mut operands_buffer = [0.0; MAX_OPERANDS_LEN];
    let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);

    while let Some(operator) = dict_parser.parse_next() {
        match operator.get() {
            private_dict_operator::BLUE_VALUES => {
                dict.blue_values = Some(dict_parser.parse_delta()?)
            }
            private_dict_operator::OTHER_BLUES => {
                dict.other_blues = Some(dict_parser.parse_delta()?)
            }
            private_dict_operator::FAMILY_BLUES => {
                dict.family_blues = Some(dict_parser.parse_delta()?)
            }
            private_dict_operator::FAMILY_OTHER_BLUES => {
                dict.family_other_blues = Some(dict_parser.parse_delta()?)
            }
            private_dict_operator::BLUE_SCALE => {
                dict.blue_scale = Some(dict_parser.parse_number()?)
            }
            private_dict_operator::BLUE_SHIFT => {
                dict.blue_shift = Some(dict_parser.parse_number()?)
            }
            private_dict_operator::BLUE_FUZZ => {
                dict.blue_fuzz = Some(dict_parser.parse_number()?)
            }
            private_dict_operator::STD_HW => {
                dict.std_hw = Some(dict_parser.parse_number()?)
            }
            private_dict_operator::STD_VW => {
                dict.std_vw = Some(dict_parser.parse_number()?)
            }
            private_dict_operator::STEM_SNAP_H => {
                dict.stem_snap_h = Some(dict_parser.parse_delta()?)
            }
            private_dict_operator::STEM_SNAP_V => {
                dict.stem_snap_v = Some(dict_parser.parse_delta()?)
            }
            private_dict_operator::FORCE_BOLD => {
                dict.force_bold = Some(dict_parser.parse_bool()?)
            }
            private_dict_operator::LANGUAGE_GROUP => {
                dict.language_group = Some(dict_parser.parse_number()?)
            }
            private_dict_operator::EXPANSION_FACTOR => {
                dict.expansion_factor = Some(dict_parser.parse_number()?)
            }
            private_dict_operator::INITIAL_RANDOM_SEED => {
                dict.initial_random_seed = Some(dict_parser.parse_number()?)
            }
            private_dict_operator::SUBRS => {
                dict.subrs = Some(dict_parser.parse_offset()?)
            }
            private_dict_operator::DEFAULT_WIDTH_X => {
                dict.default_width_x = Some(dict_parser.parse_number()?)
            }
            private_dict_operator::NOMINAL_WIDTH_X => {
                dict.nominal_width_x = Some(dict_parser.parse_number()?)
            }
            _ => {
                // Invalid operator
                return None;
            }
        }
    }

    Some(dict)
}
