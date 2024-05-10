use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct GidMapper {
    counter: u16,
    forward: HashMap<u16, u16>,
    backward: Vec<u16>,
}

impl GidMapper {
    pub fn new() -> Self {
        let mut mapper = Self {
            counter: 0,
            forward: HashMap::new(),
            backward: Vec::new(),
        };

        // Make sure 0 is always mapped to 0
        mapper.remap(0);

        mapper
    }

    pub fn get(&self, old_gid: u16) -> Option<u16> {
        self.forward.get(&old_gid).copied()
    }

    pub fn get_reverse(&self, new_gid: u16) -> Option<u16> {
        self.backward.get(new_gid as usize).copied()
    }

    pub fn old_gids(&self) -> &[u16] {
        &self.backward
    }

    pub fn from_gid_set(gids: &[u16]) -> Self {
        let mut mapper = GidMapper::new();

        for gid in gids {
            mapper.remap(*gid);
        }

        mapper
    }

    pub fn num_gids(&self) -> u16 {
        self.counter
    }

    pub fn remap(&mut self, gid: u16) -> u16 {
        *self.forward.entry(gid).or_insert_with(|| {
            let value = self.counter;
            self.backward.push(gid);
            self.counter += 1;
            value
        })
    }
}
