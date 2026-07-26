//! A binary heap ordered by a comparator you supply at construction.
//!
//! `std::collections::BinaryHeap` is a max-heap fixed to `Ord`. Reordering it means wrapping every
//! element in a newtype, which changes the type you store, and it cannot express an order chosen
//! at run time. Here the order is a value. It can be a closure, a function pointer, a
//! `Box<dyn Fn>` picked from a table, or a named type that implements [`Compare`]. [`Max`] and
//! [`Min`] are two such values over the one mechanism, not separate code.
//!
//! ```
//! use binary_heap::{BinaryHeap, Max, MinHeap};
//! use std::cmp::Ordering;
//!
//! let mut heap = BinaryHeap::from_vec(vec![3, 1, 4, 1, 5], Max);
//! assert_eq!(heap.pop(), Some(5));
//! assert_eq!(heap.into_sorted_vec(), vec![1, 1, 3, 4]);
//!
//! // An order picked at run time, which no Ord-bound heap can express.
//! let by_last_digit: Box<dyn Fn(&i32, &i32) -> Ordering> =
//!     Box::new(|a, b| (a % 10).cmp(&(b % 10)));
//! let mut runtime = BinaryHeap::from_vec(vec![25, 19, 33], by_last_digit);
//! assert_eq!(runtime.pop(), Some(19));
//!
//! let mut least: MinHeap<i32> = vec![3, 1, 4].into_iter().collect();
//! assert_eq!(least.pop(), Some(1));
//! ```
//!
//! # Limits
//!
//! The heap is not stable. Elements that compare `Equal` come out in an unspecified order.
//!
//! A comparator that is not a total order is accepted. Every operation still ends, stays in
//! bounds, and keeps every element exactly once. Ordering guarantees are the only thing lost.
//! Detecting an inconsistent comparator costs more than a quadratic scan, so this crate does not
//! try.
//!
//! There is no `serde` support. A serialized heap cannot carry its comparator, so decoding would
//! have to invent one and then read the elements under an order that did not arrange them.
//!
//! [`BinaryHeap`] shadows `std::collections::BinaryHeap` under a glob import. Import it by path.
//!
//! # Errors
//!
//! There are none. Every operation is total. [`BinaryHeap::pop`], [`BinaryHeap::peek`], and
//! [`BinaryHeap::peek_mut`] return `None` on an empty heap, and nothing else can fail.
//! [`BinaryHeap::with_capacity`], [`BinaryHeap::reserve`], and `extend` panic on a capacity that
//! no allocation can hold, exactly as [`Vec`](alloc::vec::Vec) does. A panic raised inside your
//! own comparator propagates and leaves the heap holding every element exactly once, possibly out
//! of order.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod compare;
mod heap;

pub use compare::{Compare, Max, Min};
pub use heap::{BinaryHeap, MaxHeap, MinHeap, PeekMut};
