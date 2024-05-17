use crate::cff::CUSTOM_SID;
use std::collections::BTreeMap;
use std::ops::Add;

#[derive(Debug, Clone)]
pub struct Remapper<T: Ord> {
    counter: T,
    forward: BTreeMap<T, T>,
    backward: Vec<T>,
}

impl<T: Ord + PartialEq + From<u8> + Add<T, Output = T> + Default + Copy> Remapper<T> {
    pub fn new() -> Self {
        Remapper::new_with_count(T::default())
    }

    fn new_with_count(count: T) -> Self {
        let mapper = Self {
            counter: count,
            forward: BTreeMap::new(),
            backward: Vec::new(),
        };
        mapper
    }

    pub fn get(&self, old: T) -> Option<T> {
        self.forward.get(&old).copied()
    }

    pub fn remap(&mut self, old: T) -> T {
        *self.forward.entry(old).or_insert_with(|| {
            let value = self.counter;
            self.backward.push(old);
            self.counter = self.counter + T::from(1);
            value
        })
    }

    pub fn len(&self) -> u32 {
        self.forward.len() as u32
    }

    // Add a method to return the iterator
    pub fn sorted(&self) -> &[T] {
        self.backward.as_ref()
    }
}

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
