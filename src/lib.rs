use std::{
    collections::VecDeque,
    fmt::Debug,
    ops::{Bound, Index, Range, RangeBounds},
};

use iter::Iter;

mod iter;

#[derive(Clone)]
pub struct RangeVec<T> {
    data: VecDeque<T>,
    offset: usize,
    default_item: T,
}

impl<T> Debug for RangeVec<T>
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
    pub fn range(&self) -> Option<Range<usize>> {
        (!self.is_empty()).then(|| self.offset..self.offset + self.data.len())
    }

    pub fn range_size(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn iter(&self, range: impl RangeBounds<usize>) -> Iter<'_, T> {
        Iter::new(self, range_bounds_to_range(range))
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }
}

impl<T> RangeVec<T>
where
    T: Default + Eq,
{
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

    pub fn get_mut<F, R>(&mut self, index: usize, f: F) -> R
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

    pub fn reset(&mut self, index: usize) {
        if let Some(item) = index
            .checked_sub(self.offset)
            .and_then(|index| self.data.get_mut(index))
        {
            *item = T::default();
            self.shrink(index);
        }
    }

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

    pub fn as_mut_slices<F, R>(&mut self, range: impl RangeBounds<usize>, f: F) -> R
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

    pub fn make_contiguous<F, R>(&mut self, range: impl RangeBounds<usize>, f: F) -> R
    where
        F: FnOnce(&mut [T]) -> R,
    {
        let range = range_bounds_to_range(range);
        if range.is_empty() {
            return f(&mut []);
        }

        self.grow_to_include(range.start);
        self.grow_to_include(range.end);

        let data_len = self.data.len();
        let mut slice = self.data.make_contiguous();
        if let Some(overlap) = (self.offset + data_len).checked_sub(range.end) {
            slice = &mut slice[..data_len - overlap];
        }
        if let Some(overlap) = range.start.checked_sub(self.offset) {
            slice = &mut slice[overlap..];
        }

        let ret = f(slice);
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
    fn test_set_get_mut_reset() {
        let mut range_vec = RangeVec::<u8>::new();
        range_vec.set(5, 10);
        range_vec.get_mut(10, |v| *v = 20);
        range_vec.set(3, 5);
        assert_eq!(range_vec[0], 0);
        assert_eq!(range_vec.get(5), &10);
        assert_eq!(range_vec.get(10), &20);
        assert_eq!(range_vec[12], 0);
        assert_eq!(range_vec.range(), Some(3..11));

        range_vec.get_mut(3, |v| *v = 0);
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
    fn test_debug() {
        let mut range_vec = RangeVec::<u8>::new();
        assert_eq!(format!("{:?}", range_vec), "RangeVec { <empty> }");
        range_vec.set(5, 1);
        range_vec.set(7, 2);
        range_vec.set(9, 3);
        assert_eq!(
            format!("{:?}", range_vec),
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
    fn test_as_mut_slices() {
        let mut range_vec = RangeVec::<i32>::new();
        range_vec.set(6, 6);
        range_vec.set(7, 7);
        range_vec.set(8, 8);
        range_vec.set(9, -1);
        range_vec.set(5, 5);
        range_vec.as_mut_slices(3..10, |left, right| {
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
    fn test_make_contiguous() {
        let mut range_vec = RangeVec::<i32>::new();
        range_vec.set(6, 6);
        range_vec.set(7, 7);
        range_vec.set(8, 8);
        range_vec.set(9, -1);
        range_vec.set(5, 5);
        range_vec.make_contiguous(3..10, |slice| {
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
