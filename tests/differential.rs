//! Lockstep differential against two independent oracles.
//!
//! Oracle A is a vector kept sorted. Oracle B is `std::collections::BinaryHeap`. Every step
//! compares `peek`, `pop`, and `len`, not just the end state, so a divergence is reported at the
//! operation that caused it. The heap-property checker lives in `props.rs`.

mod common;

use std::cmp::Ordering;

use binary_heap::BinaryHeap;
use common::{distinct_value, Rng, SortedModel, StdModel, STRICT_ORDERS, TIED_ORDERS};

const SEEDS: u64 = 60;
const STEPS: usize = 1200;

/// Sizes each program steers toward, so the corpus spends time at the boundaries of a complete
/// tree rather than hovering near an empty heap.
const TARGETS: [usize; 12] = [0, 1, 2, 3, 7, 8, 9, 15, 16, 17, 200, 1024];

/// Picks the next operation, biased toward `target` so the heap actually reaches that size.
///
/// Returns 0 for push, 1 for pop, 2 for peek, 3 for clear.
fn next_op(rng: &mut Rng, len: usize, target: usize) -> u8 {
    let roll = rng.below(100);
    // Below the target, favour pushes. At or above it, favour pops. Peek and clear stay rare.
    let push_share = if len < target { 78 } else { 28 };
    if roll < push_share {
        0
    } else if roll < 95 {
        1
    } else if roll < 99 {
        2
    } else {
        3
    }
}

/// One random program run against the sorted model under a strict total order, where the crate and
/// the oracle must agree element for element.
fn strict_vs_sorted(seed: u64, order: fn(&i64, &i64) -> Ordering, name: &str) {
    let mut rng = Rng::new(seed);
    let target = TARGETS[(seed as usize) % TARGETS.len()];
    let mut heap = BinaryHeap::new(order);
    let mut model = SortedModel::new(order);
    let mut tag = 0usize;

    for step in 0..STEPS {
        match next_op(&mut rng, heap.len(), target) {
            0 => {
                let value = distinct_value(&mut rng, tag);
                tag += 1;
                heap.push(value);
                model.push(value);
            }
            1 => assert_eq!(heap.pop(), model.pop(), "{name} seed {seed} step {step}"),
            2 => assert_eq!(heap.peek(), model.peek(), "{name} seed {seed} step {step}"),
            _ => {
                heap.clear();
                model.clear();
            }
        }
        assert_eq!(heap.len(), model.len(), "{name} seed {seed} step {step}");
    }
}

/// The same program shape against `std::collections::BinaryHeap`.
fn strict_vs_std(seed: u64, order: fn(&i64, &i64) -> Ordering, name: &str) {
    let mut rng = Rng::new(seed);
    let target = TARGETS[(seed as usize) % TARGETS.len()];
    let mut heap = BinaryHeap::new(order);
    let mut model = StdModel::new(order);
    let mut tag = 0usize;

    for step in 0..STEPS {
        match next_op(&mut rng, heap.len(), target) {
            0 => {
                let value = distinct_value(&mut rng, tag);
                tag += 1;
                heap.push(value);
                model.push(value);
            }
            1 => assert_eq!(heap.pop(), model.pop(), "{name} seed {seed} step {step}"),
            2 => assert_eq!(heap.peek(), model.peek(), "{name} seed {seed} step {step}"),
            _ => {
                heap.clear();
                model.clear();
            }
        }
        assert_eq!(heap.len(), model.len(), "{name} seed {seed} step {step}");
    }
}

/// Ties are dense here, so equal elements may legitimately come out in different orders and an
/// element-for-element check would fail on correct output. Two checks survive ties. Each popped
/// triple must compare `Equal`, and the raw values the heap still holds plus the raw values it
/// returned must add up to the raw values pushed into it.
fn tied_lockstep(seed: u64, order: fn(&i64, &i64) -> Ordering, name: &str) {
    let mut rng = Rng::new(seed);
    let target = TARGETS[(seed as usize) % TARGETS.len()];
    let mut heap = BinaryHeap::new(order);
    let mut sorted = SortedModel::new(order);
    let mut std_model = StdModel::new(order);
    let mut pushed = Vec::new();
    let mut popped = Vec::new();

    for step in 0..STEPS {
        // No clear, so the pushed and popped tallies stay comparable.
        match next_op(&mut rng, heap.len(), target).min(2) {
            0 => {
                let value = rng.below(8) as i64;
                pushed.push(value);
                heap.push(value);
                sorted.push(value);
                std_model.push(value);
            }
            1 => {
                let ours = heap.pop();
                let theirs = sorted.pop();
                let stds = std_model.pop();
                assert_eq!(
                    ours.is_some(),
                    theirs.is_some(),
                    "{name} seed {seed} step {step}"
                );
                assert_eq!(
                    ours.is_some(),
                    stds.is_some(),
                    "{name} seed {seed} step {step}"
                );
                if let (Some(a), Some(b), Some(c)) = (ours, theirs, stds) {
                    assert_eq!(
                        order(&a, &b),
                        Ordering::Equal,
                        "{name} seed {seed} step {step}"
                    );
                    assert_eq!(
                        order(&a, &c),
                        Ordering::Equal,
                        "{name} seed {seed} step {step}"
                    );
                    popped.push(a);
                }
            }
            _ => {
                let ours = heap.peek().copied();
                let theirs = sorted.peek().copied();
                let stds = std_model.peek().copied();
                assert_eq!(
                    ours.is_some(),
                    theirs.is_some(),
                    "{name} seed {seed} step {step}"
                );
                if let (Some(a), Some(b), Some(c)) = (ours, theirs, stds) {
                    assert_eq!(
                        order(&a, &b),
                        Ordering::Equal,
                        "{name} seed {seed} step {step}"
                    );
                    assert_eq!(
                        order(&a, &c),
                        Ordering::Equal,
                        "{name} seed {seed} step {step}"
                    );
                }
            }
        }
        assert_eq!(heap.len(), sorted.len(), "{name} seed {seed} step {step}");
        assert_eq!(
            heap.len(),
            std_model.len(),
            "{name} seed {seed} step {step}"
        );
    }

    popped.extend_from_slice(heap.as_slice());
    popped.sort_unstable();
    pushed.sort_unstable();
    assert_eq!(popped, pushed, "{name} seed {seed} element multiset");
}

#[test]
fn strict_programs_match_the_sorted_model_step_for_step() {
    let mut cases = 0;
    for (name, order) in STRICT_ORDERS {
        for seed in 0..SEEDS {
            strict_vs_sorted(seed, order, name);
            cases += STEPS;
        }
    }
    assert_eq!(cases, 288_000);
}

#[test]
fn strict_programs_match_the_standard_library_heap_step_for_step() {
    let mut cases = 0;
    for (name, order) in STRICT_ORDERS {
        for seed in 0..SEEDS {
            strict_vs_std(seed, order, name);
            cases += STEPS;
        }
    }
    assert_eq!(cases, 288_000);
}

#[test]
fn tie_heavy_programs_match_both_oracles_step_for_step() {
    let mut cases = 0;
    for (name, order) in TIED_ORDERS {
        for seed in 0..SEEDS {
            tied_lockstep(seed, order, name);
            cases += STEPS;
        }
    }
    assert_eq!(cases, 216_000);
}

#[test]
fn into_sorted_vec_matches_sort_by_at_every_corpus_length() {
    let lengths = [0usize, 1, 2, 3, 7, 8, 9, 15, 16, 17, 1023, 1024, 1025];
    let mut cases = 0;

    for (name, order) in STRICT_ORDERS {
        for &len in &lengths {
            let mut rng = Rng::new(len as u64 + 7);
            let values: Vec<i64> = (0..len).map(|i| distinct_value(&mut rng, i)).collect();

            let mut expected = values.clone();
            expected.sort_by(order);

            let heap = BinaryHeap::from_vec(values, order);
            let capacity = heap.capacity();
            let sorted = heap.into_sorted_vec();

            assert_eq!(sorted, expected, "{name} at length {len}");
            // Sorting happens in the heap's own buffer, so nothing new is allocated.
            assert_eq!(sorted.capacity(), capacity, "{name} at length {len}");
            cases += 1;
        }
    }

    assert_eq!(cases, 52);
}

#[test]
fn a_hundred_thousand_elements_sort_like_sort_by() {
    let mut rng = Rng::new(99);
    let values: Vec<i64> = (0..100_000).map(|_| rng.next_u64() as i64).collect();

    let mut expected = values.clone();
    expected.sort_unstable();

    let sorted = BinaryHeap::from_vec(values, |a: &i64, b: &i64| a.cmp(b)).into_sorted_vec();
    assert_eq!(sorted, expected);
}

#[test]
fn from_vec_drains_like_a_push_loop_under_a_strict_order() {
    let mut cases = 0;

    for (name, order) in STRICT_ORDERS {
        for seed in 0..SEEDS {
            let mut rng = Rng::new(seed + 1000);
            let len = rng.below(200);
            let values: Vec<i64> = (0..len).map(|i| distinct_value(&mut rng, i)).collect();

            let mut pushed = BinaryHeap::new(order);
            for &value in &values {
                pushed.push(value);
            }

            let bulk = BinaryHeap::from_vec(values, order);
            assert_eq!(
                bulk.into_sorted_vec(),
                pushed.into_sorted_vec(),
                "{name} seed {seed}"
            );
            cases += 1;
        }
    }

    assert_eq!(cases, 240);
}
