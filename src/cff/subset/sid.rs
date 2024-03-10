use crate::cff::subset::Remapper;
use crate::stream::StringId;

pub struct SidRemapper(Remapper<StringId>);

impl SidRemapper {
    pub fn new() -> Self {
        Self(Remapper::new_from(391))
    }

    pub fn remap(&mut self, string_id: StringId) -> StringId {
        if string_id.0 <= 390 {
            return string_id;
        } else {
            return self.0.remap(string_id);
        }
    }
}
