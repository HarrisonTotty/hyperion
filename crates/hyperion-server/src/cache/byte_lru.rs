//! A least-recently-used cache bounded by bytes, not entries.
//!
//! A bulge cell is ten thousand times heavier than a rim cell, so a count of entries bounds
//! nothing (brainstorm, "Runtime and code shape"). Each entry is charged its heap bytes, the size
//! of the value itself, and a fixed [`ENTRY_OVERHEAD_BYTES`] for the bookkeeping around it, so
//! that a million empty cells still count. Eviction takes the least recently used entry first
//! until the new one fits, and an entry larger than the whole budget is handed back rather than
//! stored. Eviction is always safe: everything cached can be regenerated.
//!
//! [`ByteLru`] is a `HashMap` of entries plus a `BTreeMap` from use-tick to key, the oldest first
//! (plan 04, design note 23). [`SharedByteLru`] puts it behind one `std::sync::Mutex`; sharding
//! waits for a benchmark that shows contention.

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::mem;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Bytes charged to every entry on top of its value: the hash table slot, the use-tick index
/// entry, the `Arc`'s counts and the key, rounded up.
pub const ENTRY_OVERHEAD_BYTES: usize = 96;

/// The heap memory a cached value owns, beyond `size_of` of the value itself.
pub trait HeapBytes {
    /// Bytes owned on the heap: for a `Vec<T>`, its capacity times `size_of::<T>()`, plus
    /// whatever the elements own in turn.
    fn heap_bytes(&self) -> usize;
}

/// What [`ByteLru::insert`] did with an entry.
#[derive(Debug)]
#[must_use = "a refused entry is handed back and not stored"]
pub enum Insertion<V> {
    /// The entry was stored, after evicting this many others.
    Stored {
        /// Entries evicted to make room; a replaced entry with the same key is not counted.
        evicted: usize,
    },
    /// The entry is larger than the whole budget. It was not stored, and the cache is unchanged.
    Refused(Arc<V>),
}

/// A snapshot of a cache's size and use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LruCounters {
    entries: usize,
    bytes: usize,
    budget: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    refused: u64,
}

impl LruCounters {
    /// Entries held.
    #[must_use]
    pub fn entries(&self) -> usize {
        self.entries
    }

    /// Bytes charged for the entries held, never above [`LruCounters::budget`].
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// The byte budget.
    #[must_use]
    pub fn budget(&self) -> usize {
        self.budget
    }

    /// Lookups that found their key.
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Lookups that did not.
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Entries evicted to make room for others.
    #[must_use]
    pub fn evictions(&self) -> u64 {
        self.evictions
    }

    /// Entries refused for being larger than the whole budget.
    #[must_use]
    pub fn refused(&self) -> u64 {
        self.refused
    }
}

/// A least-recently-used cache of `Arc<V>` bounded by bytes.
#[derive(Debug)]
pub struct ByteLru<K, V> {
    entries: HashMap<K, Entry<V>>,
    /// Keys by the tick of their last use, the least recently used first.
    by_use: BTreeMap<u64, K>,
    next_tick: u64,
    counters: LruCounters,
}

#[derive(Debug)]
struct Entry<V> {
    value: Arc<V>,
    charge: usize,
    tick: u64,
}

impl<K, V> ByteLru<K, V>
where
    K: Eq + Hash + Clone,
    V: HeapBytes,
{
    /// An empty cache that holds at most `budget_bytes` of charged entries.
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            by_use: BTreeMap::new(),
            next_tick: 0,
            counters: LruCounters {
                budget: budget_bytes,
                ..LruCounters::default()
            },
        }
    }

    /// What an entry holding `value` is charged: its heap bytes, its own size and
    /// [`ENTRY_OVERHEAD_BYTES`].
    #[must_use]
    pub fn charge(value: &V) -> usize {
        value
            .heap_bytes()
            .saturating_add(mem::size_of::<V>())
            .saturating_add(ENTRY_OVERHEAD_BYTES)
    }

    /// The value for `key`, marking it as the most recently used.
    pub fn get(&mut self, key: &K) -> Option<Arc<V>> {
        let tick = self.tick();
        let Some(entry) = self.entries.get_mut(key) else {
            self.counters.misses += 1;
            return None;
        };
        self.by_use.remove(&entry.tick);
        entry.tick = tick;
        self.by_use.insert(tick, key.clone());
        self.counters.hits += 1;
        Some(Arc::clone(&entry.value))
    }

    /// Whether `key` is held, without counting a lookup or marking a use.
    #[must_use]
    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    /// Stores `value` under `key` as the most recently used entry, replacing any entry of that
    /// key and evicting the least recently used others until the total fits the budget.
    pub fn insert(&mut self, key: K, value: Arc<V>) -> Insertion<V> {
        let charge = Self::charge(&value);
        let mut displaced = Vec::new();
        self.insert_charged(key, value, charge, &mut displaced)
    }

    /// [`ByteLru::insert`] with the charge computed already, and the values it evicts or replaces
    /// handed out in `displaced`, so that [`SharedByteLru`] can compute the one and drop the others
    /// outside its lock.
    fn insert_charged(
        &mut self,
        key: K,
        value: Arc<V>,
        charge: usize,
        displaced: &mut Vec<Arc<V>>,
    ) -> Insertion<V> {
        if charge > self.counters.budget {
            self.counters.refused += 1;
            return Insertion::Refused(value);
        }
        if let Some(replaced) = self.entries.remove(&key) {
            self.by_use.remove(&replaced.tick);
            self.counters.bytes -= replaced.charge;
            displaced.push(replaced.value);
        }
        let mut evicted = 0;
        // `bytes` is the sum of the held entries' charges and never above the budget, so it
        // cannot overflow; the sum with the new charge can, when the budget is near `usize::MAX`.
        while self
            .counters
            .bytes
            .checked_add(charge)
            .is_none_or(|total| total > self.counters.budget)
        {
            // Entries are charged at least the overhead, so the loop ends with the cache empty
            // at the latest, when `bytes` is 0 and `charge`, which is within the budget, fits.
            let Some((_, oldest)) = self.by_use.pop_first() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.counters.bytes -= entry.charge;
                displaced.push(entry.value);
            }
            evicted += 1;
        }
        let tick = self.tick();
        self.by_use.insert(tick, key.clone());
        self.entries.insert(
            key,
            Entry {
                value,
                charge,
                tick,
            },
        );
        self.counters.bytes += charge;
        self.counters.evictions += u64::try_from(evicted).unwrap_or(u64::MAX);
        Insertion::Stored { evicted }
    }

    /// Entries held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Bytes charged for the entries held.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.counters.bytes
    }

    /// The byte budget.
    #[must_use]
    pub fn budget(&self) -> usize {
        self.counters.budget
    }

    /// Size, budget, and the hit, miss, eviction and refusal counts so far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        LruCounters {
            entries: self.entries.len(),
            ..self.counters
        }
    }

    fn tick(&mut self) -> u64 {
        let tick = self.next_tick;
        self.next_tick = self
            .next_tick
            .checked_add(1)
            .expect("a u64 use counter does not overflow: 2⁶⁴ cache operations take centuries");
        tick
    }
}

/// A [`ByteLru`] behind one mutex, shared by every thread that uses it.
///
/// Values are computed outside the lock and inserted afterwards; two threads may compute the same
/// value once in a while, and the second insert replaces an equal value, which is cheaper than
/// coordinating. Charges are computed before the lock is taken, and evicted or replaced values
/// are dropped after it is released.
#[derive(Debug)]
pub struct SharedByteLru<K, V> {
    inner: Mutex<ByteLru<K, V>>,
}

impl<K, V> SharedByteLru<K, V>
where
    K: Eq + Hash + Clone,
    V: HeapBytes,
{
    /// An empty cache that holds at most `budget_bytes` of charged entries.
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(ByteLru::new(budget_bytes)),
        }
    }

    /// See [`ByteLru::get`].
    pub fn get(&self, key: &K) -> Option<Arc<V>> {
        self.lock().get(key)
    }

    /// See [`ByteLru::contains_key`].
    #[must_use]
    pub fn contains_key(&self, key: &K) -> bool {
        self.lock().contains_key(key)
    }

    /// See [`ByteLru::insert`].
    pub fn insert(&self, key: K, value: Arc<V>) -> Insertion<V> {
        let charge = ByteLru::<K, V>::charge(&value);
        let mut displaced = Vec::new();
        let insertion = self
            .lock()
            .insert_charged(key, value, charge, &mut displaced);
        // Dropped with the lock released, so that freeing a large value, or a value's own `Drop`,
        // never runs inside the critical section.
        drop(displaced);
        insertion
    }

    /// See [`ByteLru::len`].
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// See [`ByteLru::is_empty`].
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// See [`ByteLru::bytes`].
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.lock().bytes()
    }

    /// See [`ByteLru::counters`].
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.lock().counters()
    }

    fn lock(&self) -> MutexGuard<'_, ByteLru<K, V>> {
        // A panic inside the lock can only come from the key's `Hash` or `Eq`. The cache would
        // then be at worst a little off in its accounting, and eviction is always safe, so a
        // poisoned lock is still worth using.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A value owning `0` bytes on the heap.
    #[derive(Debug, PartialEq, Eq)]
    struct Blob(usize);

    impl HeapBytes for Blob {
        fn heap_bytes(&self) -> usize {
            self.0
        }
    }

    /// The charge of an empty `Blob`.
    const EMPTY: usize = mem::size_of::<Blob>() + ENTRY_OVERHEAD_BYTES;

    fn blob(heap: usize) -> Arc<Blob> {
        Arc::new(Blob(heap))
    }

    /// The internal accounting agrees with the entries, and stays within the budget.
    fn check_invariants(cache: &ByteLru<u32, Blob>) {
        let charged: usize = cache.entries.values().map(|entry| entry.charge).sum();
        assert_eq!(cache.bytes(), charged);
        assert!(cache.bytes() <= cache.budget());
        assert_eq!(cache.by_use.len(), cache.entries.len());
        for (tick, key) in &cache.by_use {
            assert_eq!(cache.entries[key].tick, *tick);
        }
    }

    #[test]
    fn entries_are_charged_heap_bytes_size_and_overhead() {
        assert_eq!(ByteLru::<u32, Blob>::charge(&Blob(1000)), 1000 + EMPTY);
        assert_eq!(ByteLru::<u32, Blob>::charge(&Blob(usize::MAX)), usize::MAX);
    }

    #[test]
    fn eviction_order_follows_use_not_insertion() {
        let mut cache = ByteLru::new(3 * EMPTY);
        for key in [1, 2, 3] {
            assert!(matches!(
                cache.insert(key, blob(0)),
                Insertion::Stored { evicted: 0 }
            ));
        }
        // 1 was inserted first but is now the most recently used; 2 is the oldest in use.
        assert!(cache.get(&1).is_some());
        assert!(matches!(
            cache.insert(4, blob(0)),
            Insertion::Stored { evicted: 1 }
        ));
        assert!(!cache.contains_key(&2));
        assert!(cache.contains_key(&1) && cache.contains_key(&3) && cache.contains_key(&4));
        assert!(matches!(
            cache.insert(5, blob(0)),
            Insertion::Stored { evicted: 1 }
        ));
        assert!(!cache.contains_key(&3));
        check_invariants(&cache);
    }

    #[test]
    fn a_large_entry_evicts_as_many_as_it_needs() {
        let mut cache = ByteLru::new(4 * EMPTY);
        for key in 1..=4 {
            let _ = cache.insert(key, blob(0));
        }
        // Charged three empty entries' worth, so three of the four oldest go.
        assert!(matches!(
            cache.insert(9, blob(2 * EMPTY)),
            Insertion::Stored { evicted: 3 }
        ));
        assert_eq!(cache.len(), 2);
        assert!(cache.contains_key(&4) && cache.contains_key(&9));
        check_invariants(&cache);
    }

    #[test]
    fn the_byte_total_never_exceeds_the_budget_across_a_scripted_sequence() {
        let budget = 20 * EMPTY;
        let mut cache = ByteLru::new(budget);
        // A fixed linear congruential sequence: sizes from empty to a quarter of the budget,
        // keys from a small range so that replacements and hits are common.
        let mut state: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            usize::try_from(state >> 33).unwrap()
        };
        for step in 0..10_000 {
            let key = u32::try_from(next() % 64).unwrap();
            if next() % 3 == 0 {
                let _ = cache.get(&key);
            } else {
                let heap = next() % (budget / 4);
                let _ = cache.insert(key, blob(heap));
            }
            assert!(cache.bytes() <= budget, "over budget at step {step}");
            if step % 100 == 0 {
                check_invariants(&cache);
            }
        }
        check_invariants(&cache);
        let counters = cache.counters();
        assert!(counters.hits() > 0 && counters.misses() > 0 && counters.evictions() > 0);
    }

    #[test]
    fn an_oversized_insert_leaves_the_cache_unchanged() {
        let mut cache = ByteLru::new(4 * EMPTY);
        let _ = cache.insert(1, blob(0));
        let _ = cache.insert(2, blob(EMPTY));
        let before = (cache.len(), cache.bytes());
        let oversized = blob(4 * EMPTY);
        match cache.insert(3, Arc::clone(&oversized)) {
            Insertion::Refused(value) => assert!(Arc::ptr_eq(&value, &oversized)),
            Insertion::Stored { .. } => panic!("an entry above the budget was stored"),
        }
        // Replacing a held key with an oversized value keeps the old value.
        assert!(matches!(
            cache.insert(2, blob(4 * EMPTY)),
            Insertion::Refused(_)
        ));
        assert_eq!((cache.len(), cache.bytes()), before);
        assert_eq!(cache.get(&2).as_deref(), Some(&Blob(EMPTY)));
        assert_eq!(cache.counters().refused(), 2);
        check_invariants(&cache);
    }

    #[test]
    fn an_entry_of_exactly_the_budget_is_stored() {
        let mut cache = ByteLru::new(EMPTY + 10);
        let _ = cache.insert(1, blob(0));
        assert!(matches!(
            cache.insert(2, blob(10)),
            Insertion::Stored { evicted: 1 }
        ));
        assert_eq!(cache.bytes(), cache.budget());
    }

    #[test]
    fn replacing_a_key_adjusts_the_total() {
        let mut cache = ByteLru::new(10 * EMPTY);
        let _ = cache.insert(1, blob(500));
        assert_eq!(cache.bytes(), 500 + EMPTY);
        assert!(matches!(
            cache.insert(1, blob(20)),
            Insertion::Stored { evicted: 0 }
        ));
        assert_eq!(cache.bytes(), 20 + EMPTY);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&1).as_deref(), Some(&Blob(20)));
        check_invariants(&cache);
    }

    #[test]
    fn empty_values_are_still_bounded_by_the_overhead() {
        let mut cache = ByteLru::new(10 * EMPTY);
        for key in 0..1_000 {
            let _ = cache.insert(key, blob(0));
        }
        assert_eq!(cache.len(), 10);
        assert_eq!(cache.bytes(), 10 * EMPTY);
        assert_eq!(cache.counters().evictions(), 990);
        check_invariants(&cache);
    }

    #[test]
    fn a_zero_budget_stores_nothing() {
        let mut cache = ByteLru::new(0);
        assert!(matches!(cache.insert(1, blob(0)), Insertion::Refused(_)));
        assert!(cache.is_empty());
    }

    #[test]
    fn hits_misses_and_evictions_are_counted() {
        let mut cache = ByteLru::new(2 * EMPTY);
        assert!(cache.get(&1).is_none());
        let _ = cache.insert(1, blob(0));
        assert!(cache.get(&1).is_some());
        assert!(cache.get(&1).is_some());
        let _ = cache.insert(2, blob(0));
        let _ = cache.insert(3, blob(0));
        let counters = cache.counters();
        assert_eq!(counters.hits(), 2);
        assert_eq!(counters.misses(), 1);
        assert_eq!(counters.evictions(), 1);
        assert_eq!(counters.entries(), 2);
        assert_eq!(counters.bytes(), 2 * EMPTY);
        assert_eq!(counters.budget(), 2 * EMPTY);
    }

    #[test]
    fn a_budget_of_usize_max_still_evicts_and_accounts_exactly() {
        let mut cache = ByteLru::new(usize::MAX);
        assert!(matches!(
            cache.insert(1, blob(usize::MAX - EMPTY)),
            Insertion::Stored { evicted: 0 }
        ));
        assert_eq!(cache.bytes(), usize::MAX);
        assert!(matches!(
            cache.insert(2, blob(0)),
            Insertion::Stored { evicted: 1 }
        ));
        assert_eq!(cache.bytes(), EMPTY);
        assert_eq!(cache.len(), 1);
        check_invariants(&cache);
    }

    /// A value that records, when dropped, whether its cache's lock was free.
    struct Probe {
        cache: std::sync::Weak<SharedByteLru<u32, Probe>>,
        dropped_unlocked: Arc<Mutex<Vec<bool>>>,
    }

    impl HeapBytes for Probe {
        fn heap_bytes(&self) -> usize {
            0
        }
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            if let Some(cache) = self.cache.upgrade() {
                let unlocked = cache.inner.try_lock().is_ok();
                self.dropped_unlocked.lock().unwrap().push(unlocked);
            }
        }
    }

    #[test]
    fn the_shared_cache_drops_evicted_and_replaced_values_outside_its_lock() {
        let probe_charge = ByteLru::<u32, Probe>::charge(&Probe {
            cache: std::sync::Weak::new(),
            dropped_unlocked: Arc::default(),
        });
        let cache = Arc::new(SharedByteLru::new(probe_charge));
        let dropped_unlocked = Arc::new(Mutex::new(Vec::new()));
        let probe = || {
            Arc::new(Probe {
                cache: Arc::downgrade(&cache),
                dropped_unlocked: Arc::clone(&dropped_unlocked),
            })
        };
        let _ = cache.insert(1, probe());
        // Replaces key 1.
        let _ = cache.insert(1, probe());
        // Evicts key 1.
        let _ = cache.insert(2, probe());
        assert_eq!(*dropped_unlocked.lock().unwrap(), [true, true]);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn the_shared_cache_stays_within_budget_under_threads() {
        let budget = 32 * EMPTY;
        let cache = SharedByteLru::new(budget);
        std::thread::scope(|scope| {
            for thread in 0..4_u32 {
                let cache = &cache;
                scope.spawn(move || {
                    for step in 0..2_000_u32 {
                        let key = (step * 7 + thread) % 100;
                        if cache.get(&key).is_none() {
                            let heap = usize::try_from(key).unwrap() * 3;
                            let _ = cache.insert(key, blob(heap));
                        }
                        assert!(cache.bytes() <= budget);
                    }
                });
            }
        });
        let inner = cache.lock();
        check_invariants(&inner);
        let counters = inner.counters();
        assert_eq!(counters.hits() + counters.misses(), 8_000);
    }
}
