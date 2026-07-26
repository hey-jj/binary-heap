//! A cloned heap carries its comparator, checked over a full drain.
//!
//! A copy that takes the elements and keeps the receiver's comparator reads those elements under
//! an order that did not arrange them. Nothing panics. `peek` still agrees, and so does the first
//! pop, because both arrays have the same root. Divergence starts at the second pop. Every test
//! here drains to empty so it sees that far.

mod common;

use std::cmp::Ordering;

use binary_heap::{BinaryHeap, Compare, Max, Min};
use common::{Order, Rng};

// Two function pointers of one type, so two heaps of one type can carry two different orders.
fn ascending(left: &i64, right: &i64) -> Ordering {
    left.cmp(right)
}

fn descending(left: &i64, right: &i64) -> Ordering {
    right.cmp(left)
}

/// Same closure type on every call, different captured state, so the same trick works for
/// closures.
fn by_bias(bias: i64) -> impl Fn(&i64, &i64) -> Ordering + Clone {
    move |left: &i64, right: &i64| (left ^ bias).cmp(&(right ^ bias))
}

/// A comparator that holds a field, which is the shape a closure cannot be given a name.
#[derive(Clone)]
struct ByRotation {
    offset: i64,
}

impl Compare<i64> for ByRotation {
    fn compare(&self, left: &i64, right: &i64) -> Ordering {
        let rotate = |value: &i64| value.wrapping_add(self.offset).rem_euclid(97);
        rotate(left).cmp(&rotate(right))
    }
}

fn drain<C: Compare<i64>>(mut heap: BinaryHeap<i64, C>) -> Vec<i64> {
    let mut out = Vec::new();
    while let Some(item) = heap.pop() {
        out.push(item);
    }
    out
}

/// The five distinct values used throughout. Under `ascending` they drain 9, 7, 5, 3, 1, and a
/// copy that kept a `descending` comparator returns 9 and then 1.
const VALUES: [i64; 5] = [5, 1, 9, 3, 7];

#[test]
fn clone_from_adopts_source_comparator() {
    let mut receiver = BinaryHeap::from_vec(VALUES.to_vec(), descending as Order<i64>);
    let source = BinaryHeap::from_vec(VALUES.to_vec(), ascending as Order<i64>);

    receiver.clone_from(&source);

    // A pair the two orders disagree on.
    assert_eq!(receiver.comparator().compare(&1, &2), Ordering::Less);
    // And agreement with the source across the whole corpus of pairs.
    for left in VALUES {
        for right in VALUES {
            assert_eq!(
                receiver.comparator().compare(&left, &right),
                source.comparator().compare(&left, &right),
                "pair {left} {right}"
            );
        }
    }
}

#[test]
fn clone_from_matches_assignment_over_full_drain() {
    let mut receiver = BinaryHeap::from_vec(VALUES.to_vec(), descending as Order<i64>);
    let source = BinaryHeap::from_vec(VALUES.to_vec(), ascending as Order<i64>);

    receiver.clone_from(&source);
    let mut assigned = source.clone();

    // The first pop agrees even when the comparator was dropped, because both arrays share a
    // root. The second pop is where a dropped comparator first shows.
    assert_eq!(receiver.pop(), Some(9));
    assert_eq!(assigned.pop(), Some(9));
    assert_eq!(receiver.pop(), Some(7));
    assert_eq!(assigned.pop(), Some(7));

    assert_eq!(drain(receiver), vec![5, 3, 1]);
    assert_eq!(drain(assigned), vec![5, 3, 1]);
}

#[test]
fn clone_from_with_closure_comparator_drains_like_source() {
    let mut receiver = BinaryHeap::from_vec(VALUES.to_vec(), by_bias(0));
    let source = BinaryHeap::from_vec(VALUES.to_vec(), by_bias(0b1111));

    receiver.clone_from(&source);

    // Under the bias the order is by `value ^ 15`, which ranks 1 highest and 9 lowest.
    assert_eq!(drain(receiver), vec![1, 3, 5, 7, 9]);
    assert_eq!(drain(source.clone()), vec![1, 3, 5, 7, 9]);
}

#[test]
fn clone_from_with_stateful_comparator_drains_like_source() {
    let mut receiver = BinaryHeap::from_vec(VALUES.to_vec(), ByRotation { offset: 0 });
    let source = BinaryHeap::from_vec(VALUES.to_vec(), ByRotation { offset: 90 });

    receiver.clone_from(&source);

    // Rotating by 90 modulo 97 sends 5 to 95 and 7 to 0, so 5 leads and 7 trails.
    assert_eq!(drain(receiver), vec![5, 3, 1, 9, 7]);
    assert_eq!(drain(source.clone()), vec![5, 3, 1, 9, 7]);
}

#[test]
fn clone_from_inside_vec_of_heaps() {
    // `Vec::clone_from` reuses the slots it already has and clones into them one by one, so this
    // reaches the same path through one more layer.
    let mut receiver = vec![BinaryHeap::from_vec(
        VALUES.to_vec(),
        descending as Order<i64>,
    )];
    let source = vec![BinaryHeap::from_vec(
        VALUES.to_vec(),
        ascending as Order<i64>,
    )];

    receiver.clone_from(&source);

    assert_eq!(receiver.len(), 1);
    assert_eq!(drain(receiver.remove(0)), vec![9, 7, 5, 3, 1]);
}

#[test]
fn clone_from_with_zero_sized_comparator() {
    let mut greatest = BinaryHeap::from_vec(vec![1, 2, 3], Max);
    greatest.clone_from(&BinaryHeap::from_vec(VALUES.to_vec(), Max));
    assert_eq!(drain(greatest), vec![9, 7, 5, 3, 1]);

    let mut least = BinaryHeap::from_vec(vec![1, 2, 3], Min);
    least.clone_from(&BinaryHeap::from_vec(VALUES.to_vec(), Min));
    assert_eq!(drain(least), vec![1, 3, 5, 7, 9]);
}

#[test]
fn a_clone_and_its_source_agree_step_for_step_through_a_shared_program() {
    let mut cases = 0;

    for seed in 0..40u64 {
        let mut rng = Rng::new(seed + 77);
        let len = rng.below(120);
        let values: Vec<i64> = (0..len).map(|_| rng.next_u64() as i64).collect();

        let mut original = BinaryHeap::from_vec(
            values,
            ByRotation {
                offset: seed as i64,
            },
        );
        let mut copy = original.clone();

        for step in 0..200 {
            if rng.below(3) == 0 {
                assert_eq!(original.pop(), copy.pop(), "seed {seed} step {step}");
            } else {
                let value = rng.next_u64() as i64;
                original.push(value);
                copy.push(value);
            }
            assert_eq!(
                original.as_slice(),
                copy.as_slice(),
                "seed {seed} step {step}"
            );
        }

        assert_eq!(drain(original), drain(copy), "seed {seed}");
        cases += 1;
    }

    assert_eq!(cases, 40);
}
