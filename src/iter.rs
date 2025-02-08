use std::{collections::vec_deque, iter::FusedIterator, ops::Range};

use crate::RangeVec;

pub struct Iter<'a, T: 'a> {
    iter_range: Range<usize>,
    filled_range: Range<usize>,
    filled_iter: vec_deque::Iter<'a, T>,
    default_item: &'a T,
}

impl<'a, T> Iter<'a, T> {
    pub(super) fn new(range_vec: &'a RangeVec<T>, range: Range<usize>) -> Self {
        Self {
            iter_range: range,
            filled_range: range_vec.offset..range_vec.offset + range_vec.data.len(),
            filled_iter: range_vec.data.iter(),
            default_item: &range_vec.default_item,
        }
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.iter_range.is_empty() {
            None
        } else if self.filled_range.contains(&self.iter_range.start) {
            self.iter_range.start += 1;
            self.filled_range.start += 1;
            self.filled_iter.next()
        } else {
            self.iter_range.start += 1;
            Some(self.default_item)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.iter_range.end - self.iter_range.start;
        (size, Some(size))
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.iter_range.is_empty() {
            None
        } else if self.filled_range.contains(&(self.iter_range.end - 1)) {
            self.iter_range.end -= 1;
            self.filled_range.end -= 1;
            self.filled_iter.next_back()
        } else {
            self.iter_range.end -= 1;
            Some(self.default_item)
        }
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {}

impl<'a, T> FusedIterator for Iter<'a, T> {}

#[test]
fn test_iter() {
    let mut range_vec = RangeVec::<u8>::new();
    range_vec.set(5, 1);
    range_vec.set(6, 2);
    range_vec.set(7, 3);

    let mut iter = range_vec.iter(3..9);
    assert_eq!(iter.len(), 6);
    assert_eq!(iter.next(), Some(&0));
    assert_eq!(iter.next(), Some(&0));
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next_back(), Some(&0));
    assert_eq!(iter.next_back(), Some(&3));
    assert_eq!(iter.len(), 1);
    assert_eq!(iter.next(), Some(&2));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
}
