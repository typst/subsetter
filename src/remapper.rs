use std::collections::BTreeMap;
use std::ops::Add;

#[derive(Debug, Clone)]
pub(crate) struct Remapper<C, T> {
    counter: C,
    forward: BTreeMap<T, T>,
    backward: Vec<T>,
}

pub(crate) trait CheckedAdd: Sized + Add<Self, Output = Self> {
    /// Adds two numbers, checking for overflow. If overflow happens, `None` is
    /// returned.
    fn checked_add(&self, v: &Self) -> Option<Self>;
}

impl CheckedAdd for u8 {
    fn checked_add(&self, v: &Self) -> Option<Self> {
        u8::checked_add(*self, *v)
    }
}

impl CheckedAdd for u16 {
    fn checked_add(&self, v: &Self) -> Option<Self> {
        u16::checked_add(*self, *v)
    }
}

impl CheckedAdd for u32 {
    fn checked_add(&self, v: &Self) -> Option<Self> {
        u32::checked_add(*self, *v)
    }
}

impl<C: CheckedAdd + Copy + From<u8>, T: Ord + Copy + From<u8> + From<C>> Remapper<C, T> {
    pub fn new() -> Self
    where
        C: Default,
    {
        Remapper::new_with_count(C::default())
    }

    fn new_with_count(count: C) -> Self {
        Self {
            counter: count,
            forward: BTreeMap::new(),
            backward: Vec::new(),
        }
    }

    pub fn get(&self, old: T) -> Option<T> {
        self.forward.get(&old).copied()
    }

    pub fn remap(&mut self, old: T) -> T {
        *self.forward.entry(old).or_insert_with(|| {
            let value = self.counter;
            self.backward.push(old);
            self.counter = self
                .counter
                .checked_add(&C::from(1))
                .expect("remapper was overflowed");
            value.into()
        })
    }

    pub fn len(&self) -> C {
        self.counter
    }

    pub fn sequential_iter(&self) -> impl Iterator<Item = T> + '_ {
        self.backward.iter().copied()
    }
}

#[derive(Clone)]
pub struct GidMapper(Remapper<u16, u16>);
pub type OldGid = u16;
pub type NewGid = u16;

impl GidMapper {
    pub fn new() -> Self {
        let mut remapper = Remapper::new();
        // We always map
        remapper.remap(0);
        GidMapper(remapper)
    }

    pub fn num_gids(&self) -> u16 {
        self.0.len()
    }

    pub fn remap(&mut self, old: u16) -> u16 {
        self.0.remap(old)
    }

    pub fn get(&self, old: u16) -> Option<u16> {
        self.0.get(old)
    }

    pub fn iter(&self) -> GidIterator {
        GidIterator {
            current: 0,
            max: self.num_gids(),
            entries: &self.0.backward,
        }
    }

    // Returned gids are sorted by their new ID.
    pub fn old_gids(&self) -> impl Iterator<Item = u16> + '_ {
        self.0.backward.iter().copied()
    }
}

pub struct GidIterator<'a> {
    current: u16,
    max: u16,
    entries: &'a [u16],
}

impl Iterator for GidIterator<'_> {
    type Item = (NewGid, OldGid);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.max {
            return None;
        }

        let new_gid = self.current;
        let old_gid = self.entries[new_gid as usize];

        self.current += 1;
        Some((new_gid, old_gid))
    }
}
