//! Every comparator shape the crate promises, built from a downstream crate.
//!
//! These run here rather than inside the library because the coherence question only has meaning
//! from outside. A caller must be able to write `impl Compare<T> for MyType` without colliding
//! with the blanket impl over `Fn`. If that ever stops holding, the design is gone, so it is a
//! test and not an assumption.

use std::cmp::Ordering;
use std::fmt::Debug;
use std::iter::FusedIterator;

use binary_heap::{BinaryHeap, Compare, IntoIter, Iter, Max, MaxHeap, MinHeap};

/// A comparator chosen after the program starts, which is the shape a runtime flag produces.
type BoxedOrder = Box<dyn Fn(&i32, &i32) -> Ordering>;

/// A comparator with no fields, the shape `Max` and `Min` use.
struct ByLowBits;

impl Compare<i32> for ByLowBits {
    fn compare(&self, left: &i32, right: &i32) -> Ordering {
        (left & 0xF).cmp(&(right & 0xF))
    }
}

/// A comparator that holds state, which the order can then depend on.
struct NearestTo {
    target: i32,
}

impl Compare<i32> for NearestTo {
    fn compare(&self, left: &i32, right: &i32) -> Ordering {
        // Nearer to the target ranks higher, so the root is the nearest element.
        let distance = |value: &i32| (value - self.target).unsigned_abs();
        distance(right).cmp(&distance(left))
    }
}

#[test]
fn a_named_comparator_type_does_not_collide_with_the_blanket_fn_impl() {
    let mut heap = BinaryHeap::new(ByLowBits);
    heap.push(0x21);
    heap.push(0x1F);
    heap.push(0x40);
    assert_eq!(heap.pop(), Some(0x1F));
}

#[test]
fn a_stateful_comparator_type_orders_by_its_own_field() {
    let heap = BinaryHeap::from_vec(vec![1, 40, 19, 26], NearestTo { target: 20 });
    assert_eq!(heap.peek(), Some(&19));
    // Ascending under this comparator runs from farthest to nearest.
    assert_eq!(heap.into_sorted_vec(), vec![40, 1, 26, 19]);
}

#[test]
fn a_closure_a_function_pointer_and_a_boxed_dyn_fn_all_build_a_heap() {
    let mut from_closure = BinaryHeap::new(|a: &i32, b: &i32| a.cmp(b));
    from_closure.push(3);
    from_closure.push(8);
    assert_eq!(from_closure.pop(), Some(8));

    let pointer: fn(&i32, &i32) -> Ordering = |a, b| b.cmp(a);
    let mut from_pointer = BinaryHeap::new(pointer);
    from_pointer.push(3);
    from_pointer.push(8);
    assert_eq!(from_pointer.pop(), Some(3));

    // The runtime-dispatch shape. The order is chosen from a table after the program starts, which
    // no `Ord`-bound heap can express.
    let table: [BoxedOrder; 2] = [
        Box::new(|a, b| a.cmp(b)),
        Box::new(|a, b| (a % 10).cmp(&(b % 10))),
    ];
    let mut chosen = BinaryHeap::new(table.into_iter().nth(1).unwrap());
    chosen.push(37);
    chosen.push(45);
    assert_eq!(chosen.pop(), Some(37));
}

#[test]
fn the_type_aliases_default_and_collect() {
    let mut least = MinHeap::<i32>::default();
    least.push(9);
    least.push(2);
    assert_eq!(least.pop(), Some(2));

    let greatest: MaxHeap<i32> = vec![3, 8, 5].into_iter().collect();
    assert_eq!(greatest.into_sorted_vec(), vec![3, 5, 8]);
}

#[test]
fn an_element_type_without_clone_supports_everything_but_cloning() {
    #[derive(Debug, PartialEq)]
    struct Opaque(u8);

    let mut heap = BinaryHeap::from_vec(
        vec![Opaque(3), Opaque(9), Opaque(1)],
        |a: &Opaque, b: &Opaque| a.0.cmp(&b.0),
    );

    assert_eq!(heap.peek(), Some(&Opaque(9)));
    heap.push(Opaque(5));
    assert_eq!(heap.pop(), Some(Opaque(9)));
    assert_eq!(heap.len(), 3);
    assert_eq!(heap.iter().count(), 3);
    assert_eq!(
        heap.into_sorted_vec(),
        vec![Opaque(1), Opaque(3), Opaque(5)]
    );
}

#[test]
fn a_heap_of_send_elements_crosses_a_thread_boundary() {
    // `Send` comes from the fields, so this compiles only if both the buffer and the comparator
    // carry it. No hand-written impl says so.
    let heap = BinaryHeap::from_vec(vec![4i64, 1, 9], |a: &i64, b: &i64| a.cmp(b));
    let handle = std::thread::spawn(move || heap.into_sorted_vec());
    assert_eq!(handle.join().unwrap(), vec![1, 4, 9]);
}

/// Names every trait the buffer's own iterators handed callers before the newtypes existed.
///
/// Passing an iterator through this is the check that wrapping took nothing away.
fn every_capability<I>(iterator: I) -> I
where
    I: DoubleEndedIterator + ExactSizeIterator + FusedIterator + Debug,
{
    iterator
}

#[test]
fn iter_returns_the_crate_type_and_keeps_every_capability_the_slice_iterator_gave() {
    let heap = BinaryHeap::from_vec(vec![1, 5, 3], Max);
    // Heap-array order, which is arbitrary. Every expectation below reads it from the heap rather
    // than restating it, so this tests the iterator and not the arrangement.
    let order: Vec<i32> = heap.as_slice().to_vec();

    // The annotation is the assertion. `slice::Iter` would not satisfy it.
    let mut borrowed: Iter<'_, i32> = every_capability(heap.iter());
    assert_eq!(borrowed.len(), 3);
    assert_eq!(borrowed.size_hint(), (3, Some(3)));
    assert_eq!(
        format!("{:?}", borrowed),
        format!("Iter({:?})", heap.as_slice())
    );

    // `Clone` holds for every element type, as it does on the wrapped iterator.
    assert_eq!(borrowed.clone().copied().collect::<Vec<i32>>(), order);
    let mut backward = order.clone();
    backward.reverse();
    assert_eq!(
        borrowed.clone().rev().copied().collect::<Vec<i32>>(),
        backward
    );

    assert_eq!(borrowed.next_back(), order.last());
    assert_eq!(borrowed.next(), order.first());
    assert_eq!(borrowed.count(), 1);

    // `nth` skips from the front, counting from the element `next` would return.
    assert_eq!(heap.iter().nth(1), order.get(1));

    // `&heap` iterates through the same type.
    let by_reference: Iter<'_, i32> = (&heap).into_iter();
    assert_eq!(by_reference.last(), order.last());
}

#[test]
fn into_iter_returns_the_crate_type_and_keeps_every_capability_the_vector_iterator_gave() {
    let heap = BinaryHeap::from_vec(vec![1, 5, 3], Max);
    let order: Vec<i32> = heap.as_slice().to_vec();

    // The annotation is the assertion. `alloc::vec::IntoIter` would not satisfy it.
    let mut owned: IntoIter<i32> = every_capability(heap.into_iter());
    assert_eq!(owned.len(), 3);
    assert_eq!(owned.size_hint(), (3, Some(3)));
    assert_eq!(format!("{:?}", owned), format!("IntoIter({:?})", order));

    // `Clone` carries the wrapped iterator's own bound, so it asks for `i32: Clone`.
    assert_eq!(owned.clone().collect::<Vec<i32>>(), order);

    assert_eq!(owned.next_back(), order.last().copied());
    // `nth` skips from the front, counting from the element `next` would return.
    assert_eq!(owned.nth(1), order.get(1).copied());
    assert_eq!(owned.count(), 0);
}
