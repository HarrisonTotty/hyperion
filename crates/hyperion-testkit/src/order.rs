//! Order independence: "generating A then B equals generating B then A equals generating B alone".
//!
//! A pure function passes trivially. The helper exists for generators that sit behind caches,
//! where a value computed after its neighbours can differ from the same value computed cold.

use std::fmt::Debug;

/// Seed of the fixed permutation. Not part of any generated output.
const PERMUTATION_SEED: u64 = 0x5eed_0f0b_de75_0001;

/// Asserts that `f` gives each key the same value whatever was asked before it.
///
/// `f` is evaluated over `keys` forwards, backwards, in one fixed pseudo-random permutation, and
/// once more per key as a separate pass. All four values of each key must be equal.
///
/// `f` is the same closure in every pass, so a cache it captures (through interior mutability)
/// stays warm from one pass to the next, which is what the check exercises: each value must not
/// depend on which keys were asked before it. To compare against a cold evaluation as well, call
/// the helper a second time with an `f` that builds a fresh cache on every call.
///
/// # Panics
///
/// If any key's values differ, naming the key's position in `keys` and the pass that disagreed.
///
/// # Examples
///
/// ```
/// use hyperion_testkit::order::assert_order_independent;
///
/// let keys = [3_u64, 1, 4, 1, 5];
/// assert_order_independent(&keys, |k| k.wrapping_mul(0x9e37_79b9_7f4a_7c15));
/// ```
#[track_caller]
pub fn assert_order_independent<K, V: PartialEq + Debug>(keys: &[K], f: impl Fn(&K) -> V) {
    let forwards: Vec<V> = keys.iter().map(&f).collect();

    let mut backwards: Vec<(usize, V)> = keys
        .iter()
        .enumerate()
        .rev()
        .map(|(i, k)| (i, f(k)))
        .collect();
    backwards.reverse();

    let mut permuted: Vec<(usize, V)> = permutation(keys.len())
        .into_iter()
        .map(|i| (i, f(&keys[i])))
        .collect();
    permuted.sort_by_key(|(i, _)| *i);

    for (position, key) in keys.iter().enumerate() {
        let alone = f(key);
        let reference = &forwards[position];
        for (pass, value) in [
            ("backwards", &backwards[position].1),
            ("permuted", &permuted[position].1),
            ("alone", &alone),
        ] {
            assert!(
                value == reference,
                "order dependence at position {position}: the {pass} pass gave\n  {value:?}\n\
                 but the forwards pass gave\n  {reference:?}"
            );
        }
    }
}

/// A fixed permutation of `0..len`: Fisher–Yates driven by an inline 64-bit LCG.
fn permutation(len: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..len).collect();
    let mut state = PERMUTATION_SEED;
    for i in (1..len).rev() {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let bound = u64::try_from(i + 1).expect("a slice length fits in 64 bits");
        let j =
            usize::try_from((state >> 33) % bound).expect("a value below a usize fits in usize");
        order.swap(i, j);
    }
    order
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn pure_closure_passes() {
        let keys: Vec<u64> = (0..50).collect();
        assert_order_independent(&keys, |k| (k * k, k.to_string()));
    }

    #[test]
    fn empty_and_single_key_pass() {
        assert_order_independent(&[] as &[u64], |k| *k);
        assert_order_independent(&[9_u64], |k| *k);
    }

    #[test]
    #[should_panic(expected = "order dependence at position 0")]
    fn hidden_counter_fails() {
        let calls = Cell::new(0_u64);
        let keys: Vec<u64> = (0..10).collect();
        assert_order_independent(&keys, |k| {
            calls.set(calls.get() + 1);
            k + calls.get()
        });
    }

    #[test]
    fn permutation_is_a_fixed_shuffle_of_every_index() {
        let p = permutation(64);
        let mut sorted = p.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..64).collect::<Vec<_>>());
        assert_ne!(p, sorted, "the permutation should not be the identity");
        assert_eq!(p, permutation(64));
    }
}
