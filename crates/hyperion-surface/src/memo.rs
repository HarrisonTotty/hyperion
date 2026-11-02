//! An open-addressed table of `u64` keys, for the caller's caches that are read far more often than
//! filled: the far field's kept vertex heights ([`crate::bake::horizon`]) and the small craters'
//! cell draws ([`crate::synth::craters`]).
//!
//! A look-up is a multiplication, a shift and a probe or two, where a `BTreeMap` of the far field's
//! up to 131,072 heights walked its levels comparing keys: the two maps' searches were about 26 M
//! of a high draft's 375 M instructions on the pod's 336-patch stream (R10.T6.a's lever, callgrind,
//! pod, 16 vCPU, 2026-11-02). Nothing reads the table's order, and each value a cache keeps is a
//! pure function of its key, so where an entry lies changes no output.

/// The key of an empty slot. Every caller's keys keep their top three bits below `0b111` (a cube
/// face's index, at most 5, or zero), so none is this.
const EMPTY: u64 = u64::MAX;

/// The slots a table takes at its first entry: a power of two.
const FIRST_SLOTS: usize = 64;

/// Fibonacci hashing's multiplier, ⌊2⁶⁴ ÷ φ⌋, odd: the top bits of a key's product with it depend
/// on all of the key's bits (Knuth, The Art of Computer Programming, vol. 3, §6.4).
const GOLDEN: u64 = 0x9e37_79b9_7f4a_7c15;

/// A map from `u64` keys to `V`s by open addressing with linear probing, at most half full, which
/// grows by doubling and empties only whole ([`clear`](Self::clear)).
#[derive(Debug, Clone, Default)]
pub(crate) struct Table<V> {
    /// Each slot's key, [`EMPTY`] where it holds none; a power of two of them, or none.
    keys: Vec<u64>,
    /// Each slot's value, `V::default()` where it holds none.
    values: Vec<V>,
    /// The slots held.
    len: usize,
}

impl<V: Default> Table<V> {
    /// An empty table, which allocates its slots at its first entry.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
            len: 0,
        }
    }

    /// How many entries it holds.
    #[must_use]
    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    /// The value of `key`, if it holds one.
    ///
    /// # Panics
    ///
    /// If `key` is the empty slots' key, all ones, which no caller's key is.
    #[must_use]
    pub(crate) fn get(&self, key: u64) -> Option<&V> {
        let (at, held) = self.probe(key)?;
        held.then(|| &self.values[at])
    }

    /// The value of `key`, made by `make` and held first where it holds none.
    ///
    /// # Panics
    ///
    /// If `key` is the empty slots' key, all ones, which no caller's key is.
    pub(crate) fn get_or_insert_with(&mut self, key: u64, make: impl FnOnce() -> V) -> &mut V {
        let at = match self.probe(key) {
            Some((at, true)) => at,
            found => {
                let at = match found {
                    // Room for one more within half the slots: the free slot found.
                    Some((at, _)) if 2 * (self.len + 1) <= self.keys.len() => at,
                    _ => {
                        self.grow();
                        self.probe(key).expect("a grown table has slots").0
                    }
                };
                // Made before its key is placed, so that a `make` that panics leaves no entry.
                let value = make();
                self.keys[at] = key;
                self.values[at] = value;
                self.len += 1;
                at
            }
        };
        &mut self.values[at]
    }

    /// Forgets every entry, keeping the slots' allocation.
    pub(crate) fn clear(&mut self) {
        self.keys.fill(EMPTY);
        self.values.fill_with(V::default);
        self.len = 0;
    }

    /// The slot of `key`, and whether it holds it; or the first free slot of its probe, where it
    /// does not. `None` before the first entry, when there are no slots.
    ///
    /// # Panics
    ///
    /// If `key` is the empty slots' key, all ones, which would match a free slot.
    #[must_use]
    fn probe(&self, key: u64) -> Option<(usize, bool)> {
        assert_ne!(key, EMPTY, "a table's key is never all ones");
        if self.keys.is_empty() {
            return None;
        }
        let mask = self.keys.len() - 1;
        let mut at = home(key, self.keys.len());
        loop {
            match self.keys[at] {
                held if held == key => return Some((at, true)),
                EMPTY => return Some((at, false)),
                _ => at = (at + 1) & mask,
            }
        }
    }

    /// Doubles the slots, [`FIRST_SLOTS`] at first, and places every entry again.
    fn grow(&mut self) {
        let slots = (2 * self.keys.len()).max(FIRST_SLOTS);
        let keys = std::mem::replace(&mut self.keys, vec![EMPTY; slots]);
        let mut values = std::mem::take(&mut self.values);
        self.values.resize_with(slots, V::default);
        let mask = slots - 1;
        for (key, value) in keys.into_iter().zip(values.drain(..)) {
            if key == EMPTY {
                continue;
            }
            let mut at = home(key, slots);
            while self.keys[at] != EMPTY {
                at = (at + 1) & mask;
            }
            self.keys[at] = key;
            self.values[at] = value;
        }
    }
}

/// The slot `key`'s probe starts at among `slots`, a power of two: the top bits of its Fibonacci
/// product.
#[must_use]
fn home(key: u64, slots: usize) -> usize {
    debug_assert!(
        slots.is_power_of_two(),
        "a table's slots are a power of two"
    );
    let bits = slots.trailing_zeros();
    let top = key.wrapping_mul(GOLDEN) >> (u64::BITS - bits);
    usize::try_from(top).expect("a slot index is below the table's length")
}

#[cfg(test)]
mod tests {
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn a_table_holds_each_key_s_first_value_through_its_growth() {
        let mut table = Table::<u64>::new();
        assert_eq!(table.get(7), None);
        // Keys shaped as the callers' are: fields packed at fixed shifts.
        let keys: Vec<u64> = (0..5_000_u64)
            .map(|n| ((n % 6) << 61) | ((n % 13) << 56) | ((n / 7) << 28) | ((n * 31) % 977))
            .collect();
        for (n, &key) in (0_u64..).zip(&keys) {
            let first = *table.get_or_insert_with(key, || n);
            assert!(first <= n, "a key's value is its first insertion's");
        }
        let distinct: std::collections::BTreeSet<u64> = keys.iter().copied().collect();
        assert_eq!(table.len(), distinct.len());
        for (n, &key) in (0_u64..).zip(&keys) {
            let first = keys
                .iter()
                .position(|&k| k == key)
                .expect("the key is listed");
            assert_eq!(table.get(key), Some(&u64::try_from(first).unwrap()), "{n}");
        }
        assert!(2 * table.len() <= table.keys.len(), "at most half full");
    }

    #[test]
    fn a_cleared_table_holds_nothing_and_fills_again() {
        let mut table = Table::<Vec<u8>>::new();
        for key in 0..100 {
            table.get_or_insert_with(key, || vec![1; 3]);
        }
        let slots = table.keys.len();
        table.clear();
        assert_eq!(table.len(), 0);
        assert_eq!(table.keys.len(), slots, "the allocation is kept");
        assert!((0..100).all(|key| table.get(key).is_none()));
        assert!(
            table.values.iter().all(Vec::is_empty),
            "the values are dropped"
        );
        table.get_or_insert_with(5, Vec::new).push(9);
        assert_eq!(table.get(5), Some(&vec![9]));
    }

    /// The tests that panic: a panic traps the browser's module, after which its binary's later
    /// tests run in a best-effort state (the crate's `tests/panics.rs`), which cannot reach the
    /// crate-private table.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    mod native_only {
        use super::*;

        #[test]
        #[should_panic(expected = "a table's key is never all ones")]
        fn the_empty_key_is_refused() {
            Table::<u8>::new().get_or_insert_with(u64::MAX, || 0);
        }

        #[test]
        #[should_panic(expected = "a table's key is never all ones")]
        fn the_empty_key_is_never_looked_up() {
            let mut table = Table::<u8>::new();
            table.get_or_insert_with(3, || 1);
            let _ = table.get(u64::MAX);
        }

        // Only where a panic unwinds, as it does natively, can the table be read after one.
        #[cfg(panic = "unwind")]
        #[test]
        fn a_make_that_panics_leaves_no_entry() {
            let mut table = Table::<Vec<u8>>::new();
            table.get_or_insert_with(1, || vec![1]);
            let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                table.get_or_insert_with(2, || panic!("no value"));
            }));
            assert!(refused.is_err());
            assert_eq!(table.get(2), None);
            assert_eq!(table.len(), 1);
        }
    }
}
