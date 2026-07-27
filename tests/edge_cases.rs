//! One test per degenerate input, plus comparators built to break the loops.
//!
//! The hostile comparators are the interesting half. A comparator that lies about its own order
//! must still leave a heap that ends every loop, stays in bounds, and holds every element once.

mod common;

use std::cell::Cell;
use std::cmp::Ordering;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering as Atomic};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use binary_heap::{BinaryHeap, Compare, Max, Min, PeekMut};
use common::{assert_heap_property, Order, Rng};

#[test]
fn an_empty_heap_answers_every_query_without_a_comparator_call() {
    let calls = Cell::new(0usize);
    let counted = |a: &i32, b: &i32| {
        calls.set(calls.get() + 1);
        a.cmp(b)
    };

    let mut heap = BinaryHeap::new(counted);

    assert_eq!(heap.pop(), None);
    assert_eq!(heap.peek(), None);
    assert!(heap.peek_mut().is_none());
    assert_eq!(heap.len(), 0);
    assert!(heap.is_empty());
    assert_eq!(heap.as_slice(), &[] as &[i32]);
    heap.clear();
    assert_eq!(heap.len(), 0);
    assert_eq!(calls.get(), 0);

    let empty: BinaryHeap<i32, Max> = BinaryHeap::new(Max);
    assert_eq!(empty.into_vec(), Vec::new());
    let empty: BinaryHeap<i32, Max> = BinaryHeap::new(Max);
    assert_eq!(empty.into_sorted_vec(), Vec::new());
}

#[test]
fn a_one_element_heap_yields_that_element_and_empties() {
    let mut heap = BinaryHeap::new(Max);
    heap.push(42);

    assert_eq!(heap.peek(), Some(&42));
    assert_eq!(heap.pop(), Some(42));
    assert_eq!(heap.len(), 0);

    assert_eq!(
        BinaryHeap::from_vec(vec![42], Max).into_sorted_vec(),
        vec![42]
    );
}

#[test]
fn from_vec_on_an_empty_vector_calls_the_comparator_zero_times() {
    let calls = Cell::new(0usize);
    let counted = |a: &i32, b: &i32| {
        calls.set(calls.get() + 1);
        a.cmp(b)
    };

    let heap = BinaryHeap::from_vec(Vec::new(), counted);

    assert_eq!(heap.len(), 0);
    assert_eq!(calls.get(), 0);
}

#[test]
fn a_heap_of_equal_elements_returns_every_one_of_them() {
    let mut heap = BinaryHeap::from_vec(vec![7i32; 500], Max);
    assert_heap_property(&heap, "all equal", 0, 0);

    let mut count = 0;
    while let Some(item) = heap.pop() {
        assert_eq!(item, 7);
        count += 1;
    }
    assert_eq!(count, 500);
}

#[test]
fn a_comparator_that_calls_everything_equal_keeps_the_invariant() {
    let mut heap = BinaryHeap::from_vec((0..300).collect(), |_: &i32, _: &i32| Ordering::Equal);
    assert_heap_property(&heap, "always equal", 0, 0);

    for step in 0..300 {
        heap.push(step);
        assert_heap_property(&heap, "always equal", 0, step as usize);
    }

    let mut drained = heap.into_sorted_vec();
    drained.sort_unstable();
    let mut expected: Vec<i32> = (0..300).chain(0..300).collect();
    expected.sort_unstable();
    assert_eq!(drained, expected);
}

#[test]
fn extreme_integer_values_survive_every_operation() {
    let values = vec![i64::MIN, i64::MAX, 0, -1, 1, i64::MIN + 1, i64::MAX - 1];
    let heap = BinaryHeap::from_vec(values.clone(), Max);
    let mut expected = values;
    expected.sort_unstable();
    assert_eq!(heap.into_sorted_vec(), expected);

    // Ascending under `Min` is descending by value, because `Min` reverses the comparison.
    let heap = BinaryHeap::from_vec(vec![u64::MAX, 0, 1, u64::MAX - 1], Min);
    assert_eq!(heap.into_sorted_vec(), vec![u64::MAX, u64::MAX - 1, 1, 0]);
}

#[test]
fn floats_order_through_a_caller_supplied_closure() {
    // `f64` has no `Ord`, so the caller brings the total order. The IEEE-754 total order is one,
    // and it separates the two zeros and gives NaN a place, which `partial_cmp` cannot do.
    //
    // Reading the bits as `i64` almost gives it. Positive values already sort correctly against
    // each other, negative ones come out reversed and above the positives. Flipping every bit
    // below the sign of a negative value fixes both, so XOR the sign bit smeared across the low
    // 63 bits and compare.
    fn total_order(left: &f64, right: &f64) -> Ordering {
        let key = |value: &f64| {
            let bits = value.to_bits() as i64;
            bits ^ (((bits >> 63) as u64) >> 1) as i64
        };
        key(left).cmp(&key(right))
    }

    let values = vec![
        0.5f64,
        -0.0,
        0.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ];
    let sorted = BinaryHeap::from_vec(values, total_order).into_sorted_vec();

    assert_eq!(sorted[0], f64::NEG_INFINITY);
    assert!(sorted[1].is_sign_negative());
    assert!(sorted[2].is_sign_positive());
    assert_eq!(sorted[3], 0.5);
    assert_eq!(sorted[4], f64::INFINITY);
    assert!(sorted[5].is_nan());
}

#[test]
fn a_full_length_heap_of_zero_sized_elements_builds_and_sorts_without_walking_it() {
    // `vec![(); usize::MAX]` allocates nothing and builds in constant time. A heapify or a sort
    // that walked it would run longer than the machine will, so the check is a timeout.
    //
    // That constant-time build is newer than 1.56.0, which writes the elements one at a time and
    // never returns. Nothing in this crate is involved, and the test suite runs on stable only, so
    // this test never sees 1.56.0.
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut heap = BinaryHeap::from_vec(vec![(); usize::MAX], Max);
        let built = heap.len();
        let popped = heap.pop();
        let sorted = heap.into_sorted_vec().len();
        let _ = tx.send((built, popped, sorted));
    });

    match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(result) => assert_eq!(result, (usize::MAX, Some(()), usize::MAX - 1)),
        Err(_) => panic!("a zero-sized element type made the heap walk its length"),
    }
}

#[test]
fn with_capacity_zero_allocates_nothing() {
    let heap: BinaryHeap<i32, Max> = BinaryHeap::with_capacity(0, Max);
    assert_eq!(heap.capacity(), 0);
    assert_eq!(heap.len(), 0);
}

#[test]
fn sorted_reverse_sorted_and_uniform_input_all_heapify_correctly() {
    const LEN: i32 = 1025;

    for (name, input) in [
        ("ascending", (0..LEN).collect::<Vec<i32>>()),
        ("descending", (0..LEN).rev().collect::<Vec<i32>>()),
        ("uniform", vec![3i32; LEN as usize]),
    ] {
        let heap = BinaryHeap::from_vec(input.clone(), Max);
        assert_heap_property(&heap, name, 0, 0);

        let mut expected = input;
        expected.sort_unstable();
        assert_eq!(heap.into_sorted_vec(), expected, "{}", name);
    }
}

#[test]
fn comparators_that_always_answer_the_same_way_terminate_and_keep_every_element() {
    let fixed: [(&str, Order<i32>); 3] = [
        ("always less", |_, _| Ordering::Less),
        ("always greater", |_, _| Ordering::Greater),
        ("always equal", |_, _| Ordering::Equal),
    ];

    for (name, order) in fixed {
        let mut heap = BinaryHeap::from_vec((0..400).collect::<Vec<i32>>(), order);
        for value in 400..500 {
            heap.push(value);
        }

        let mut drained = heap.into_sorted_vec();
        drained.sort_unstable();
        assert_eq!(drained, (0..500).collect::<Vec<i32>>(), "{}", name);
    }
}

#[test]
fn a_comparator_drawn_from_a_generator_never_hangs_or_loses_an_element() {
    let mut cases = 0;

    for seed in 0..60u64 {
        // A fresh answer on every call, so the order is not even consistent with itself.
        let state = Cell::new(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let chaotic = |_: &i32, _: &i32| {
            let mut x = state.get();
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            state.set(x);
            match x % 3 {
                0 => Ordering::Less,
                1 => Ordering::Equal,
                _ => Ordering::Greater,
            }
        };

        let mut heap = BinaryHeap::new(chaotic);
        let mut pushed = Vec::new();
        let mut popped = Vec::new();

        for value in 0..400i32 {
            heap.push(value);
            pushed.push(value);
            if value % 3 == 0 {
                if let Some(item) = heap.pop() {
                    popped.push(item);
                }
            }
        }

        popped.extend_from_slice(heap.as_slice());
        popped.sort_unstable();
        pushed.sort_unstable();
        assert_eq!(popped, pushed, "seed {}", seed);
        cases += 1;
    }

    assert_eq!(cases, 60);
}

#[test]
fn a_comparator_that_panics_leaves_every_element_in_the_heap() {
    static DROPS: AtomicUsize = AtomicUsize::new(0);

    struct Tracked(i32);

    impl Drop for Tracked {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Atomic::SeqCst);
        }
    }

    // Panics on the call after the budget runs out, so a test can place the failure inside a
    // chosen operation.
    let budget = Cell::new(usize::MAX);
    let fragile = |a: &Tracked, b: &Tracked| {
        let left = budget.get();
        assert_ne!(left, 0, "the comparator ran out of budget");
        budget.set(left - 1);
        a.0.cmp(&b.0)
    };

    let mut heap = BinaryHeap::from_vec((0..64).map(Tracked).collect(), fragile);
    DROPS.store(0, Atomic::SeqCst);

    // Inside a push, after the element is in the buffer and while it is being sifted.
    budget.set(2);
    let pushed = catch_unwind(AssertUnwindSafe(|| heap.push(Tracked(1000))));
    assert!(pushed.is_err());
    assert_eq!(heap.len(), 65);
    assert_eq!(DROPS.load(Atomic::SeqCst), 0);

    // Inside a pop, which must not lose the element it was about to return.
    budget.set(1);
    let popped = catch_unwind(AssertUnwindSafe(|| heap.pop()));
    assert!(popped.is_err());
    assert_eq!(heap.len(), 65);
    assert_eq!(DROPS.load(Atomic::SeqCst), 0);

    // The heap still works once the comparator behaves again.
    budget.set(usize::MAX);
    let mut values: Vec<i32> = Vec::new();
    while let Some(item) = heap.pop() {
        values.push(item.0);
    }
    values.sort_unstable();
    assert_eq!(values, (0..64).chain(1000..1001).collect::<Vec<i32>>());
    assert_eq!(DROPS.load(Atomic::SeqCst), 65);
}

#[test]
#[should_panic(expected = "capacity overflow")]
fn push_past_the_zero_sized_length_limit_panics() {
    // The one input that makes `push` panic. `vec![(); usize::MAX]` costs nothing to build, so the
    // next push has nowhere to put the element and `Vec` says so. Runs on stable only, for the
    // same reason as the test above.
    let mut heap = BinaryHeap::from_vec(vec![(); usize::MAX], Max);
    heap.push(());
}

#[test]
fn an_inconsistent_comparator_can_leave_the_heap_property_broken() {
    // Safe code, no leaked guard, no unsafe. The docs say the guarantee covers comparators that
    // keep answering the same way, and this is what falls outside it.
    let heap = BinaryHeap::from_vec(vec![1, 2, 3, 4, 5], |_: &i32, _: &i32| Ordering::Less);
    assert_eq!(heap.as_slice(), [1, 2, 3, 4, 5]);
    assert_eq!(
        heap.comparator()
            .compare(&heap.as_slice()[0], &heap.as_slice()[1]),
        Ordering::Less,
        "the root should not outrank its child under this comparator"
    );

    // A comparator that changes its own answers reaches the same place. The buffer keeps the
    // arrangement the build asked for, and the new answers disagree with it.
    let reversed = Cell::new(false);
    let flipping = |a: &i32, b: &i32| {
        if reversed.get() {
            b.cmp(a)
        } else {
            a.cmp(b)
        }
    };
    let heap = BinaryHeap::from_vec(vec![1, 2, 3, 4, 5], flipping);
    assert_ne!(
        heap.comparator()
            .compare(&heap.as_slice()[0], &heap.as_slice()[1]),
        Ordering::Less
    );
    reversed.set(true);
    assert_eq!(
        heap.comparator()
            .compare(&heap.as_slice()[0], &heap.as_slice()[1]),
        Ordering::Less
    );
}

#[test]
#[should_panic(expected = "capacity overflow")]
fn reserve_past_the_allocation_limit_panics() {
    let mut heap: BinaryHeap<i64, Max> = BinaryHeap::new(Max);
    heap.reserve(usize::MAX);
}

#[test]
#[should_panic(expected = "capacity overflow")]
fn with_capacity_past_the_allocation_limit_panics() {
    let _heap: BinaryHeap<i64, Max> = BinaryHeap::with_capacity(usize::MAX, Max);
}

#[test]
fn a_peek_guard_restores_the_invariant_for_a_smaller_larger_or_equal_root() {
    for (step, replacement) in [-100i32, 4, 100].into_iter().enumerate() {
        let mut heap = BinaryHeap::from_vec(vec![9, 4, 7, 1, 3, 8], Max);
        if let Some(mut root) = heap.peek_mut() {
            *root = replacement;
        }
        assert_heap_property(&heap, "peek_mut write", 0, step);

        let mut expected = vec![4, 7, 1, 3, 8, replacement];
        expected.sort_unstable();
        assert_eq!(heap.into_sorted_vec(), expected);
    }
}

#[test]
fn peek_mut_pop_takes_the_root_and_leaves_a_valid_heap() {
    let mut heap = BinaryHeap::from_vec(vec![9, 4, 7, 1, 3, 8], Max);
    assert_eq!(PeekMut::pop(heap.peek_mut().unwrap()), 9);
    assert_heap_property(&heap, "peek_mut pop", 0, 0);
    assert_eq!(heap.into_sorted_vec(), vec![1, 3, 4, 7, 8]);
}

#[test]
fn forgetting_a_peek_guard_keeps_every_element_and_panics_at_no_later_point() {
    let mut heap = BinaryHeap::from_vec(vec![9, 4, 7, 1, 3, 8], Max);

    let mut guard = heap.peek_mut().unwrap();
    *guard = -1;
    // Leaking the guard skips the sift, so the heap property can stay violated. Documented. An
    // inconsistent comparator reaches the same state by another route, covered above.
    std::mem::forget(guard);

    assert_eq!(heap.len(), 6);
    assert_eq!(heap.peek(), Some(&-1));

    // Every later operation still ends and keeps the element multiset.
    heap.push(5);
    let mut drained = heap.into_sorted_vec();
    drained.sort_unstable();
    assert_eq!(drained, vec![-1, 1, 3, 4, 5, 7, 8]);
}

#[test]
fn every_corpus_element_type_sorts_like_sort_by() {
    #[derive(Clone, PartialEq, Debug)]
    struct Wide {
        key: u64,
        padding: [u64; 7],
    }

    let mut rng = Rng::new(2026);

    // `i8` over 500 draws is dense in ties, and equal `i8` values are the same value, so the
    // sorted sequences match exactly even though the heap is not stable.
    let mut bytes: Vec<i8> = (0..500).map(|_| rng.next_u64() as i8).collect();
    let heap = BinaryHeap::from_vec(bytes.clone(), Max);
    bytes.sort_unstable();
    assert_eq!(heap.into_sorted_vec(), bytes);

    // The remaining types carry a unique low field so the order is strict and the comparison is
    // element for element.
    let mut pairs: Vec<(u32, u32)> = (0..500).map(|i| (rng.next_u64() as u32 % 8, i)).collect();
    let heap = BinaryHeap::from_vec(pairs.clone(), Max);
    pairs.sort_unstable();
    assert_eq!(heap.into_sorted_vec(), pairs);

    let mut words: Vec<String> = (0..500)
        .map(|i| format!("{:016x}-{:03}", rng.next_u64(), i))
        .collect();
    let heap = BinaryHeap::from_vec(words.clone(), Min);
    words.sort_by(|a, b| b.cmp(a));
    assert_eq!(heap.into_sorted_vec(), words);

    let mut wide: Vec<Wide> = (0..500u64)
        .map(|i| Wide {
            key: (rng.next_u64() & !0xFFF) | i,
            padding: [0; 7],
        })
        .collect();
    let heap = BinaryHeap::from_vec(wide.clone(), |a: &Wide, b: &Wide| a.key.cmp(&b.key));
    wide.sort_by_key(|item| item.key);
    assert_eq!(heap.into_sorted_vec(), wide);

    let units = BinaryHeap::from_vec(vec![(); 500], Max);
    assert_eq!(units.into_sorted_vec().len(), 500);
}
