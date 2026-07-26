//! Shared test scaffolding: a seeded generator, the heap-property checker, and two oracles.
//!
//! Neither oracle shares code with the crate. The sorted model keeps a vector in order and never
//! computes a child index. The standard-library model is an implementation nobody here wrote.

#![allow(dead_code)]

use std::cmp::Ordering;
use std::collections::BinaryHeap as StdHeap;
use std::fmt;

use binary_heap::{BinaryHeap, Compare};

/// xorshift64. Every seed is fixed in the calling test, so every failure reproduces.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // A zero state is the one fixed point of xorshift, so force a nonzero one.
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform enough for a test corpus. `bound` must be nonzero.
    pub fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

/// Oracle two: the heap property, checked from outside the crate through the public API.
///
/// Stronger than comparing popped output, because it fails at the operation that corrupted the
/// array rather than at the pop that eventually reveals it.
pub fn assert_heap_property<T, C: Compare<T>>(
    heap: &BinaryHeap<T, C>,
    name: &str,
    seed: u64,
    step: usize,
) {
    let items = heap.as_slice();
    let cmp = heap.comparator();
    for child in 1..items.len() {
        let parent = (child - 1) / 2;
        // The message formats only on failure, so this stays allocation free in the hot loop.
        assert_ne!(
            cmp.compare(&items[parent], &items[child]),
            Ordering::Less,
            "{} seed {} step {}: element {} outranks its parent {}",
            name,
            seed,
            step,
            child,
            parent
        );
    }
}

/// Oracle A: a vector kept sorted under the comparator, greatest last.
///
/// `push` is a binary search plus an insert, `pop` takes the last element. O(n) per push and no
/// index arithmetic at all, so it shares nothing with a heap beyond the comparator.
pub struct SortedModel<T, C> {
    items: Vec<T>,
    cmp: C,
}

impl<T, C: Compare<T>> SortedModel<T, C> {
    pub fn new(cmp: C) -> Self {
        SortedModel {
            items: Vec::new(),
            cmp,
        }
    }

    pub fn push(&mut self, item: T) {
        let cmp = &self.cmp;
        let at = match self
            .items
            .binary_search_by(|probe| cmp.compare(probe, &item))
        {
            Ok(hit) => hit,
            Err(gap) => gap,
        };
        self.items.insert(at, item);
    }

    pub fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }

    pub fn peek(&self) -> Option<&T> {
        self.items.last()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

/// Element wrapper whose `Ord` calls a stored function pointer, so the standard-library heap can
/// be driven by the same comparator the crate is given.
#[derive(Clone)]
pub struct Keyed<T> {
    pub value: T,
    pub order: fn(&T, &T) -> Ordering,
}

/// Prints the value and skips the comparator. A function pointer prints as an address, which tells
/// a reader nothing, and deriving it would ask for `Debug` on a type holding two elided lifetimes.
impl<T: fmt::Debug> fmt::Debug for Keyed<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keyed").field("value", &self.value).finish()
    }
}

impl<T> PartialEq for Keyed<T> {
    fn eq(&self, other: &Self) -> bool {
        (self.order)(&self.value, &other.value) == Ordering::Equal
    }
}

impl<T> Eq for Keyed<T> {}

impl<T> PartialOrd for Keyed<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Keyed<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.order)(&self.value, &other.value)
    }
}

/// Oracle B: `std::collections::BinaryHeap` over [`Keyed`].
pub struct StdModel<T> {
    inner: StdHeap<Keyed<T>>,
    order: fn(&T, &T) -> Ordering,
}

impl<T: Ord> StdModel<T> {
    pub fn new(order: fn(&T, &T) -> Ordering) -> Self {
        StdModel {
            inner: StdHeap::new(),
            order,
        }
    }

    pub fn push(&mut self, value: T) {
        self.inner.push(Keyed {
            value,
            order: self.order,
        });
    }

    pub fn pop(&mut self) -> Option<T> {
        self.inner.pop().map(|keyed| keyed.value)
    }

    pub fn peek(&self) -> Option<&T> {
        self.inner.peek().map(|keyed| &keyed.value)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

/// The comparator shape the corpus tables hold, named so the tables stay readable.
pub type Order<T> = fn(&T, &T) -> Ordering;

/// A comparator chosen after the program starts. This is the shape a runtime flag produces.
pub type BoxedOrder<T> = Box<dyn Fn(&T, &T) -> Ordering>;

/// Comparators that are strict total orders whenever the elements are distinct.
pub const STRICT_ORDERS: [(&str, Order<i64>); 4] = [
    ("ascending", |a, b| a.cmp(b)),
    ("descending", |a, b| b.cmp(a)),
    ("low_bits_then_value", |a, b| {
        (a & 0xFF).cmp(&(b & 0xFF)).then_with(|| a.cmp(b))
    }),
    ("magnitude_then_value", |a, b| {
        a.unsigned_abs()
            .cmp(&b.unsigned_abs())
            .then_with(|| a.cmp(b))
    }),
];

/// Comparators with dense ties, used against a small element alphabet.
pub const TIED_ORDERS: [(&str, Order<i64>); 3] = [
    ("ascending", |a, b| a.cmp(b)),
    ("all_equal", |_, _| Ordering::Equal),
    ("by_parity", |a, b| (a & 1).cmp(&(b & 1))),
];

/// Distinct values whose high bits are random and whose low 12 bits are the draw index.
pub fn distinct_value(rng: &mut Rng, index: usize) -> i64 {
    assert!(index < 4096, "the tag field holds 12 bits");
    ((rng.next_u64() >> 20) as i64) * 4096 + index as i64
}

/// `floor(log2(n))` for `n >= 1`, used to check the comparison-count bounds.
pub fn floor_log2(n: usize) -> u32 {
    assert!(n > 0, "log2 of zero is undefined");
    usize::BITS - 1 - n.leading_zeros()
}
