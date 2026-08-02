# binary-heap

A binary heap ordered by a comparator you supply at construction.

`std::collections::BinaryHeap` is a max-heap fixed to `Ord`. Reordering it means wrapping every
element in a newtype, which changes the type you store, and it cannot express an order chosen at
run time. Here the order is a value. It can be a closure, a function pointer, a `Box<dyn Fn>`
picked from a table, or a named type that implements `Compare`. `Max` and `Min` are two such
values.

```toml
[dependencies]
binary-heap = "0.2.0"
```

## Use

```rust
use binary_heap::{BinaryHeap, Max, MinHeap};
use std::cmp::Ordering;

let mut heap = BinaryHeap::from_vec(vec![3, 1, 4, 1, 5], Max);
assert_eq!(heap.pop(), Some(5));
assert_eq!(heap.into_sorted_vec(), vec![1, 1, 3, 4]);

// An order picked at run time.
let by_last_digit: Box<dyn Fn(&i32, &i32) -> Ordering> =
    Box::new(|a, b| (a % 10).cmp(&(b % 10)));
let mut runtime = BinaryHeap::from_vec(vec![25, 19, 33], by_last_digit);
assert_eq!(runtime.pop(), Some(19));

let mut least: MinHeap<i32> = vec![3, 1, 4].into_iter().collect();
assert_eq!(least.pop(), Some(1));
```

A comparator that carries state gets a name instead:

```rust
use binary_heap::{BinaryHeap, Compare};
use std::cmp::Ordering;

struct NearestTo {
    target: i32,
}

impl Compare<i32> for NearestTo {
    fn compare(&self, left: &i32, right: &i32) -> Ordering {
        let distance = |value: &i32| (value - self.target).unsigned_abs();
        distance(right).cmp(&distance(left))
    }
}

let heap = BinaryHeap::from_vec(vec![1, 40, 19, 26], NearestTo { target: 20 });
assert_eq!(heap.peek(), Some(&19));
```

## Cost

`push` is amortized O(log n), `pop` is O(log n), and `peek` and `peek_mut` are O(1). `from_vec`
heapifies in O(n) and reuses the vector's buffer. `into_sorted_vec` is O(n log n), sorts in the
heap's own buffer, and allocates nothing.

`peek_mut` returns a guard that sifts once when it drops, and only if the guard handed out a
`&mut`. Reading through the guard costs no comparisons. That is the k-way merge path: take the
next item from the reader at the root and put the reader back for one sift. A pop plus a push
would cost two.

## Limits

The heap is not stable. Elements that compare `Equal` come out in an unspecified order.

A comparator that is not a total order is accepted. Every operation still ends, stays in bounds,
and keeps every element exactly once. Only the order of the output breaks. The inconsistent
comparator tests in `tests/edge_cases.rs` drive such comparators and check exactly this. Detecting an
inconsistent comparator costs more than a quadratic scan, so this crate does not try.

There is no `serde` support. A serialized heap cannot carry its comparator, so decoding would have
to invent one and then read the elements under an order that did not arrange them.

`BinaryHeap` shadows `std::collections::BinaryHeap` under a glob import. Import it by path.

## Prior art

`binary-heap-plus` does this job too and has done for years. It is the established choice, and it
is the first place to look if you want the larger surface and the cargo features that come with
it.

This crate is an independent take on the same idea. The differences a caller notices are
`#![forbid(unsafe_code)]`, no dependencies, no cargo features, and one mechanism for order, so
`Max` and `Min` are ordinary `Compare` values.

## Design

The comparator is a private field, written only in the same expression that writes the element
buffer. There is no setter, no `&mut` access to it, no `as_mut_vec`, and no `From<Vec<T>>`. So no
call sequence can swap in an order that did not arrange the buffer, and that includes
`clone_from`, which stays at the trait default and replaces the whole value.

A comparator that changes its own answers is outside that guarantee. Nothing can hold a buffer to
an order the comparator itself stops giving.

## Requirements

`#![no_std]` with `extern crate alloc`, so an allocator is the only thing needed. The crate has no
dependencies and no cargo features. `#![forbid(unsafe_code)]`.

MSRV is 1.56.0, which is what edition 2021 needs. A CI job builds the library and runs the doctests
on that exact release. The full test suite runs on stable.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or the
[MIT license](LICENSE-MIT) at your option.

Unless you state otherwise, any contribution you intentionally submit for
inclusion in this crate, as defined in the Apache-2.0 license, is dual licensed
as above, with no additional terms or conditions.
