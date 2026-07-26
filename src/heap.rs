//! The heap and the guard [`BinaryHeap::peek_mut`] returns.

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use core::mem;
use core::ops::{Deref, DerefMut};
use core::slice;

use crate::compare::{Compare, Max, Min};

/// A binary heap whose order comes from a comparator held as a value.
///
/// The comparator is written once, in the same expression that writes the element buffer, and
/// there is no way to change it afterwards. No setter, no `&mut` access to it, no `as_mut_vec`,
/// and no `From<Vec<T>>`. So no call sequence can swap in an order that did not arrange the
/// buffer. A comparator that changes its own answers, through interior mutability or through
/// state it reads elsewhere, is outside that guarantee: it can leave the buffer arranged under
/// answers it no longer gives.
///
/// [`as_slice`](Self::as_slice) and [`comparator`](Self::comparator) are both public, which lets a
/// caller check the heap property from outside this crate.
///
/// Equal elements come out in an unspecified order. The heap is not stable.
///
/// # Examples
///
/// ```
/// use binary_heap::{BinaryHeap, Min};
///
/// let mut heap = BinaryHeap::new(Min);
/// heap.push(5);
/// heap.push(1);
/// heap.push(3);
/// assert_eq!(heap.pop(), Some(1));
/// assert_eq!(heap.into_sorted_vec(), vec![5, 3]);
/// ```
#[derive(Clone)]
pub struct BinaryHeap<T, C> {
    data: Vec<T>,
    // The complete list of writes to this field is `new`, `with_capacity`, `from_vec`,
    // `Default::default`, `FromIterator::from_iter`, and the derived `Clone::clone`. Each of them
    // writes `data` in the same expression. `clone_from` is deliberately left at the trait
    // default, `*self = source.clone()`, which replaces the whole value. A hand-written body that
    // copied only the elements would leave the receiver reading them under its own comparator.
    cmp: C,
}

/// A heap that yields the greatest element first, under [`Ord`].
pub type MaxHeap<T> = BinaryHeap<T, Max>;

/// A heap that yields the least element first, under [`Ord`].
pub type MinHeap<T> = BinaryHeap<T, Min>;

impl<T, C: Compare<T>> BinaryHeap<T, C> {
    /// Builds an empty heap ordered by `cmp`. No allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let heap: BinaryHeap<i32, Max> = BinaryHeap::new(Max);
    /// assert_eq!(heap.len(), 0);
    /// assert_eq!(heap.capacity(), 0);
    /// ```
    #[must_use]
    pub fn new(cmp: C) -> Self {
        BinaryHeap {
            data: Vec::new(),
            cmp,
        }
    }

    /// Builds an empty heap ordered by `cmp` with room for `capacity` elements.
    ///
    /// `with_capacity(0, cmp)` allocates nothing and matches [`new`](Self::new).
    ///
    /// # Panics
    ///
    /// Panics if the requested capacity exceeds what an allocation can hold, the same condition
    /// under which [`Vec::with_capacity`] panics.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let heap: BinaryHeap<i32, Max> = BinaryHeap::with_capacity(0, Max);
    /// assert_eq!(heap.capacity(), 0);
    /// ```
    #[must_use]
    pub fn with_capacity(capacity: usize, cmp: C) -> Self {
        BinaryHeap {
            data: Vec::with_capacity(capacity),
            cmp,
        }
    }

    /// Turns an existing vector into a heap in O(n), reusing its buffer.
    ///
    /// This is the bulk path. Building the same heap with n calls to [`push`](Self::push) costs
    /// O(n log n).
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let heap = BinaryHeap::from_vec(vec![1, 7, 3], Max);
    /// assert_eq!(heap.peek(), Some(&7));
    /// assert_eq!(heap.len(), 3);
    /// ```
    #[must_use]
    pub fn from_vec(vec: Vec<T>, cmp: C) -> Self {
        let mut heap = BinaryHeap { data: vec, cmp };
        heap.rebuild();
        heap
    }

    /// Adds an element. Amortized O(log n).
    ///
    /// # Panics
    ///
    /// Panics if the buffer has to grow past what an allocation can hold, the same condition
    /// under which [`Vec::push`] panics. Reaching it takes a heap already holding `usize::MAX`
    /// zero-sized elements, or a growth step past `isize::MAX` bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let mut heap = BinaryHeap::new(Max);
    /// heap.push(4);
    /// assert_eq!(heap.peek(), Some(&4));
    /// ```
    pub fn push(&mut self, item: T) {
        self.data.push(item);
        self.sift_up(self.data.len() - 1);
    }

    /// Removes and returns the root, or `None` when the heap is empty. O(log n).
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let mut heap = BinaryHeap::from_vec(vec![1, 2], Max);
    /// assert_eq!(heap.pop(), Some(2));
    /// assert_eq!(heap.pop(), Some(1));
    /// assert_eq!(heap.pop(), None);
    /// ```
    pub fn pop(&mut self) -> Option<T> {
        // `checked_sub` gives the empty case and the last index in one step, with no subtraction
        // that could wrap.
        let last = self.data.len().checked_sub(1)?;
        self.data.swap(0, last);
        // Sift over `..last` first, then remove. The outgoing element stays in the buffer while
        // the comparator runs, so a panic inside it leaves the heap holding that element instead
        // of dropping it during the unwind.
        self.sift_down(0, last);
        self.data.pop()
    }

    /// Returns a reference to the root, or `None` when the heap is empty. O(1).
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Min};
    ///
    /// let heap = BinaryHeap::from_vec(vec![8, 2, 5], Min);
    /// assert_eq!(heap.peek(), Some(&2));
    /// ```
    #[must_use]
    pub fn peek(&self) -> Option<&T> {
        self.data.first()
    }

    /// Returns a guard over the root, or `None` when the heap is empty. O(1).
    ///
    /// Writing through the guard costs one sift when the guard drops, against the two a
    /// [`pop`](Self::pop) plus a [`push`](Self::push) would cost. Reading through it costs
    /// nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let mut heap = BinaryHeap::from_vec(vec![9, 4, 1], Max);
    /// if let Some(mut root) = heap.peek_mut() {
    ///     *root = 0;
    /// }
    /// assert_eq!(heap.peek(), Some(&4));
    /// ```
    pub fn peek_mut(&mut self) -> Option<PeekMut<'_, T, C>> {
        if self.data.is_empty() {
            None
        } else {
            Some(PeekMut {
                heap: self,
                sift: false,
            })
        }
    }

    /// Consumes the heap and returns its elements ascending under the comparator.
    ///
    /// The work happens in the heap's own buffer, so the returned vector keeps the heap's
    /// capacity and nothing new is allocated. O(n log n).
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let heap = BinaryHeap::from_vec(vec![3, 1, 2], Max);
    /// assert_eq!(heap.into_sorted_vec(), vec![1, 2, 3]);
    /// ```
    #[must_use]
    pub fn into_sorted_vec(mut self) -> Vec<T> {
        // Every value of a zero-sized type is the same value, so the buffer is already sorted.
        // The loop below would otherwise run once per element, and `vec![(); usize::MAX]` is a
        // legal vector that costs nothing to build.
        if mem::size_of::<T>() == 0 {
            return self.data;
        }
        // Repeatedly park the root just past the shrinking heap. The greatest element lands
        // last, so the finished array reads ascending.
        let mut end = self.data.len();
        while end > 1 {
            end -= 1;
            self.data.swap(0, end);
            self.sift_down(0, end);
        }
        self.data
    }

    /// Returns the comparator this heap was built with.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Compare, Max};
    /// use std::cmp::Ordering;
    ///
    /// let heap: BinaryHeap<i32, Max> = BinaryHeap::new(Max);
    /// assert_eq!(heap.comparator().compare(&1, &2), Ordering::Less);
    /// ```
    #[must_use]
    pub fn comparator(&self) -> &C {
        &self.cmp
    }

    /// Restores the heap property over the whole buffer, bottom up, in O(n).
    fn rebuild(&mut self) {
        // Every value of a zero-sized type is the same value, so any arrangement already
        // satisfies the property. The walk below would otherwise run `len / 2` times, and
        // `vec![(); usize::MAX]` is a legal vector that costs nothing to build.
        if mem::size_of::<T>() == 0 {
            return;
        }
        let mut node = self.data.len() / 2;
        while node > 0 {
            node -= 1;
            self.sift_down(node, self.data.len());
        }
    }

    /// Moves the element at `pos` toward the root until its parent outranks it.
    fn sift_up(&mut self, mut pos: usize) {
        // `pos` strictly decreases every iteration, so the loop ends after at most log2(len)
        // steps no matter what the comparator returns. `pos > 0` also makes `pos - 1` safe.
        while pos > 0 {
            let parent = (pos - 1) / 2;
            if self.cmp.compare(&self.data[pos], &self.data[parent]) != Ordering::Greater {
                break;
            }
            self.data.swap(pos, parent);
            pos = parent;
        }
    }

    /// Moves the element at `pos` toward the leaves of the sub-array `..end`.
    fn sift_down(&mut self, mut pos: usize, end: usize) {
        // `pos < end / 2` is exactly "this node has a left child", and it also bounds
        // `2 * pos + 2 <= end`. So neither child index can overflow, for any `end` up to
        // `usize::MAX`, in debug or in release. `pos` strictly increases, so the loop ends.
        while pos < end / 2 {
            let left = 2 * pos + 1;
            let right = left + 1;
            let mut child = left;
            if right < end
                && self.cmp.compare(&self.data[right], &self.data[left]) == Ordering::Greater
            {
                child = right;
            }
            if self.cmp.compare(&self.data[child], &self.data[pos]) != Ordering::Greater {
                break;
            }
            self.data.swap(pos, child);
            pos = child;
        }
    }
}

impl<T, C> BinaryHeap<T, C> {
    /// Returns the number of elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// assert_eq!(BinaryHeap::from_vec(vec![1, 2, 2], Max).len(), 3);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` when the heap holds no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let mut heap = BinaryHeap::new(Max);
    /// assert!(heap.is_empty());
    /// heap.push(1);
    /// assert!(!heap.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns how many elements the heap can hold before it reallocates.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let heap: BinaryHeap<i32, Max> = BinaryHeap::with_capacity(10, Max);
    /// assert!(heap.capacity() >= 10);
    /// ```
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Reserves room for at least `additional` more elements.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds what an allocation can hold, the same condition under
    /// which [`Vec::reserve`] panics.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let mut heap: BinaryHeap<i32, Max> = BinaryHeap::new(Max);
    /// heap.reserve(32);
    /// assert!(heap.capacity() >= 32);
    /// assert_eq!(heap.len(), 0);
    /// ```
    pub fn reserve(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    /// Drops every element and keeps the allocated capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let mut heap = BinaryHeap::from_vec(vec![1, 2, 3], Max);
    /// let capacity = heap.capacity();
    /// heap.clear();
    /// assert_eq!(heap.len(), 0);
    /// assert_eq!(heap.capacity(), capacity);
    /// ```
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Returns the elements in heap-array order. Index 0 is the root.
    ///
    /// The rest of the order is an implementation detail beyond the heap property, which is that
    /// no element outranks its parent.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let heap = BinaryHeap::from_vec(vec![1, 5, 3], Max);
    /// assert_eq!(heap.as_slice()[0], 5);
    /// ```
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    /// Consumes the heap and returns its buffer in heap-array order, unsorted.
    ///
    /// Pair it with [`from_vec`](BinaryHeap::from_vec) to get the bulk operations this crate does
    /// not provide, such as removing elements by predicate.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let heap = BinaryHeap::from_vec(vec![1, 5, 3, 9], Max);
    ///
    /// let mut kept = heap.into_vec();
    /// kept.retain(|value| value % 3 != 0);
    ///
    /// let heap = BinaryHeap::from_vec(kept, Max);
    /// assert_eq!(heap.into_sorted_vec(), vec![1, 5]);
    /// ```
    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        self.data
    }

    /// Iterates the elements in heap-array order, unsorted.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let heap = BinaryHeap::from_vec(vec![1, 5, 3], Max);
    /// assert_eq!(heap.iter().sum::<i32>(), 9);
    /// ```
    pub fn iter(&self) -> slice::Iter<'_, T> {
        self.data.iter()
    }
}

impl<T, C: Compare<T> + Default> Default for BinaryHeap<T, C> {
    /// Builds an empty heap ordered by `C::default()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::MinHeap;
    ///
    /// let mut heap = MinHeap::<i32>::default();
    /// heap.push(4);
    /// heap.push(2);
    /// assert_eq!(heap.pop(), Some(2));
    /// ```
    fn default() -> Self {
        Self::new(C::default())
    }
}

impl<T, C: Compare<T>> Extend<T> for BinaryHeap<T, C> {
    /// Pushes every element of `iter`, one at a time, after one reserve.
    ///
    /// Cost is O(m log(n + m)) for m added elements. Loading a whole collection is cheaper
    /// through [`from_vec`](BinaryHeap::from_vec), which is O(n).
    ///
    /// # Panics
    ///
    /// Panics if the iterator's lower size hint pushes the capacity past what an allocation can
    /// hold, the same condition under which [`Vec::reserve`] panics.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let mut heap = BinaryHeap::from_vec(vec![1, 2], Max);
    /// heap.extend(vec![7, 3]);
    /// assert_eq!(heap.into_sorted_vec(), vec![1, 2, 3, 7]);
    /// ```
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        // One reserve up front, so a bulk extend does not walk the growth curve.
        self.data.reserve(iter.size_hint().0);
        for item in iter {
            self.push(item);
        }
    }
}

impl<T, C: Compare<T> + Default> FromIterator<T> for BinaryHeap<T, C> {
    /// Collects into a heap ordered by `C::default()`, heapifying once in O(n).
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::MaxHeap;
    ///
    /// let heap: MaxHeap<i32> = vec![3, 8, 5].into_iter().collect();
    /// assert_eq!(heap.peek(), Some(&8));
    /// ```
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::from_vec(iter.into_iter().collect(), C::default())
    }
}

impl<T, C> IntoIterator for BinaryHeap<T, C> {
    type Item = T;
    type IntoIter = alloc::vec::IntoIter<T>;

    /// Yields the elements in heap-array order, unsorted. For sorted output call
    /// [`into_sorted_vec`](BinaryHeap::into_sorted_vec) first.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let heap = BinaryHeap::from_vec(vec![1, 5, 3], Max);
    /// assert_eq!(heap.into_iter().sum::<i32>(), 9);
    /// ```
    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl<'a, T, C> IntoIterator for &'a BinaryHeap<T, C> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    /// Yields references in heap-array order, unsorted.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let heap = BinaryHeap::from_vec(vec![1, 5, 3], Max);
    /// let mut seen: Vec<i32> = (&heap).into_iter().copied().collect();
    /// seen.sort_unstable();
    /// assert_eq!(seen, vec![1, 3, 5]);
    /// ```
    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

// Hand-written so a heap prints even when the comparator has no `Debug`. A comparator is often a
// closure, and closures have none.
impl<T: fmt::Debug, C> fmt::Debug for BinaryHeap<T, C> {
    /// Prints the elements in heap-array order, unsorted.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// // The comparator is a closure, which has no `Debug`, and the heap still prints.
    /// let heap = BinaryHeap::from_vec(vec![1, 5, 3], |a: &i32, b: &i32| a.cmp(b));
    /// assert_eq!(format!("{:?}", heap), "[5, 1, 3]");
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.data.iter()).finish()
    }
}

/// A guard over the root, returned by [`BinaryHeap::peek_mut`].
///
/// [`Deref`] reads the root. [`DerefMut`] hands out `&mut T` and marks the root as changed, and
/// the guard sifts it down when it drops. A guard that only read costs no comparisons.
///
/// Leaking the guard with [`core::mem::forget`] skips that sift. The heap keeps every element and
/// stays usable, and the heap property can stay violated until the next operation restores it.
///
/// A guard that handed out `&mut` calls your comparator while it drops. If that call panics while
/// the guard is already dropping during another unwind, the process aborts, as a panic during a
/// panic always does.
///
/// # Examples
///
/// ```
/// use binary_heap::{BinaryHeap, Max, PeekMut};
///
/// let mut heap = BinaryHeap::from_vec(vec![3, 1, 2], Max);
///
/// // Read only, no sift.
/// assert_eq!(*heap.peek_mut().unwrap(), 3);
///
/// // Write, then one sift on drop.
/// if let Some(mut root) = heap.peek_mut() {
///     *root = 0;
/// }
/// assert_eq!(heap.peek(), Some(&2));
///
/// // Take the root instead.
/// let taken = heap.peek_mut().map(PeekMut::pop);
/// assert_eq!(taken, Some(2));
/// ```
// The `C: Compare<T>` bound sits on the struct because a `Drop` impl must repeat its type's
// bounds, and the drop glue needs the comparator to sift. Nothing else in the crate puts a bound
// on a struct definition.
pub struct PeekMut<'a, T, C: Compare<T>> {
    heap: &'a mut BinaryHeap<T, C>,
    // Set by `DerefMut` only, so a guard that never wrote skips the sift in `Drop`.
    sift: bool,
}

impl<T, C: Compare<T>> PeekMut<'_, T, C> {
    /// Removes and returns the root.
    ///
    /// An associated function, not a method, so it cannot shadow a `pop` that the element type
    /// reaches through [`Deref`].
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max, PeekMut};
    ///
    /// let mut heap = BinaryHeap::from_vec(vec![1, 6], Max);
    /// let root = PeekMut::pop(heap.peek_mut().unwrap());
    /// assert_eq!(root, 6);
    /// assert_eq!(heap.len(), 1);
    /// ```
    pub fn pop(mut this: Self) -> T {
        // `pop` restores the property itself, so the drop must not sift a second time.
        this.sift = false;
        match this.heap.pop() {
            Some(root) => root,
            // Unreachable. A guard is built only over a non-empty heap and holds the only borrow
            // of it, so nothing can empty the heap while the guard is alive.
            None => unreachable(),
        }
    }
}

impl<T, C: Compare<T>> Deref for PeekMut<'_, T, C> {
    type Target = T;

    fn deref(&self) -> &T {
        match self.heap.data.first() {
            Some(root) => root,
            // Unreachable, for the same reason as in `PeekMut::pop`.
            None => unreachable(),
        }
    }
}

impl<T, C: Compare<T>> DerefMut for PeekMut<'_, T, C> {
    fn deref_mut(&mut self) -> &mut T {
        // Handing out `&mut T` is the only signal that the root may have changed.
        self.sift = true;
        match self.heap.data.first_mut() {
            Some(root) => root,
            None => unreachable(),
        }
    }
}

impl<T, C: Compare<T>> Drop for PeekMut<'_, T, C> {
    fn drop(&mut self) {
        if self.sift {
            let end = self.heap.data.len();
            self.heap.sift_down(0, end);
        }
    }
}

impl<T: fmt::Debug, C: Compare<T>> fmt::Debug for PeekMut<'_, T, C> {
    /// Prints the root.
    ///
    /// # Examples
    ///
    /// ```
    /// use binary_heap::{BinaryHeap, Max};
    ///
    /// let mut heap = BinaryHeap::from_vec(vec![1, 5, 3], Max);
    /// assert_eq!(format!("{:?}", heap.peek_mut().unwrap()), "PeekMut(5)");
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PeekMut").field(&**self).finish()
    }
}

/// Reports a state the type system allows but no call sequence reaches.
///
/// Every caller sits behind a proof that the heap is non-empty. Routing them all here keeps one
/// message and one place for a reviewer to check.
#[cold]
fn unreachable() -> ! {
    panic!("a peek guard exists only over a non-empty heap")
}
