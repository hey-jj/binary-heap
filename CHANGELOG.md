# Changelog

## 0.1.0

First release.

- `BinaryHeap<T, C>` ordered by a `Compare<T>` value given at construction.
- `Max` and `Min` comparators, with the `MaxHeap<T>` and `MinHeap<T>` aliases.
- A blanket `Compare` impl over `Fn(&T, &T) -> Ordering`, so closures, function pointers, and
  `Box<dyn Fn>` work with no wrapper.
- O(n) `from_vec`, in-place `into_sorted_vec`, and a `peek_mut` guard that sifts once and only
  after a write.
- `#![no_std]` with `extern crate alloc`. No dependencies and no cargo features.
