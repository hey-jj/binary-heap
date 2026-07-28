# Changelog

## 0.2.0

Breaking.

- `iter` returns `Iter<'_, T>` and `IntoIterator` yields `IntoIter<T>`, both defined by this
  crate. The public API no longer names `core::slice::Iter` or `alloc::vec::IntoIter`, so the
  backing storage stays private and can change without breaking callers. Iteration order is
  unchanged and still arbitrary. Code that wrote either old type into a signature needs the new
  name.

Other changes.

- `Max` and `Min` derive `Hash`, `PartialOrd`, and `Ord`. Both are unit structs, so the impls
  carry no data, and their absence kept a comparator out of any keyed or sorted collection.
- Dual licensed under MIT OR Apache-2.0. `LICENSE` is now `LICENSE-MIT` and `LICENSE-APACHE`
  sits beside it. 0.1.0 stays MIT only.
- README names `binary-heap-plus` as prior art and says what this crate does differently.

## 0.1.0

First release.

- `BinaryHeap<T, C>` ordered by a `Compare<T>` value given at construction.
- `Max` and `Min` comparators, with the `MaxHeap<T>` and `MinHeap<T>` aliases.
- A blanket `Compare` impl over `Fn(&T, &T) -> Ordering`, so closures, function pointers, and
  `Box<dyn Fn>` work with no wrapper.
- O(n) `from_vec`, in-place `into_sorted_vec`, and a `peek_mut` guard that sifts once and only
  after a write.
- `#![no_std]` with `extern crate alloc`. No dependencies and no cargo features.
