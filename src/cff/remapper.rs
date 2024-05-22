use crate::cff::number::StringId;
use crate::remapper::Remapper;

pub type FontDictRemapper = Remapper<u8, u8>;

/// A wrapper around `Remapper` that takes care of automatically
/// accounting for the fact that SIDs up to 390 are standard strings.
pub struct SidRemapper(Remapper<u16, u16>);

impl SidRemapper {
    pub fn new() -> Self {
        SidRemapper(Remapper::new())
    }

    pub fn get(&self, old: StringId) -> Option<StringId> {
        if old.is_standard_string() {
            Some(old)
        } else {
            self.0
                .get(old.0 - StringId::STANDARD_STRING_LEN)
                .and_then(|n| n.checked_add(StringId::STANDARD_STRING_LEN))
                .map(StringId::from)
        }
    }

    pub fn remap(&mut self, old: StringId) -> StringId {
        if old.is_standard_string() {
            old
        } else {
            StringId::from(
                self.0.remap(old.0 - StringId::STANDARD_STRING_LEN)
                    + StringId::STANDARD_STRING_LEN,
            )
        }
    }

    pub fn sids(&self) -> impl Iterator<Item = StringId> + '_ {
        self.0
            .sorted_iter()
            .map(|n| StringId::from(n + StringId::STANDARD_STRING_LEN))
    }
}

#[cfg(test)]
mod tests {
    use crate::cff::number::StringId;
    use crate::cff::remapper::SidRemapper;

    #[test]
    fn test_remap_1() {
        let mut sid_remapper = SidRemapper::new();
        assert_eq!(sid_remapper.remap(StringId(5)), StringId(5));
        assert_eq!(sid_remapper.remap(StringId(100)), StringId(100));
        assert_eq!(sid_remapper.remap(StringId(400)), StringId(391));
        assert_eq!(sid_remapper.remap(StringId(395)), StringId(392));
        assert_eq!(sid_remapper.remap(StringId(502)), StringId(393));
        assert_eq!(sid_remapper.remap(StringId(156)), StringId(156));
        assert_eq!(sid_remapper.remap(StringId(480)), StringId(394));
        assert_eq!(sid_remapper.remap(StringId(400)), StringId(391));
        assert_eq!(sid_remapper.get(StringId(395)), Some(StringId(392)));

        assert_eq!(
            sid_remapper.sids().collect::<Vec<_>>(),
            vec![StringId(400), StringId(395), StringId(502), StringId(480)]
        )
    }
}
