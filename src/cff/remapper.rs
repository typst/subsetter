use std::collections::BTreeMap;
use std::ops::Add;



pub struct SidRemapper(Remapper<u16>);

impl SidRemapper {
    pub fn new() -> Self {
        SidRemapper(Remapper::new_with_count(0))
    }

    pub fn get(&self, old: u16) -> Option<u16> {
        if old < CUSTOM_SID {
            return Some(old);
        } else {
            self.0.get(old)
        }
    }

    pub fn remap(&mut self, old: u16) -> u16 {
        if old < CUSTOM_SID {
            return CUSTOM_SID;
        } else {
            self.0.remap(old)
        }
    }

    pub fn sorted(&self) -> &[u16] {
        self.0.sorted()
    }
}
