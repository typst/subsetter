use crate::stream::{Readable, Reader};

/// A slice-like container that converts internal binary data only on access.
///
/// Array values are stored in a continuous data chunk.
#[derive(Clone, Copy)]
pub struct LazyArray16<'a, T> {
    data: &'a [u8],
    data_type: core::marker::PhantomData<T>,
}

impl<T> Default for LazyArray16<'_, T> {
    #[inline]
    fn default() -> Self {
        LazyArray16 { data: &[], data_type: core::marker::PhantomData }
    }
}

impl<'a, T: Readable<'a>> LazyArray16<'a, T> {
    /// Creates a new `LazyArray`.
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        LazyArray16 { data, data_type: core::marker::PhantomData }
    }

    /// Returns a value at `index`.
    #[inline]
    pub fn get(&self, index: u16) -> Option<T> {
        if index < self.len() {
            let start = usize::from(index) * T::SIZE;
            let end = start + T::SIZE;
            self.data
                .get(start..end)
                .map(Reader::new)
                .and_then(|mut r| T::read(&mut r))
        } else {
            None
        }
    }

    /// Returns array's length.
    #[inline]
    pub fn len(&self) -> u16 {
        (self.data.len() / T::SIZE) as u16
    }
}

impl<'a, T: Readable<'a> + core::fmt::Debug + Copy> core::fmt::Debug
    for LazyArray16<'a, T>
{
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_list().entries(*self).finish()
    }
}

impl<'a, T: Readable<'a>> IntoIterator for LazyArray16<'a, T> {
    type Item = T;
    type IntoIter = LazyArrayIter16<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        LazyArrayIter16 { data: self, index: 0 }
    }
}

/// An iterator over `LazyArray16`.
#[derive(Clone, Copy)]
#[allow(missing_debug_implementations)]
pub struct LazyArrayIter16<'a, T> {
    data: LazyArray16<'a, T>,
    index: u16,
}

impl<'a, T: Readable<'a>> Default for LazyArrayIter16<'a, T> {
    #[inline]
    fn default() -> Self {
        LazyArrayIter16 { data: LazyArray16::new(&[]), index: 0 }
    }
}

impl<'a, T: Readable<'a>> Iterator for LazyArrayIter16<'a, T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.index += 1; // TODO: check
        self.data.get(self.index - 1)
    }

    #[inline]
    fn count(self) -> usize {
        usize::from(self.data.len().saturating_sub(self.index))
    }
}
