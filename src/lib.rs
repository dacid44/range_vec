use std::{
    collections::VecDeque,
    fmt::Debug,
    ops::{Bound, Range, RangeBounds},
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
        } else if index >= self.offset + self.data.len() {
            // index >= offset + length: grow right and insert value
            let additional = index - (self.offset + self.data.len()) + 1;
            self.data.reserve(additional);
            for _ in 0..additional {
                self.data.push_back(T::default());
            }
        }
    }

    pub fn get<F, R>(&self, index: usize, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        match index
            .checked_sub(self.offset)
            .and_then(|index| self.data.get(index))
        {
            Some(item) => f(item),
            None => f(&Default::default()),
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
            .rev()
            .position(|item| item != &self.default_item)
        {
            Some(index) => {
                let Some(index) = index.checked_sub(1) else {
                    return;
                };
                self.data.drain(self.data.len() - index..);
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
}

impl<T> RangeVec<T>
where
    T: Default + Clone,
{
    pub fn get_owned(&self, index: usize) -> T {
        index
            .checked_sub(self.offset)
            .and_then(|index| self.data.get(index))
            .cloned()
            .unwrap_or_default()
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
    fn test_get() {
        let range_vec = RangeVec::<u8>::new();
        assert_eq!(range_vec.get(0, |v| *v), 0);
        assert_eq!(range_vec.get_owned(1_000_000), 0);
        assert!(range_vec.is_empty());
    }

    #[test]
    fn test_set_get_mut_reset() {
        let mut range_vec = RangeVec::<u8>::new();
        range_vec.set(5, 10);
        range_vec.get_mut(10, |v| *v = 20);
        assert_eq!(range_vec.get_owned(0), 0);
        assert_eq!(range_vec.get(5, |v| *v), 10);
        assert_eq!(range_vec.get_owned(10), 20);
        assert_eq!(range_vec.get_owned(12), 0);
        assert_eq!(range_vec.range(), Some(5..11));

        range_vec.get_mut(5, |v| *v = 0);
        assert_eq!(range_vec.range(), Some(10..11));
        range_vec.set(10, 0);
        assert!(range_vec.is_empty());

        range_vec.set(20, 100);
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
        assert_eq!(range_vec.get_owned(7), 2);
    }
}
