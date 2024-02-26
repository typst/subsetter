use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) struct InternalMapper {
    pub(crate) forward: HashMap<u16, u16>,
    pub(crate) backward: Vec<u16>
}

impl InternalMapper {
    pub fn new() -> Self {
        Self {
            forward: HashMap::new(),
            backward: Vec::new()
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum MapperVariant {
    IdentityMapper,
    HashmapMapper(InternalMapper)
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
            MapperVariant::HashmapMapper(ref h) => h.forward.get(&gid).copied()
        }
    }
}