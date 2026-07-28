//! The comparison trait and the two comparators that cover `Ord`.

use core::cmp::Ordering;

/// An order over `T` held as a value.
///
/// Every `Fn(&T, &T) -> Ordering` implements this, so a closure, a function pointer, and a
/// `Box<dyn Fn(&T, &T) -> Ordering>` all work with no wrapper type. Implement it by hand when the
/// comparator carries state or wants a name.
///
/// # Examples
///
/// ```
/// use binary_heap::{BinaryHeap, Compare};
/// use std::cmp::Ordering;
///
/// struct ByLength;
///
/// impl Compare<String> for ByLength {
///     fn compare(&self, left: &String, right: &String) -> Ordering {
///         left.len().cmp(&right.len())
///     }
/// }
///
/// let mut heap = BinaryHeap::new(ByLength);
/// heap.push("a".to_string());
/// heap.push("aaa".to_string());
/// assert_eq!(heap.pop(), Some("aaa".to_string()));
/// ```
pub trait Compare<T: ?Sized> {
    /// Order `left` against `right`.
    ///
    /// The element that compares [`Ordering::Greater`] against every other element sits at the
    /// root, so it is the one [`pop`](crate::BinaryHeap::pop) returns first.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{Compare, Max};
    /// use std::cmp::Ordering;
    ///
    /// assert_eq!(Max.compare(&2, &1), Ordering::Greater);
    /// ```
    fn compare(&self, left: &T, right: &T) -> Ordering;
}

// `&F` and `Box<F>` also implement `Fn` when `F` does, so this one impl already covers borrowed
// and boxed comparators. Separate impls for them would overlap this one and the compiler would
// reject them.
impl<T: ?Sized, F> Compare<T> for F
where
    F: Fn(&T, &T) -> Ordering,
{
    fn compare(&self, left: &T, right: &T) -> Ordering {
        self(left, right)
    }
}

/// Orders by [`Ord`] and puts the greatest element at the root.
///
/// # Examples
///
/// ```
/// use binary_heap::{BinaryHeap, Max};
///
/// let heap = BinaryHeap::from_vec(vec![2, 9, 4], Max);
/// assert_eq!(heap.peek(), Some(&9));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Max;

/// Orders by [`Ord`] reversed and puts the least element at the root.
///
/// # Examples
///
/// ```
/// use binary_heap::{BinaryHeap, Min};
///
/// let heap = BinaryHeap::from_vec(vec![2, 9, 4], Min);
/// assert_eq!(heap.peek(), Some(&2));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Min;

impl<T: Ord + ?Sized> Compare<T> for Max {
    fn compare(&self, left: &T, right: &T) -> Ordering {
        left.cmp(right)
    }
}

impl<T: Ord + ?Sized> Compare<T> for Min {
    fn compare(&self, left: &T, right: &T) -> Ordering {
        right.cmp(left)
    }
}
