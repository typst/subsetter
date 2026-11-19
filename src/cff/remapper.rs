use crate::cff::number::StringId;
use crate::remapper::Remapper;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

// FDSelect stores an 8-bit index, so the count can reach 256.
pub type FontDictRemapper = Remapper<u16, u8>;

/// Remap old SIDs to new SIDs, and also allow the insertion of
/// new strings.
pub struct SidRemapper<'a> {
    /// Next SID to be assigned.
    counter: StringId,
    /// A map from SIDs to their corresponding string.
    sid_to_string: BTreeMap<StringId, Cow<'a, [u8]>>,
    /// A map from strings to their corresponding SID (so the reverse of `sid_to_string`).
    string_to_sid: HashMap<Cow<'a, [u8]>, StringId>,
    /// A map from old SIDs to new SIDs.
    old_sid_to_new_sid: HashMap<StringId, StringId>,
}

impl<'a> SidRemapper<'a> {
    pub fn new() -> Self {
        Self {
            counter: StringId(StringId::STANDARD_STRING_LEN),
            sid_to_string: BTreeMap::new(),
            string_to_sid: HashMap::new(),
            old_sid_to_new_sid: HashMap::new(),
        }
    }

    pub fn get(&self, string: &[u8]) -> Option<StringId> {
        self.string_to_sid.get(string).copied()
    }

    /// Get the new SID to a correpsonding old SID.
    pub fn get_new_sid(&self, sid: StringId) -> Option<StringId> {
        self.old_sid_to_new_sid.get(&sid).copied()
    }

    /// Remap a string.
    pub fn remap(&mut self, string: impl Into<Cow<'a, [u8]>> + Clone) -> StringId {
        *self.string_to_sid.entry(string.clone().into()).or_insert_with(|| {
            let value = self.counter;
            self.sid_to_string.insert(value, string.into());
            self.counter =
                StringId(self.counter.0.checked_add(1).expect("sid remapper overflowed"));

            value
        })
    }

    /// Remap an old SID and its corresponding string.
    pub fn remap_with_old_sid(
        &mut self,
        sid: StringId,
        string: impl Into<Cow<'a, [u8]>> + Clone,
    ) -> StringId {
        if let Some(new_sid) = self.old_sid_to_new_sid.get(&sid) {
            *new_sid
        } else {
            let new_sid = self.remap(string);
            self.old_sid_to_new_sid.insert(sid, new_sid);
            new_sid
        }
    }

    /// Returns an iterator over the strings, ordered by their new SID.
    pub fn sorted_strings(&self) -> impl Iterator<Item = &Cow<'_, [u8]>> + '_ {
        self.sid_to_string.values()
    }
}

#[cfg(test)]
mod tests {
    use crate::cff::number::StringId;
    use crate::cff::remapper::{FontDictRemapper, SidRemapper};
    use crate::cff::subroutines::SubroutineCollection;
    use std::borrow::Cow;

    #[test]
    fn all_font_dict_indices_are_addressable() {
        let subrs = SubroutineCollection::new(vec![vec![]; 256]);
        assert!(subrs.get_handler(u8::MAX).is_some());

        let mut map = FontDictRemapper::new();
        for fd in 0..=u8::MAX {
            assert_eq!(map.try_remap(fd), Ok(fd));
        }
        assert_eq!(map.len(), 256);
        assert_eq!(map.get(u8::MAX), Some(u8::MAX));
    }

    #[test]
    fn test_remap_1() {
        let mut sid_remapper = SidRemapper::new();
        assert_eq!(sid_remapper.remap(b"hi".to_vec()), StringId(391));
        assert_eq!(sid_remapper.remap(b"there".to_vec()), StringId(392));
        assert_eq!(sid_remapper.remap(b"hi".to_vec()), StringId(391));
        assert_eq!(sid_remapper.remap(b"test".to_vec()), StringId(393));

        assert_eq!(
            sid_remapper.sorted_strings().cloned().collect::<Vec<_>>(),
            vec![
                Cow::<[u8]>::Owned(b"hi".to_vec()),
                Cow::Owned(b"there".to_vec()),
                Cow::Owned(b"test".to_vec())
            ]
        )
    }
}
