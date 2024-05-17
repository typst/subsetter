use crate::cff::types::StringId;
use crate::remapper::Remapper;
use std::ops::Add;

pub type SubroutineMapper = Remapper<u32, u32>;

pub struct SidRemapper(Remapper<u16, u16>);

impl SidRemapper {
    pub fn new() -> Self {
        SidRemapper(Remapper::new())
    }

    pub fn get(&self, old: StringId) -> Option<StringId> {
        if old.is_standard_string() {
            return Some(old);
        } else {
            self.0
                .get(old.0)
                .and_then(|n| n.checked_add(StringId::CUSTOM_SID))
                .map(StringId::from)
        }
    }

    pub fn remap(&mut self, old: StringId) -> StringId {
        if old.is_standard_string() {
            return old;
        } else {
            StringId::from(
                self.0.remap(old.0 - StringId::CUSTOM_SID) + StringId::CUSTOM_SID,
            )
        }
    }

    pub fn sids(&self) -> impl Iterator<Item = StringId> + '_ {
        self.0
            .sequential_iter()
            .map(|n| StringId::from(n + StringId::CUSTOM_SID))
    }
}

#[cfg(test)]
mod tests {
    use crate::cff::remapper::SidRemapper;
    use crate::cff::types::StringId;

    #[test]
    fn test_remap_1() {
        let mut sid_remapper = SidRemapper::new();
        assert_eq!(sid_remapper.remap(StringId(5)), StringId(5));
        assert_eq!(sid_remapper.remap(StringId(100)), StringId(100));
        assert_eq!(sid_remapper.remap(StringId(400)), StringId(392));
        assert_eq!(sid_remapper.remap(StringId(395)), StringId(393));
        assert_eq!(sid_remapper.remap(StringId(502)), StringId(394));
        assert_eq!(sid_remapper.remap(StringId(156)), StringId(156));
        assert_eq!(sid_remapper.remap(StringId(480)), StringId(395));
        assert_eq!(sid_remapper.remap(StringId(400)), StringId(392));

        assert_eq!(
            sid_remapper.sids().collect::<Vec<_>>(),
            vec![StringId(400), StringId(395), StringId(502), StringId(480)]
        )
    }
}
