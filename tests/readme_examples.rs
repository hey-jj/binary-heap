//! Every code block in the README, copied in and run.
//!
//! The README is not pulled into rustdoc, so its examples get no doctest. Without this file a
//! README edit can ship a claim the crate does not honour. Each test below is one block, verbatim,
//! and its imports sit inside the function so the copy stays exact.

// The boxed comparator type is written out in the README, where a reader needs to see it. Naming
// it here would break the copy.
#![allow(clippy::type_complexity)]

#[test]
fn readme_use_block() {
    use binary_heap::{BinaryHeap, Max, MinHeap};
    use std::cmp::Ordering;

    let mut heap = BinaryHeap::from_vec(vec![3, 1, 4, 1, 5], Max);
    assert_eq!(heap.pop(), Some(5));
    assert_eq!(heap.into_sorted_vec(), vec![1, 1, 3, 4]);

    // An order picked at run time, which no Ord-bound heap can express.
    let by_last_digit: Box<dyn Fn(&i32, &i32) -> Ordering> =
        Box::new(|a, b| (a % 10).cmp(&(b % 10)));
    let mut runtime = BinaryHeap::from_vec(vec![25, 19, 33], by_last_digit);
    assert_eq!(runtime.pop(), Some(19));

    let mut least: MinHeap<i32> = vec![3, 1, 4].into_iter().collect();
    assert_eq!(least.pop(), Some(1));
}

#[test]
fn readme_stateful_comparator_block() {
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
}
