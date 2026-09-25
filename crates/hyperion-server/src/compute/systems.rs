//! The caches of generated systems: each system's stars, and each system's range brief, bounded in
//! bytes (plan 06, P06.T34).
//!
//! A system's [`SystemStars`] holds every star's track and remnant, which costs a millisecond or
//! two per evolved star to build (plan 06's Risks; ruling 46 of 2026-09-22), and the `SYSTEM`
//! display asks for the same system again at every time it is scrubbed to. So the server keeps what
//! it built: [`SharedSystemCache`] is a [`SharedByteLru`] over `(GalaxyKey, SystemId)`, so that two
//! saves of one seed share their systems and nothing generated under one generator version is lent
//! for another (plan 04, design note 23).
//!
//! An entry is a system's stars as state at the epoch, never a summary at some time: the models
//! answer for any time inside the clock window, so a request at another time reuses the entry, and
//! eviction is always safe, since nothing generated is remembered anywhere else. Two threads may
//! build one system at the same time; the second insert replaces an equal value, which is cheaper
//! than coordinating (the same ruling as [`SharedByteLru`]'s).
//!
//! [`SharedBriefCache`] holds what a range query's row needs instead: each system's
//! [`BriefModel`] (plan 06, P06.T38.e), its primary alone by the cheapest exact route and its star
//! count, under the same key and the same rules. It is a cache of its own because its entries are a
//! few kilobytes where a system's stars can be tens, and a 20,000-row answer would otherwise evict
//! every system the `SYSTEM` display is looking at.

use std::sync::Arc;

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{ResolveSystemError, SystemRecord, resolve};
use hyperion_sim::id::SystemId;
use hyperion_sim::stellar::brief::BriefModel;
use hyperion_sim::stellar::system::SystemStars;

use super::GalaxyKey;
use crate::cache::{HeapBytes, LruCounters, SharedByteLru};

/// What names a system in the cache: the galaxy it belongs to, and the system itself.
type SystemEntryKey = (GalaxyKey, SystemId);

impl HeapBytes for SystemStars {
    /// The sim's own count of what the stars own on the heap: every star's track and the
    /// hierarchy's lists ([`SystemStars::heap_bytes`]).
    fn heap_bytes(&self) -> usize {
        Self::heap_bytes(self)
    }
}

/// The generated systems of every galaxy the server holds, in one byte budget.
#[derive(Debug)]
pub struct SharedSystemCache {
    systems: SharedByteLru<SystemEntryKey, SystemStars>,
}

impl SharedSystemCache {
    /// An empty cache that holds at most `budget_bytes` of charged systems.
    ///
    /// A budget of zero caches nothing: every system is generated, answered from and dropped.
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            systems: SharedByteLru::new(budget_bytes),
        }
    }

    /// The stars of the system `id` names in `galaxy`, the galaxy `key` names: the cache's, or
    /// resolved and generated, then stored.
    ///
    /// Generation is the whole of plan 06's and plan 11's system stage
    /// ([`SystemStars::generate`]), which builds a track for every star, so this belongs on a
    /// pool job. A hit skips the resolution too: only an ID that resolved in this galaxy is ever
    /// stored under its key, and [`resolve`] is a pure function of the galaxy and the ID.
    ///
    /// # Errors
    ///
    /// The [`ResolveSystemError`] of plan 03's [`resolve`] if `id` names no system of `galaxy`.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of `key`, since its systems would then be stored under
    /// another galaxy's key. The server takes both from the request's own universe, so a mismatch
    /// is a bug.
    pub fn get_or_generate(
        &self,
        key: GalaxyKey,
        galaxy: &Galaxy,
        id: SystemId,
    ) -> Result<Arc<SystemStars>, ResolveSystemError> {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a system cache entry is of one galaxy, and this is another's"
        );
        let entry_key = (key, id);
        if let Some(stars) = self.systems.get(&entry_key) {
            return Ok(stars);
        }
        let record = resolve(galaxy, id)?;
        let stars = Arc::new(SystemStars::generate(galaxy, &record));
        // A system larger than the whole budget is handed back and still answered from; nothing
        // else needs doing with it.
        let _ = self.systems.insert(entry_key, Arc::clone(&stars));
        Ok(stars)
    }

    /// The systems held, the bytes they are charged, and the hits, misses, evictions and refusals
    /// so far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.systems.counters()
    }
}

impl HeapBytes for BriefModel {
    /// The sim's own count of what the model owns on the heap: its primary's track, if it has one
    /// ([`BriefModel::heap_bytes`]).
    fn heap_bytes(&self) -> usize {
        Self::heap_bytes(self)
    }
}

/// The range briefs' models of every galaxy the server holds, in one byte budget (plan 06,
/// P06.T34).
#[derive(Debug)]
pub struct SharedBriefCache {
    briefs: SharedByteLru<SystemEntryKey, BriefModel>,
}

impl SharedBriefCache {
    /// An empty cache that holds at most `budget_bytes` of charged models.
    ///
    /// A budget of zero caches nothing: every model is built, answered from and dropped.
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            briefs: SharedByteLru::new(budget_bytes),
        }
    }

    /// The brief model of the grid system `record` of `galaxy`, the galaxy `key` names: the
    /// cache's, or built and stored.
    ///
    /// The record comes from a range query's hit, so it needs no resolving. Building one costs
    /// its primary's main sequence or its track ([`BriefModel::new`]), so this belongs on a pool
    /// job.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of `key`, as [`SharedSystemCache::get_or_generate`].
    pub fn get_or_build(
        &self,
        key: GalaxyKey,
        galaxy: &Galaxy,
        record: &SystemRecord,
    ) -> Arc<BriefModel> {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a brief cache entry is of one galaxy, and this is another's"
        );
        let entry_key = (key, record.id());
        if let Some(model) = self.briefs.get(&entry_key) {
            return model;
        }
        let model = Arc::new(BriefModel::new(galaxy, record));
        // A model larger than the whole budget is handed back and still answered from.
        let _ = self.briefs.insert(entry_key, Arc::clone(&model));
        model
    }

    /// The models held, the bytes they are charged, and the hits, misses, evictions and refusals
    /// so far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.briefs.counters()
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::galaxy::placement::{CellKey, candidate_count, generate_cell};
    use hyperion_sim::id::Layer;
    use hyperion_sim::{GENERATOR_VERSION, Seed};

    use super::*;
    use crate::cache::ENTRY_OVERHEAD_BYTES;

    /// The seed of the galaxy these tests generate systems in, the integration tests' own.
    const SEED: u64 = 0x4d2;

    fn galaxy() -> &'static Galaxy {
        static GALAXY: std::sync::OnceLock<Galaxy> = std::sync::OnceLock::new();
        GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
    }

    fn key() -> GalaxyKey {
        GalaxyKey::new(SEED, GENERATOR_VERSION)
    }

    /// The first `n` systems of layer C at the solar circle.
    fn systems(n: usize) -> Vec<SystemId> {
        let mut cell = Vec::new();
        let mut ids = Vec::new();
        for step in 0.. {
            generate_cell(
                galaxy(),
                CellKey::new(Layer::C, [step, 812, 0]).expect("a cell of the grid"),
                &mut cell,
            );
            ids.extend(cell.drain(..).map(|record| record.id()));
            if ids.len() >= n {
                ids.truncate(n);
                return ids;
            }
        }
        unreachable!("the loop returns")
    }

    #[test]
    fn a_system_is_generated_once_and_then_lent() {
        let cache = SharedSystemCache::new(64 << 20);
        let id = systems(1)[0];
        let first = cache.get_or_generate(key(), galaxy(), id).unwrap();
        let expected = SystemStars::generate(galaxy(), &resolve(galaxy(), id).unwrap());
        assert_eq!(*first, expected);
        let second = cache.get_or_generate(key(), galaxy(), id).unwrap();
        assert!(
            Arc::ptr_eq(&first, &second),
            "the second lookup is the cache's"
        );
        let counters = cache.counters();
        assert_eq!(
            (counters.misses(), counters.hits(), counters.entries()),
            (1, 1, 1)
        );
        assert_eq!(
            counters.bytes(),
            expected.heap_bytes() + size_of::<SystemStars>() + ENTRY_OVERHEAD_BYTES
        );
    }

    #[test]
    fn an_id_that_names_no_system_is_refused_and_not_stored() {
        let cache = SharedSystemCache::new(64 << 20);
        let cell = CellKey::of(systems(1)[0]).expect("a grid ID");
        // The index after the cell's last candidate names no candidate the cell drew.
        let beyond = cell
            .candidate_id(candidate_count(galaxy(), cell))
            .expect("a cell at the solar circle has room for another index");
        assert_eq!(
            cache.get_or_generate(key(), galaxy(), beyond),
            Err(ResolveSystemError::NoSuchSystem)
        );
        assert_eq!(cache.counters().entries(), 0);
    }

    #[test]
    fn eviction_is_always_safe_and_the_answer_never_changes() {
        let ids = systems(6);
        let roomy = SharedSystemCache::new(64 << 20);
        // Room for about one system: every other lookup evicts.
        let tight = SharedSystemCache::new(
            SystemStars::generate(galaxy(), &resolve(galaxy(), ids[0]).unwrap()).heap_bytes()
                + size_of::<SystemStars>()
                + ENTRY_OVERHEAD_BYTES,
        );
        let none = SharedSystemCache::new(0);
        for _ in 0..2 {
            for &id in &ids {
                let expected = roomy.get_or_generate(key(), galaxy(), id).unwrap();
                assert_eq!(
                    tight.get_or_generate(key(), galaxy(), id).unwrap(),
                    expected
                );
                assert_eq!(none.get_or_generate(key(), galaxy(), id).unwrap(), expected);
            }
        }
        assert!(tight.counters().evictions() > 0 || tight.counters().refused() > 0);
        assert_eq!(none.counters().entries(), 0);
        assert_eq!(roomy.counters().hits(), 6);
    }

    #[test]
    #[should_panic(expected = "this is another's")]
    fn a_galaxy_of_another_seed_is_a_bug() {
        let cache = SharedSystemCache::new(64 << 20);
        let _ = cache.get_or_generate(
            GalaxyKey::new(SEED + 1, GENERATOR_VERSION),
            galaxy(),
            systems(1)[0],
        );
    }

    #[test]
    fn a_brief_model_is_built_once_and_every_cache_answers_the_same() {
        let records: Vec<SystemRecord> = systems(6)
            .into_iter()
            .map(|id| resolve(galaxy(), id).unwrap())
            .collect();
        let roomy = SharedBriefCache::new(64 << 20);
        let none = SharedBriefCache::new(0);
        for _ in 0..2 {
            for record in &records {
                let expected = BriefModel::new(galaxy(), record);
                assert_eq!(*roomy.get_or_build(key(), galaxy(), record), expected);
                assert_eq!(*none.get_or_build(key(), galaxy(), record), expected);
            }
        }
        let counters = roomy.counters();
        assert_eq!(
            (counters.misses(), counters.hits(), counters.entries()),
            (6, 6, 6)
        );
        assert_eq!(none.counters().entries(), 0);
    }
}
