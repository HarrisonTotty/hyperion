//! The surface service's cache of coarse fields, bounded by bytes (rendering plan R09, Design note
//! 19).

use std::sync::Arc;

use hyperion_sim::id::BodyId;
use hyperion_surface::field::CoarseField;

use crate::cache::{ByteLru, HeapBytes, Insertion, LruCounters, SharedByteLru};
use crate::compute::GalaxyKey;

/// What names a body's coarse field: the universe's galaxy, whose `(seed, generator_version)`
/// every cache key carries (plan 04, design note 23), and the body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldKey {
    galaxy: GalaxyKey,
    body: BodyId,
}

impl FieldKey {
    /// The field of `body` in the universe of `galaxy`.
    #[must_use]
    pub const fn new(galaxy: GalaxyKey, body: BodyId) -> Self {
        Self { galaxy, body }
    }

    /// The universe's galaxy: its seed, from which the body's surface seed is drawn, and the
    /// generator version.
    #[must_use]
    pub const fn galaxy(self) -> GalaxyKey {
        self.galaxy
    }

    /// The body.
    #[must_use]
    pub const fn body(self) -> BodyId {
        self.body
    }
}

impl HeapBytes for CoarseField {
    /// The field's own count, by its vectors' capacities: its records, its craters and their
    /// reaches, its per-cell crater index and its palette ([`CoarseField::heap_bytes`]).
    fn heap_bytes(&self) -> usize {
        Self::heap_bytes(self)
    }
}

/// The coarse fields computed so far, of every universe the server holds, in one byte budget.
///
/// Each field is charged its heap bytes ([`CoarseField::heap_bytes`]), its own size and the
/// cache's [`ENTRY_OVERHEAD_BYTES`](crate::cache::ENTRY_OVERHEAD_BYTES); the least recently used
/// are evicted first, and a field larger than the whole budget is handed back and not kept.
/// Eviction is always safe: a field is a function of its key.
#[derive(Debug)]
pub struct FieldCache {
    fields: SharedByteLru<FieldKey, CoarseField>,
}

impl FieldCache {
    /// An empty cache that holds at most `budget_bytes` of charged fields.
    ///
    /// A budget of zero keeps none.
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            fields: SharedByteLru::new(budget_bytes),
        }
    }

    /// The field of `key`, if it is held, marking it the most recently used; a hit or a miss is
    /// counted either way.
    #[must_use]
    pub fn get(&self, key: &FieldKey) -> Option<Arc<CoarseField>> {
        self.fields.get(key)
    }

    /// Keeps `field` under `key` as the most recently used, evicting the least recently used until
    /// it fits; a field larger than the whole budget is handed back.
    pub fn insert(&self, key: FieldKey, field: Arc<CoarseField>) -> Insertion<CoarseField> {
        self.fields.insert(key, field)
    }

    /// What a field is charged: its heap bytes, its own size and the entry's overhead.
    #[must_use]
    pub fn charge(field: &CoarseField) -> usize {
        ByteLru::<FieldKey, CoarseField>::charge(field)
    }

    /// The fields held, the bytes they are charged, the budget, and the hits, misses, evictions
    /// and refusals so far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.fields.counters()
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::GENERATOR_VERSION;
    use hyperion_sim::id::SystemId;
    use hyperion_surface::testing::{SyntheticWorld, synthetic_field};

    use super::*;
    use crate::cache::ENTRY_OVERHEAD_BYTES;

    fn key(index: u16) -> FieldKey {
        let system = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
        FieldKey::new(
            GalaxyKey::new(0x4d2, GENERATOR_VERSION),
            BodyId::new(system, index),
        )
    }

    /// A field is charged its own count of its heap bytes, its size and the entry's overhead, and
    /// the cache holds no more than its budget: a second field evicts the first where only one
    /// fits, and one larger than the whole budget is refused.
    #[test]
    fn a_field_is_charged_its_heap_bytes_and_the_budget_holds() {
        let field = Arc::new(synthetic_field(SyntheticWorld::CeresLike));
        let charge = FieldCache::charge(&field);
        assert_eq!(
            charge,
            field.heap_bytes() + size_of::<CoarseField>() + ENTRY_OVERHEAD_BYTES
        );
        let cache = FieldCache::new(charge + charge / 2);
        assert!(matches!(
            cache.insert(key(0x0100), Arc::clone(&field)),
            Insertion::Stored { evicted: 0 }
        ));
        assert!(matches!(
            cache.insert(key(0x0200), Arc::clone(&field)),
            Insertion::Stored { evicted: 1 }
        ));
        assert!(cache.get(&key(0x0100)).is_none(), "the first was evicted");
        assert!(cache.get(&key(0x0200)).is_some());
        let counters = cache.counters();
        assert_eq!(
            (counters.entries(), counters.bytes(), counters.evictions()),
            (1, charge, 1)
        );
        let small = FieldCache::new(charge - 1);
        assert!(matches!(
            small.insert(key(0x0100), field),
            Insertion::Refused(_)
        ));
        assert_eq!(
            (small.counters().entries(), small.counters().refused()),
            (0, 1)
        );
    }

    #[test]
    fn a_key_names_its_galaxy_and_body() {
        let key = key(0x0300);
        assert_eq!(key.galaxy(), GalaxyKey::new(0x4d2, GENERATOR_VERSION));
        assert_eq!(key.body().body_index(), 0x0300);
    }
}
