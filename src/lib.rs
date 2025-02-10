//! `RangeVec` is a data structure that will return a value for any index, but only a small range
//! of values are non-default, and only these are stored. It is based on a ring buffer
//! ([`VecDeque`]) so that it may efficiently grow in either direction. It is useful for
//! applications such as backing storage for scrolling data, and was originally designed for use in
//! change tracking for an emulator's memory viewer.

use std::{
    collections::VecDeque,
    fmt::{Debug, Display},
    ops::{Bound, Index, Range, RangeBounds},
};

pub use iter::Iter;

mod iter;

/// `RangeVec` is a data structure that will return a value for any index, but only a small range
/// of values are non-default, and only these are stored. It is based on a ring buffer
/// ([`VecDeque`]) so that it may efficiently grow in either direction.
///
/// `RangeVec` requires that the stored type implement [`Default`] and [`Eq`], and it will return
/// the default value whenever an index outside of its stored range is accessed. The stored range
/// will automatically be grown or shrunk to exactly match the smallest possible range of
/// non-default values after every mutation. To facilitate this, all mutable access is currently
/// done through closures, so that the ring buffer may be adjusted based on whether the value is
/// equal to `T::default()` after mutation. There may be a guard-based API in the future as well.
///
/// Because of this, `RangeVec` currently has no `.iter_mut()` method, as without a guard it would
/// not be possible to adjust the backing storage after a mutation. However, though less flexible,
/// the [`mutate_many`] or [`mutate_non_default`] methods may work instead. The slice access methods
/// [`as_mut_slices_with`] and [`make_contiguous_with`] may also be of interest. For the same
/// reason, `RangeVec` implements [`Index`] so you can get elements using square bracked syntax:
/// `let x = my_range_vec[50];`, but does not implement [`IndexMut`]. Again, this may be possible in
/// the future using a guard API.
///
/// Because the backing storage is contiguous, this data structure is most efficient when all of
/// the non-default values are within a small range. If they are sparse, consider using a map
/// instead, particularly one with a hashing algorithm tuned for performance on integer indices.
///
/// The stored type's implementation of [`Default`] should be fairly cheap, as it will be called
/// frequently to initialize values before mutation of indices outside of the stored range, or to
/// initialize default values between stored non-default values. It is a logic error for two calls
/// to `T::default()` to return different results during the `RangeVec`'s lifetime.
///
/// [`VecDeque`]: std::collections::vec_deque::VecDeque
/// [`Default`]: std::default::Default
/// [`Eq`]: std::cmp::Eq
/// [`Index`]: std::ops::Index
/// [`IndexMut`]: std::ops::IndexMut
///
/// [`mutate_many`]: RangeVec::mutate_many
/// [`mutate_non_default`]: RangeVec::mutate_non_default
/// [`as_mut_slices_with`]: RangeVec::as_mut_slices_with
/// [`make_contiguous_with`]: RangeVec::make_contiguous_with
///
#[derive(Debug, Clone)]
pub struct RangeVec<T> {
    data: VecDeque<T>,
    offset: usize,
    default_item: T,
}

impl<T> Display for RangeVec<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.range() {
            Some(range) => f
                .debug_struct("RangeVec")
                .field("range", &range)
                .field("data", &self.data)
                .finish(),
            None => write!(f, "RangeVec {{ <empty> }}"),
        }
    }
}

impl<T> Default for RangeVec<T>
where
    T: Default + Eq,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Index<usize> for RangeVec<T>
where
    T: Default + Eq,
{
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index)
    }
}

impl<T> RangeVec<T> {
    /// Returns the currently stored range of the internal buffer, exactly encompassing the
    /// leftmost (inclusive) and rightmost (exclusive) non-default values. This will return `None`
    /// if the range is empty, i.e., if there are no non-default values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use range_vec::RangeVec;
    /// let mut range_vec: RangeVec<i32> = RangeVec::new();
    /// assert_eq!(range_vec.range(), None);
    ///
    /// range_vec.set(5, 1);
    /// range_vec.set(10, 2);
    /// assert_eq!(range_vec.range(), Some(5..11));
    /// ```
    pub fn range(&self) -> Option<Range<usize>> {
        (!self.is_empty()).then(|| self.offset..self.offset + self.data.len())
    }

    /// Returns the size of the stored range.
    ///
    /// # Examples
    ///
    /// ```
    /// # use range_vec::RangeVec;
    /// let mut range_vec: RangeVec<i32> = RangeVec::new();
    /// assert_eq!(range_vec.range_size(), 0);
    ///
    /// range_vec.set(5, 1);
    /// range_vec.set(10, 2);
    /// assert_eq!(range_vec.range_size(), 6);
    /// ```
    pub fn range_size(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if there are any stored values, i.e., if any values are non-default.
    ///
    /// # Examples
    ///
    /// ```
    /// # use range_vec::RangeVec;
    /// let mut range_vec: RangeVec<i32> = RangeVec::new();
    /// assert!(range_vec.is_empty());
    ///
    /// range_vec.set(5, 1);
    /// assert!(!range_vec.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Creates an iterator over the specified range. A range unbounded on the left will start at
    /// `0` (inclusive), and one unbounded on the right will end at `usize::MAX` (exclusive). The
    /// iterator will emit values of type `&T`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use range_vec::RangeVec;
    /// let mut range_vec: RangeVec<i32> = RangeVec::new();
    /// range_vec.set(3, 1);
    /// range_vec.set(5, 2);
    /// let numbers: Vec<i32> = range_vec.iter(1..=7).copied().collect();
    /// assert_eq!(numbers, vec![0, 0, 1, 0, 2, 0, 0]);
    /// ```
    pub fn iter(&self, range: impl RangeBounds<usize>) -> Iter<'_, T> {
        Iter::new(self, range_bounds_to_range(range))
    }

    /// Clears the `RangeVec`, resetting all values to default.
    ///
    /// # Examples
    ///
    /// ```
    /// # use range_vec::RangeVec;
    /// let mut range_vec: RangeVec<i32> = RangeVec::new();
    /// range_vec.set(3, 1);
    /// range_vec.set(5, 2);
    /// range_vec.clear();
    /// assert!(range_vec.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.data.clear();
    }
}

impl<T> RangeVec<T>
where
    T: Default + Eq,
{
    /// Creates an empty (all-default) `RangeVec`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use range_vec::RangeVec;
    /// let range_vec: RangeVec<i32> = RangeVec::new();
    /// assert_eq!(range_vec.range(), None);
    /// ```
    pub fn new() -> Self {
        Self {
            data: VecDeque::new(),
            offset: 0,
            default_item: T::default(),
        }
    }

    fn grow_to_include(&mut self, index: usize) {
        if self.data.is_empty() {
            // Empty: set offset = index and insert value
            self.offset = index;
            self.data.push_back(T::default());
        } else if index < self.offset {
            // index < offset: grow left, set offset = index and insert value
            let additional = self.offset - index;
            self.data.reserve(additional);
            for _ in 0..additional {
                self.data.push_front(T::default());
            }
            self.offset = index;
        } else if index >= self.offset + self.data.len() {
            // index >= offset + length: grow right and insert value
            let additional = index - (self.offset + self.data.len()) + 1;
            self.data.reserve(additional);
            for _ in 0..additional {
                self.data.push_back(T::default());
            }
        }
    }

    /// Provides a reference to the element at the given index, or to a default element.
    ///
    /// # Examples
    ///
    /// ```
    /// # use range_vec::RangeVec;
    /// let mut range_vec: RangeVec<i32> = RangeVec::new();
    /// range_vec.set(5, 1);
    /// assert_eq!(range_vec.get(5), &1);
    /// assert_eq!(range_vec.get(10), &0);
    /// ```
    pub fn get(&self, index: usize) -> &T {
        match index
            .checked_sub(self.offset)
            .and_then(|index| self.data.get(index))
        {
            Some(item) => item,
            None => &self.default_item,
        }
    }

    fn shrink_left(&mut self) {
        match self.data.iter().position(|item| item != &self.default_item) {
            Some(index) => {
                self.data.drain(..index);
                self.offset += index;
            }
            None => self.data.clear(),
        }
    }

    fn shrink_right(&mut self) {
        match self
            .data
            .iter()
            .rposition(|item| item != &self.default_item)
        {
            Some(index) => {
                self.data.drain(index + 1..);
            }
            None => self.data.clear(),
        }
    }

    fn shrink(&mut self, index: usize) {
        if index == self.offset {
            self.shrink_left();
        } else if index == self.offset + self.data.len() - 1 {
            self.shrink_right();
        }
    }

    fn grow_and_set(&mut self, index: usize, value: T) {
        if value != self.default_item {
            self.grow_to_include(index);
            self.data[index - self.offset] = value;
        }
    }

    /// Set the value at index `index`. If the element is outside of the stored range and is not
    /// equal to `T::default()`, the ring buffer will be grown to accomodate it. If it is inside
    /// the stored range and is equal to `T::default`, the ring buffer will be shrunk accordingly.
    ///
    /// # Examples
    ///
    /// ```
    /// # use range_vec::RangeVec;
    /// let mut range_vec: RangeVec<i32> = RangeVec::new();
    /// range_vec.set(5, 1);
    /// range_vec.set(7, 2);
    /// range_vec.set(9, 3);
    /// assert_eq!(range_vec.range(), Some(5..10));
    ///
    /// range_vec.set(5, 0);
    /// assert_eq!(range_vec.range(), Some(7..10));
    /// ```
    pub fn set(&mut self, index: usize, value: T) {
        match index
            .checked_sub(self.offset)
            .and_then(|index| self.data.get_mut(index))
        {
            Some(item) => {
                // index is inside the current range
                *item = value;
                self.shrink(index);
            }
            None => {
                // index is outside the current range
                self.grow_and_set(index, value);
            }
        }
    }

    /// Mutate the value at index `index`. If the element is outside of the stored range and is not
    /// equal to `T::default()` after mutation, the ring buffer will be grown to accomodate it. If
    /// it is inside the stored range and is equal to `T::default` after mutation, the ring buffer
    /// will be shrunk accordingly. Any value returned from the passed closure `f` will be returned
    /// from the method.
    ///
    /// # Examples
    ///
    /// ```
    /// # use range_vec::RangeVec;
    /// let mut range_vec: RangeVec<i32> = RangeVec::new();
    /// range_vec.get_mut_with(5, |v| *v = 1);
    /// range_vec.get_mut_with(7, |v| *v += 2);
    /// range_vec.get_mut_with(9, |v| *v = 3);
    /// assert_eq!(range_vec.range(), Some(5..10));
    ///
    /// range_vec.get_mut_with(5, |v| *v -= 1);
    /// range_vec.get_mut_with(9, |v| *v = 0);
    /// assert_eq!(range_vec.range(), Some(7..8));
    /// ```
    pub fn get_mut_with<F, R>(&mut self, index: usize, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        match index
            .checked_sub(self.offset)
            .and_then(|index| self.data.get_mut(index))
        {
            Some(item) => {
                // index is inside the current range
                let ret = f(item);
                self.shrink(index);
                ret
            }
            None => {
                // index is outside the current range
                let mut value = T::default();
                let ret = f(&mut value);
                self.grow_and_set(index, value);
                ret
            }
        }
    }

    /// Mutate a range of values. This method is equivalent to calling
    /// [`get_mut_with`](RangeVec::get_mut_with) repeatedly on an entire range of values, except it does not
    /// pass through the closure's return value. In addition, it only makes the checks to grow and
    /// shrink the backing storage once. The closure is also passed the index as its first
    /// argument.
    ///
    /// # Examples
    ///
    /// ```
    /// # use range_vec::RangeVec;
    /// let mut range_vec: RangeVec<i32> = RangeVec::new();
    /// range_vec.mutate_many(5..15, |_, v| *v += 1);
    /// range_vec.mutate_many(10..15, |_, v| *v -= 1);
    /// range_vec.mutate_many(5..8, |i, v| *v += i as i32);
    /// assert_eq!(range_vec.range(), Some(5..10));
    /// assert_eq!(
    ///     range_vec.iter(5..15).copied().collect::<Vec<i32>>(),
    ///     vec![6, 7, 8, 1, 1, 0, 0, 0, 0, 0],
    /// );
    /// ```
    pub fn mutate_many<F>(&mut self, range: impl RangeBounds<usize>, mut f: F)
    where
        F: FnMut(usize, &mut T),
    {
        let range = range_bounds_to_range(range);
        for i in range {
            if let Some(item) = i
                .checked_sub(self.offset)
                .and_then(|index| self.data.get_mut(index))
            {
                f(i, item);
            } else {
                let mut value = T::default();
                f(i, &mut value);
                self.grow_and_set(i, value);
            }
        }
        self.shrink_left();
        self.shrink_right();
    }

    /// Mutate all values that are not the default value. This method will call `f` repeatedly on
    /// each element `v` where `v != T::default()`, and only makes the checks to grow and shrink the
    /// backing storage once. The closure is also passed the index as its first argument.
    ///
    /// # Examples
    ///
    /// ```
    /// # use range_vec::RangeVec;
    /// let mut range_vec: RangeVec<i32> = RangeVec::new();
    /// range_vec.set(5, -5);
    /// range_vec.set(7, 1);
    /// range_vec.set(9, 2);
    ///
    /// range_vec.mutate_non_default(|i, v| *v += i as i32);
    /// assert_eq!(range_vec.range(), Some(7..10));
    /// assert_eq!(
    ///     range_vec.iter(7..10).copied().collect::<Vec<i32>>(),
    ///     vec![8, 0, 11],
    /// );
    /// ```
    pub fn mutate_non_default<F>(&mut self, mut f: F)
    where
        F: FnMut(usize, &mut T),
    {
        for (i, item) in self.data.iter_mut().enumerate() {
            if item != &self.default_item {
                f(i + self.offset, item);
            }
        }
        self.shrink_left();
        self.shrink_right();
    }

    /// Reset the value at a given index to `T::default()`, and shrink the backing storage
    /// accordingly. If `index` is outside the stored range, this method is a no-op.
    ///
    /// # Examples
    ///
    /// ```
    /// # use range_vec::RangeVec;
    /// let mut range_vec: RangeVec<i32> = RangeVec::new();
    /// range_vec.set(5, 1);
    /// range_vec.set(7, 2);
    /// range_vec.set(9, 3);
    ///
    /// range_vec.reset(7);
    /// assert_eq!(range_vec.range(), Some(5..10));
    /// assert_eq!(range_vec.get(7), &0);
    ///
    /// range_vec.reset(5);
    /// assert_eq!(range_vec.range(), Some(9..10));
    /// ```
    pub fn reset(&mut self, index: usize) {
        if let Some(item) = index
            .checked_sub(self.offset)
            .and_then(|index| self.data.get_mut(index))
        {
            *item = T::default();
            self.shrink(index);
        }
    }

    /// Reset all values outside of `range` to `T::default()`, and shrink the backing storage
    /// accordingly.
    ///
    /// # Examples
    /// ```
    /// # use range_vec::RangeVec;
    /// let mut range_vec: RangeVec<i32> = RangeVec::new();
    /// range_vec.set(5, 1);
    /// range_vec.set(7, 2);
    /// range_vec.set(9, 3);
    ///
    /// range_vec.truncate(6..);
    /// assert_eq!(range_vec.range(), Some(7..10));
    ///
    /// range_vec.truncate(15..20);
    /// assert!(range_vec.is_empty());
    pub fn truncate(&mut self, range: impl RangeBounds<usize>) {
        let range = range_bounds_to_range(range);
        if range.is_empty() {
            // Clear the entire buffer if the range is empty
            self.data.clear();
            return;
        }

        // Drain right of range
        let new_end = range.end.saturating_sub(self.offset).min(self.data.len());
        self.data.drain(new_end..);

        // Drain left of range
        let new_start = range.start.saturating_sub(self.offset).min(self.data.len());
        self.data.drain(..new_start);
        self.offset += new_start;

        self.shrink_left();
        self.shrink_right();
    }

    /// Mutably access the backing storage for `range`. This method will grow the ring buffer to
    /// include the entire range if needed, and shrink afterwards as appropriate. Because the
    /// backing storage is a ring buffer, it may be split up into two slices, which are provided as
    /// arguments to `f`. If all of the data is contiguous, the second slice will be empty. Any
    /// value returned from `f` will be returned from the method.
    ///
    /// If you need to access a single contiguous slice and don't care about paying the cost to
    /// rearrange the backing storage, use [`make_contiguous_with`](RangeVec::make_contiguous_with).
    pub fn as_mut_slices_with<F, R>(&mut self, range: impl RangeBounds<usize>, f: F) -> R
    where
        F: FnOnce(&mut [T], &mut [T]) -> R,
    {
        let range = range_bounds_to_range(range);
        if range.is_empty() {
            return f(&mut [], &mut []);
        }

        self.grow_to_include(range.start);
        self.grow_to_include(range.end);
        let data_len = self.data.len();
        let (mut left, mut right) = self.data.as_mut_slices();
        let split_point = self.offset + left.len();

        if let Some(overlap) = split_point.checked_sub(range.end) {
            right = &mut [];
            let left_len = left.len();
            left = &mut left[..left_len - overlap];
        } else if let Some(overlap) = (self.offset + data_len).checked_sub(range.end) {
            let right_len = right.len();
            right = &mut right[..right_len - overlap];
        }

        if let Some(overlap) = range.start.checked_sub(split_point) {
            left = &mut [];
            right = &mut right[overlap..];
        } else if let Some(overlap) = range.start.checked_sub(self.offset) {
            left = &mut left[overlap..];
        }

        if left.is_empty() && !right.is_empty() {
            left = right;
            right = &mut [];
        }

        let ret = f(left, right);
        self.shrink_left();
        self.shrink_right();
        ret
    }

    /// Rearrange the backing storage to make it contiguous, and mutably access it for `range`. This
    /// method will grow the ring buffer to include the entire range if needed, and shrink
    /// afterwards as appropriate. Any value returned from `f` will be returned freom the method.
    ///
    /// If you don't want to pay the cost to rearrange the backing storage but are okay with the
    /// data being split up into two slices, use [`as_mut_slices_with`](RangeVec::as_mut_slices_with).
    pub fn make_contiguous_with<F, R>(&mut self, range: impl RangeBounds<usize>, f: F) -> R
    where
        F: FnOnce(&mut [T]) -> R,
    {
        let range = range_bounds_to_range(range);
        if range.is_empty() {
            return f(&mut []);
        }

        self.grow_to_include(range.start);
        self.grow_to_include(range.end);

        let (left, right) = self.data.as_mut_slices();
        let ret = if range.end - self.offset <= left.len() {
            f(left)
        } else if range.start - self.offset >= left.len() {
            f(right)
        } else {
            let data_len = self.data.len();
            let mut slice = self.data.make_contiguous();
            if let Some(overlap) = (self.offset + data_len).checked_sub(range.end) {
                slice = &mut slice[..data_len - overlap];
            }
            if let Some(overlap) = range.start.checked_sub(self.offset) {
                slice = &mut slice[overlap..];
            }
            f(slice)
        };

        self.shrink_left();
        self.shrink_right();
        ret
    }
}

fn range_bounds_to_range(range_bounds: impl RangeBounds<usize>) -> Range<usize> {
    let start = match range_bounds.start_bound() {
        Bound::Excluded(bound) => *bound + 1,
        Bound::Included(bound) => *bound,
        Bound::Unbounded => 0,
    };
    let end = match range_bounds.end_bound() {
        Bound::Excluded(bound) => *bound,
        Bound::Included(bound) => *bound + 1,
        Bound::Unbounded => usize::MAX,
    };
    start..end
}

#[cfg(test)]
mod test {
    use super::RangeVec;

    #[test]
    fn test_get_index() {
        let range_vec = RangeVec::<u8>::new();
        assert_eq!(range_vec.get(0), &0);
        assert_eq!(range_vec.get(100), &0);
        assert_eq!(range_vec[200], 0);
        assert!(range_vec.is_empty());
    }

    #[test]
    fn test_set_get_mut_with_reset() {
        let mut range_vec = RangeVec::<u8>::new();
        range_vec.set(5, 10);
        range_vec.get_mut_with(10, |v| *v = 20);
        range_vec.set(3, 5);
        assert_eq!(range_vec[0], 0);
        assert_eq!(range_vec.get(5), &10);
        assert_eq!(range_vec.get(10), &20);
        assert_eq!(range_vec[12], 0);
        assert_eq!(range_vec.range(), Some(3..11));

        range_vec.get_mut_with(3, |v| *v = 0);
        assert_eq!(range_vec.range(), Some(5..11));
        range_vec.set(10, 0);
        range_vec.set(5, 0);
        assert!(range_vec.is_empty());

        range_vec.set(20, 100);
        range_vec.reset(20);
        range_vec.reset(20);
        assert!(range_vec.is_empty());
    }

    #[test]
    fn test_display() {
        let mut range_vec = RangeVec::<u8>::new();
        assert_eq!(format!("{}", range_vec), "RangeVec { <empty> }");
        range_vec.set(5, 1);
        range_vec.set(7, 2);
        range_vec.set(9, 3);
        assert_eq!(
            format!("{}", range_vec),
            "RangeVec { range: 5..10, data: [1, 0, 2, 0, 3] }"
        );
    }

    #[test]
    fn test_mutate_many() {
        let mut range_vec = RangeVec::<i32>::new();
        range_vec.set(5, -1);
        range_vec.set(7, 1);
        range_vec.mutate_many(5..9, |_, value| *value += 1);
        assert_eq!(range_vec.range(), Some(6..9));
        assert_eq!(
            range_vec.iter(6..9).copied().collect::<Vec<_>>(),
            vec![1, 2, 1]
        );
    }

    #[test]
    fn test_mutate_non_default() {
        let mut range_vec = RangeVec::<i32>::new();
        range_vec.set(5, -1);
        range_vec.set(7, 1);
        range_vec.mutate_non_default(|_, value| *value += 1);
        assert_eq!(range_vec.range(), Some(7..8));
        assert_eq!(range_vec[7], 2);
    }

    #[test]
    fn test_truncate() {
        let mut range_vec = RangeVec::<u8>::new();
        range_vec.set(5, 5);
        range_vec.set(6, 6);
        range_vec.set(7, 7);
        range_vec.set(8, 8);
        range_vec.set(9, 9);
        range_vec.truncate(6..9);
        assert_eq!(range_vec.range(), Some(6..9));

        range_vec.truncate(10..15);
        assert!(range_vec.is_empty());
    }

    #[test]
    fn test_as_mut_slices_with() {
        let mut range_vec = RangeVec::<i32>::new();
        range_vec.set(6, 6);
        range_vec.set(7, 7);
        range_vec.set(8, 8);
        range_vec.set(9, -1);
        range_vec.set(5, 5);
        range_vec.as_mut_slices_with(3..10, |left, right| {
            for item in left.iter_mut().chain(right.iter_mut()) {
                *item += 1;
            }
        });
        assert_eq!(range_vec.range(), Some(3..9));
        assert_eq!(
            range_vec.iter(3..9).copied().collect::<Vec<_>>(),
            vec![1, 1, 6, 7, 8, 9]
        );
    }

    #[test]
    fn test_make_contiguous_with() {
        let mut range_vec = RangeVec::<i32>::new();
        range_vec.set(6, 6);
        range_vec.set(7, 7);
        range_vec.set(8, 8);
        range_vec.set(9, -1);
        range_vec.set(5, 5);
        range_vec.make_contiguous_with(3..10, |slice| {
            for item in slice {
                *item += 1;
            }
        });
        assert_eq!(range_vec.range(), Some(3..9));
        assert_eq!(
            range_vec.iter(3..9).copied().collect::<Vec<_>>(),
            vec![1, 1, 6, 7, 8, 9]
        );
    }
}
