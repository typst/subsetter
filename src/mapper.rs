use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) struct InternalMapper {
    counter: u16,
    forward: HashMap<u16, u16>,
    backward: Vec<u16>,
}

impl From<HashMap<u16, u16>> for InternalMapper {
    fn from(value: HashMap<u16, u16>) -> Self {
        let mut mapper = InternalMapper::new();

        let mut items = value.iter().collect::<Vec<_>>();
        items.sort_by_key(|(_, v)| *v);

        for (k, _) in items {
            mapper.insert(*k);
        }

        mapper
    }
}

impl InternalMapper {
    pub fn new() -> Self {
        Self {
            counter: 0,
            forward: HashMap::new(),
            backward: Vec::new(),
        }
    }

    pub fn get(&self, gid: u16) -> Option<u16> {
        self.forward.get(&gid).copied()
    }

    pub fn get_reverse(&self, gid: u16) -> Option<u16> {
        self.backward.get(gid as usize).copied()
    }

    pub fn old_gids(&self) -> &Vec<u16> {
        &self.backward
    }

    // pub fn num_gids(&self) -> u16 {
    //     self.counter
    // }

    pub fn insert(&mut self, gid: u16) {
        self.forward.entry(gid).or_insert_with(|| {
            let value = self.counter;
            self.backward.push(gid);
            self.counter += 1;
            value
        });
    }
}

#[derive(Debug, Clone)]
pub(crate) enum MapperVariant {
    IdentityMapper,
    HashmapMapper(InternalMapper),
}

/// A mapper that maps old gids to new ones.
#[derive(Debug, Clone)]
pub struct Mapper(pub(crate) MapperVariant);

impl Mapper {
    /// Create a mapper that maps each gid to itself.
    pub fn identity_mapper() -> Self {
        Self(MapperVariant::IdentityMapper)
    }

    /// Get the newly mapped gid for an old gid.
    pub fn get(&self, gid: u16) -> Option<u16> {
        match self.0 {
            MapperVariant::IdentityMapper => Some(gid),
            MapperVariant::HashmapMapper(ref h) => h.forward.get(&gid).copied(),
        }
    }
}
