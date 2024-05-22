use std::collections::{BTreeMap, BTreeSet};
use std::ops::Add;

/// A structure that allows to remap numeric types to new
/// numbers so that they form a contiguous sequence of numbers.
#[derive(Debug, Clone)]
pub struct Remapper<C, T> {
    /// The counter that keeps track of the next number to be assigned.
    /// Should always start with 0.
    counter: C,
    /// The map that maps numbers from their old value to their new value.
    forward: BTreeMap<T, T>,
    /// The vector that stores the "reverse" mapping, i.e. given a new number,
    /// it allows to map back to the old one.
    backward: Vec<T>,
}

/// A wrapper trait around `checked_add` so we can require it for the remapper.
pub trait CheckedAdd: Sized + Add<Self, Output = Self> {
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
    /// Create a new instance of a remapper.
    pub fn new() -> Self
    where
        C: Default,
    {
        Self {
            counter: C::default(),
            forward: BTreeMap::new(),
            backward: Vec::new(),
        }
    }

    /// Get the new mapping of a value that has been remapped before.
    /// Returns `None` if it has not been remapped.
    pub fn get(&self, old: T) -> Option<T> {
        self.forward.get(&old).copied()
    }

    /// Remap a new value, either returning the previously assigned number
    /// if it already has been remapped, and assigning a new number if it
    /// has not been remapped.
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

    /// Get the number of elements that have been remapped. Assumes that
    /// the remapper was constructed with a type where `C::default` yields 0.
    pub fn len(&self) -> C {
        self.counter
    }

    /// Returns an iterator over the old values, in ascending order that is defined
    /// by the remapping.
    pub fn sorted_iter(&self) -> impl Iterator<Item = T> + '_ {
        self.backward.iter().copied()
    }
}

/// A remapper that allows to assign a new ordering to a subset of glyphs.
/// For example, let's say that we want to subset a font that only contains the
/// glyphs 4, 9 and 16. In this case, the remapper could yield a remapping
/// that assigns the following glyph IDs:
/// 0 -> 0 (The .notdef glyph will always be included)
/// 4 -> 1
/// 9 -> 2
/// 16 -> 3
/// This is necessary because a font needs to have a contiguous sequence of
/// glyph IDs that start from 0, so we cannot just reuse the old ones, but we
/// need to define a mapping.
pub struct GlyphRemapper(GlyphRemapperType);

impl GlyphRemapper {
    /// Create a new instance of a glyph remapper.
    pub fn new() -> Self {
        let mut remapper = Remapper::new();
        // .notdef is always a part of a subset.
        remapper.remap(0);
        Self(GlyphRemapperType::CustomRemapper(remapper))
    }

    /// Create a new remapper that maps each glyph to itself.
    pub fn identity() -> Self {
        Self(GlyphRemapperType::IdentityRemapper)
    }

    /// Return whether the current mapper is an identity mapper.
    pub fn is_identity(&self) -> bool {
        match &self.0 {
            GlyphRemapperType::CustomRemapper(_) => false,
            GlyphRemapperType::IdentityRemapper => true
        }
    }

    /// Create a remapper from an existing set of glyphs. The method
    /// will ensure that the mapping is monotonically increasing.
    pub fn new_from_glyphs(glyphs: &[u16]) -> Self {
        let mut map = Self::new();
        let sorted = BTreeSet::from_iter(glyphs);

        for glyph in sorted {
            map.remap(*glyph);
        }

        map
    }

    /// Get the number of gids that have been remapped.
    pub(crate) fn num_gids(&self) -> u16 {
        match &self.0 {
            GlyphRemapperType::CustomRemapper(custom) => custom.len(),
            // Function is not exposed to the outside, and we don't use the identity mapper in the crate.
            GlyphRemapperType::IdentityRemapper => unreachable!()
        }
    }

    /// Remap a glyph ID, or return the existing mapping if the
    /// glyph ID has already been remapped before.
    pub fn remap(&mut self, old: u16) -> u16 {
        match &mut self.0 {
            GlyphRemapperType::CustomRemapper(custom) => custom.remap(old),
            GlyphRemapperType::IdentityRemapper => old
        }
    }

    /// Get the mapping of a glyph ID, if it has been remapped before.
    pub fn get(&self, old: u16) -> Option<u16> {
        match &self.0 {
            GlyphRemapperType::CustomRemapper(custom) => custom.get(old),
            GlyphRemapperType::IdentityRemapper => Some(old)
        }
    }

    /// Return an iterator that yields the old glyphs, in ascending order that
    /// is defined by the remapping. For example, if we perform the following remappings:
    /// 3, 39, 8, 3, 10, 2
    /// Then the iterator will yield the following items in the order below. The order
    /// also implicitly defines the glyph IDs in the new mapping:
    /// 0 (0), 3 (1), 39 (2), 8 (3), 10 (4), 2 (5)
    pub(crate) fn remapped_gids(&self) -> impl Iterator<Item = u16> + '_ {
        match &self.0 {
            GlyphRemapperType::CustomRemapper(custom) => custom.backward.iter().copied(),
            // Function is not exposed to the outside, and we don't use the identity mapper in the crate.
            GlyphRemapperType::IdentityRemapper => unreachable!()
        }
    }
}


enum GlyphRemapperType {
    CustomRemapper(Remapper<u16, u16>),
    IdentityRemapper
}