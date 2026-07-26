//! Oracle two, the heap property itself, plus the algebraic properties the heap owes a caller.
//!
//! The property checker runs after every mutating operation of a random program, so a corrupted
//! array fails at the operation that corrupted it rather than at the pop that reveals it.

mod common;

use std::cell::Cell;
use std::cmp::Ordering;

use binary_heap::{BinaryHeap, Compare, Max, Min};
use common::{assert_heap_property, floor_log2, BoxedOrder, Rng};

const SEEDS: u64 = 40;
const STEPS: usize = 300;

/// A comparator that carries state, which a closure cannot express under a name.
#[derive(Clone)]
struct ByRotation {
    offset: i64,
}

impl Compare<i64> for ByRotation {
    fn compare(&self, left: &i64, right: &i64) -> Ordering {
        let rotate = |value: &i64| value.wrapping_add(self.offset).rem_euclid(1009);
        rotate(left).cmp(&rotate(right))
    }
}

/// Runs a random program and checks the heap property after every step. Returns the check count.
fn exercise<C: Compare<i64>>(seed: u64, cmp: C, name: &str) -> usize {
    let mut rng = Rng::new(seed);
    let mut heap = BinaryHeap::new(cmp);

    for step in 0..STEPS {
        match rng.below(100) {
            0..=49 => heap.push(rng.next_u64() as i64),
            50..=79 => {
                heap.pop();
            }
            80..=84 => {
                let _ = heap.peek();
            }
            85..=89 => {
                // A guard that only reads must leave the array alone.
                if let Some(guard) = heap.peek_mut() {
                    let _seen = *guard;
                }
            }
            90..=95 => {
                if let Some(mut guard) = heap.peek_mut() {
                    *guard = rng.next_u64() as i64;
                }
            }
            96..=97 => {
                let batch: Vec<i64> = (0..4).map(|_| rng.next_u64() as i64).collect();
                heap.extend(batch);
            }
            _ => heap.clear(),
        }
        assert_heap_property(&heap, name, seed, step);
    }

    STEPS
}

#[test]
fn the_heap_property_holds_after_every_operation() {
    let mut checks = 0;

    for seed in 0..SEEDS {
        checks += exercise(seed, Max, "Max");
        checks += exercise(seed, Min, "Min");
        checks += exercise(seed, |a: &i64, b: &i64| b.cmp(a), "reversing closure");
        checks += exercise(
            seed,
            |a: &i64, b: &i64| (a & 0x3F).cmp(&(b & 0x3F)),
            "by-key closure",
        );

        let bias = (seed as i64).wrapping_mul(7);
        let capturing = move |a: &i64, b: &i64| a.wrapping_add(bias).cmp(&b.wrapping_add(bias));
        checks += exercise(seed, capturing, "capturing closure");

        checks += exercise(
            seed,
            ByRotation {
                offset: seed as i64,
            },
            "stateful type",
        );

        let pointer: fn(&i64, &i64) -> Ordering = |a, b| a.cmp(b);
        checks += exercise(seed, pointer, "function pointer");

        let boxed: BoxedOrder<i64> =
            Box::new(|a: &i64, b: &i64| a.unsigned_abs().cmp(&b.unsigned_abs()));
        checks += exercise(seed, boxed, "boxed dyn Fn");
    }

    assert_eq!(checks, 96_000);
}

/// Drains a heap and checks the order and the element multiset.
fn drain_and_check<C: Compare<i64>>(seed: u64, cmp: C, name: &str) {
    let mut rng = Rng::new(seed);
    let len = rng.below(300);
    let values: Vec<i64> = (0..len).map(|_| rng.next_u64() as i64).collect();

    let mut heap = BinaryHeap::from_vec(values.clone(), cmp);
    let mut drained: Vec<i64> = Vec::new();
    while let Some(item) = heap.pop() {
        if let Some(previous) = drained.last() {
            assert_ne!(
                heap.comparator().compare(previous, &item),
                Ordering::Less,
                "{} seed {}: drain rose",
                name,
                seed
            );
        }
        drained.push(item);
    }

    let mut expected = values;
    expected.sort_unstable();
    drained.sort_unstable();
    assert_eq!(
        drained, expected,
        "{} seed {}: multiset changed",
        name, seed
    );
}

#[test]
fn the_drain_never_rises_and_keeps_every_element() {
    let mut cases = 0;

    for seed in 0..SEEDS {
        drain_and_check(seed, Max, "Max");
        drain_and_check(seed, Min, "Min");
        drain_and_check(
            seed,
            |a: &i64, b: &i64| (a & 7).cmp(&(b & 7)),
            "by-key closure",
        );
        drain_and_check(seed, |_: &i64, _: &i64| Ordering::Equal, "all equal");
        drain_and_check(
            seed,
            ByRotation {
                offset: seed as i64,
            },
            "stateful type",
        );
        cases += 5;
    }

    assert_eq!(cases, 200);
}

#[test]
fn into_sorted_vec_never_falls_and_keeps_every_element() {
    let mut cases = 0;

    for seed in 0..SEEDS {
        let mut rng = Rng::new(seed + 500);
        let len = rng.below(300);
        let values: Vec<i64> = (0..len).map(|_| rng.next_u64() as i64).collect();
        let order = |a: &i64, b: &i64| (a & 0x1F).cmp(&(b & 0x1F));

        let sorted = BinaryHeap::from_vec(values.clone(), order).into_sorted_vec();
        for pair in sorted.windows(2) {
            assert_ne!(
                order(&pair[0], &pair[1]),
                Ordering::Greater,
                "seed {}",
                seed
            );
        }

        let mut expected = values;
        expected.sort_unstable();
        let mut got = sorted;
        got.sort_unstable();
        assert_eq!(got, expected, "seed {}", seed);
        cases += 1;
    }

    assert_eq!(cases, 40);
}

#[test]
fn reversing_the_comparator_reverses_the_drain() {
    let mut cases = 0;

    for seed in 0..SEEDS {
        let mut rng = Rng::new(seed + 900);
        // Distinct values, so the order is strict and the reversal is exact.
        let mut values: Vec<i64> = (0..rng.below(200) as i64).collect();
        for index in (1..values.len()).rev() {
            values.swap(index, rng.below(index + 1));
        }

        let up = BinaryHeap::from_vec(values.clone(), |a: &i64, b: &i64| a.cmp(b));
        let down = BinaryHeap::from_vec(values, |a: &i64, b: &i64| b.cmp(a));

        let mut up_drain = up.into_sorted_vec();
        let down_drain = down.into_sorted_vec();
        up_drain.reverse();
        assert_eq!(up_drain, down_drain, "seed {}", seed);
        cases += 1;
    }

    assert_eq!(cases, 40);
}

#[test]
fn extend_agrees_with_a_push_loop() {
    let mut cases = 0;

    for seed in 0..SEEDS {
        let mut rng = Rng::new(seed + 1300);
        let start: Vec<i64> = (0..rng.below(50)).map(|_| rng.next_u64() as i64).collect();
        let more: Vec<i64> = (0..rng.below(50)).map(|_| rng.next_u64() as i64).collect();

        let mut extended = BinaryHeap::from_vec(start.clone(), Max);
        extended.extend(more.clone());

        let mut pushed = BinaryHeap::from_vec(start, Max);
        for value in more {
            pushed.push(value);
        }

        assert_eq!(
            extended.into_sorted_vec(),
            pushed.into_sorted_vec(),
            "seed {}",
            seed
        );
        cases += 1;
    }

    assert_eq!(cases, 40);
}

#[test]
fn push_pop_and_bulk_build_stay_inside_the_comparison_bounds() {
    let mut cases = 0;

    for len in 1..=200usize {
        let calls = Cell::new(0usize);
        let counted = |a: &i64, b: &i64| {
            calls.set(calls.get() + 1);
            a.cmp(b)
        };

        let mut rng = Rng::new(len as u64);
        let values: Vec<i64> = (0..len).map(|_| rng.next_u64() as i64).collect();
        let mut heap = BinaryHeap::from_vec(values, counted);

        let build = calls.get();
        assert!(build <= 2 * len, "from_vec at {} used {}", len, build);

        calls.set(0);
        heap.push(rng.next_u64() as i64);
        let push = calls.get();
        let push_bound = floor_log2(heap.len()) as usize + 1;
        assert!(push <= push_bound, "push at {} used {}", len, push);

        let before = heap.len();
        calls.set(0);
        heap.pop();
        let pop = calls.get();
        let pop_bound = 2 * floor_log2(before) as usize + 2;
        assert!(pop <= pop_bound, "pop at {} used {}", before, pop);

        cases += 1;
    }

    assert_eq!(cases, 200);
}

#[test]
fn a_read_only_peek_guard_performs_no_comparisons() {
    let calls = Cell::new(0usize);
    let counted = |a: &i64, b: &i64| {
        calls.set(calls.get() + 1);
        a.cmp(b)
    };

    let mut heap = BinaryHeap::from_vec(vec![5, 9, 1, 7, 3], counted);
    calls.set(0);

    let guard = heap.peek_mut().unwrap();
    assert_eq!(*guard, 9);
    drop(guard);

    assert_eq!(calls.get(), 0);
}

#[test]
fn the_same_program_produces_the_same_array_every_run() {
    let program = |seed: u64| {
        let mut rng = Rng::new(seed);
        let mut heap = BinaryHeap::new(Max);
        for _ in 0..500 {
            if rng.below(3) == 0 {
                heap.pop();
            } else {
                heap.push(rng.next_u64() as i64);
            }
        }
        heap.into_vec()
    };

    assert_eq!(program(11), program(11));
    assert_eq!(program(12), program(12));
    assert_ne!(program(11), program(12));
}

#[test]
fn every_element_is_dropped_exactly_once_when_the_heap_drains() {
    use std::sync::atomic::{AtomicUsize, Ordering as Atomic};

    static DROPS: AtomicUsize = AtomicUsize::new(0);

    struct Tracked(i64);

    impl Drop for Tracked {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Atomic::SeqCst);
        }
    }

    let mut rng = Rng::new(4242);
    let values: Vec<i64> = (0..500).map(|_| rng.next_u64() as i64).collect();
    let tracked: Vec<Tracked> = values.iter().map(|&value| Tracked(value)).collect();

    let mut heap = BinaryHeap::from_vec(tracked, |a: &Tracked, b: &Tracked| a.0.cmp(&b.0));
    let mut drained = Vec::new();
    while let Some(item) = heap.pop() {
        drained.push(item.0);
    }

    let mut expected = values;
    expected.sort_unstable();
    drained.sort_unstable();
    assert_eq!(drained, expected);
    assert_eq!(DROPS.load(Atomic::SeqCst), 500);
}

#[test]
fn clear_drops_every_element_exactly_once() {
    use std::sync::atomic::{AtomicUsize, Ordering as Atomic};

    static DROPS: AtomicUsize = AtomicUsize::new(0);

    struct Tracked;

    impl Drop for Tracked {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Atomic::SeqCst);
        }
    }

    let mut heap = BinaryHeap::new(|_: &Tracked, _: &Tracked| Ordering::Equal);
    for _ in 0..64 {
        heap.push(Tracked);
    }
    heap.clear();

    assert_eq!(DROPS.load(Atomic::SeqCst), 64);
    assert_eq!(heap.len(), 0);
}
