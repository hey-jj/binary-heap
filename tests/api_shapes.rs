//! Every comparator shape the crate promises, built from a downstream crate.
//!
//! These run here rather than inside the library because the coherence question only has meaning
//! from outside. A caller must be able to write `impl Compare<T> for MyType` without colliding
//! with the blanket impl over `Fn`. If that ever stops holding, the design is gone, so it is a
//! test and not an assumption.

use std::cmp::Ordering;

use binary_heap::{BinaryHeap, Compare, MaxHeap, MinHeap};

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
