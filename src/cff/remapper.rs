use crate::cff::number::StringId;
use crate::remapper::Remapper;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

pub type FontDictRemapper = Remapper<u8, u8>;

pub struct SidRemapper<'a> {
    counter: StringId,
    sid_to_string: BTreeMap<StringId, Cow<'a, [u8]>>,
    string_to_sid: HashMap<Cow<'a, [u8]>, StringId>,
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

    pub fn get_new_sid(&self, sid: StringId) -> Option<StringId> {
        self.old_sid_to_new_sid.get(&sid).copied()
    }

    pub fn remap(&mut self, string: impl Into<Cow<'a, [u8]>> + Clone) -> StringId {
        *self.string_to_sid.entry(string.clone().into()).or_insert_with(|| {
            let value = self.counter;
            self.sid_to_string.insert(value, string.into());
            self.counter =
                StringId(self.counter.0.checked_add(1).expect("sid remapper overflowed"));

            value
        })
    }

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

    pub fn sorted_strings(&self) -> impl Iterator<Item = &Cow<'_, [u8]>> + '_ {
        self.sid_to_string.values().into_iter()
    }
}

#[cfg(test)]
mod tests {
    use crate::cff::number::StringId;
    use crate::cff::remapper::SidRemapper;
    use std::borrow::Cow;

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
