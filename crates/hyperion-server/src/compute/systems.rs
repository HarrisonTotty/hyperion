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
//!
//! An observed range query reads each system's brief when its light left it, which needs the
//! system's stars (plan 12, P12.T3): [`SharedSystemCache::handle`] lends the same cache to it as the
//! sim's [`StarsCache`], so that an observed query asks for no stars the `SYSTEM` display has
//! built, and builds none twice (P12.T6).

use std::sync::Arc;

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::members::{NoInteriorCache, resolve_member};
use hyperion_sim::galaxy::placement::{
    ResolveSystemError, SystemKind, SystemOrigin, SystemRecord, resolve,
};
use hyperion_sim::id::{SystemId, SystemIdKind};
use hyperion_sim::observe::{StarsCache, stars_of};
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
    /// pool job. A catalogue feature's member is resolved once and generated through its member
    /// record, at its cluster's composition, which the grid's constructor would not give it
    /// ([`MemberRecord::stars`](hyperion_sim::galaxy::features::members::MemberRecord::stars)).
    /// A hit skips the resolution too: only an ID that resolved in this galaxy is ever stored
    /// under its key, and [`resolve`] is a pure function of the galaxy and the ID.
    ///
    /// # Errors
    ///
    /// - The [`ResolveSystemError`] of plan 03's [`resolve`] if `id` names no system of `galaxy`.
    /// - [`ResolveSystemError::KindNotGenerated`] for a member of the galactic centre, whose system
    ///   stage is not generated yet (plan 09, P09.T27).
    /// - [`ResolveSystemError::LayerNotGenerated`] for a rogue planet, which has no stars: its state
    ///   is plan 14's (plan 13, P13.T5.d). A free-floating brown dwarf is generated as a single
    ///   object through the stellar stage (P13.T5.a).
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
        // A feature member is resolved once, here, rather than through `resolve` and again for its
        // stars: its feature's interior is the costly part.
        let member = match id.kind() {
            SystemIdKind::FeatureMember(member) => {
                Some(resolve_member(galaxy, &NoInteriorCache, member)?)
            }
            _ => None,
        };
        let record = match &member {
            Some(member) => *member.record(),
            None => resolve(galaxy, id)?,
        };
        // The galactic centre's members resolve since plan 09's P09.T27, but their stars take the
        // centre's composition and its black hole is no star: not generated here yet.
        if matches!(record.origin(), SystemOrigin::CentreMember { .. }) {
            return Err(ResolveSystemError::KindNotGenerated);
        }
        if record.kind() == SystemKind::RoguePlanet {
            return Err(ResolveSystemError::LayerNotGenerated(record.layer()));
        }
        let stars = Arc::new(match member {
            Some(member) => member.stars(galaxy),
            None => SystemStars::generate(galaxy, &record),
        });
        // A system larger than the whole budget is handed back and still answered from; nothing
        // else needs doing with it.
        let _ = self.systems.insert(entry_key, Arc::clone(&stars));
        Ok(stars)
    }

    /// The stars of `record`'s system in `galaxy`, the galaxy `key` names: the cache's, or built
    /// by the sim's [`stars_of`] and stored.
    ///
    /// The record is a range query's hit, so it needs no resolving; the entry is the one
    /// [`get_or_generate`](Self::get_or_generate) stores for its ID, since both build a grid
    /// system through [`SystemStars::generate`] and a feature member through its member record.
    ///
    /// # Panics
    ///
    /// - If `galaxy`'s seed is not the seed of `key`, as [`get_or_generate`](Self::get_or_generate).
    /// - For a rogue planet, which has no stars, and for a member of the galactic centre, whose
    ///   stars are not generated yet, as [`stars_of`]: an observed query asks for neither.
    pub fn get_or_build(
        &self,
        key: GalaxyKey,
        galaxy: &Galaxy,
        record: &SystemRecord,
    ) -> Arc<SystemStars> {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a system cache entry is of one galaxy, and this is another's"
        );
        let entry_key = (key, record.id());
        if let Some(stars) = self.systems.get(&entry_key) {
            return stars;
        }
        let stars = Arc::new(stars_of(galaxy, &NoInteriorCache, record));
        // A system larger than the whole budget is handed back and still answered from.
        let _ = self.systems.insert(entry_key, Arc::clone(&stars));
        stars
    }

    /// One query's view of the cache, as the sim's [`StarsCache`] for the galaxy `key` names
    /// (plan 12, P12.T6).
    #[must_use]
    pub fn handle(&self, key: GalaxyKey) -> SystemStarsHandle<'_> {
        SystemStarsHandle { cache: self, key }
    }

    /// The systems held, the bytes they are charged, and the hits, misses, evictions and refusals
    /// so far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.systems.counters()
    }
}

/// One observed query's view of the [`SharedSystemCache`]: plan 12's [`StarsCache`], over the
/// cache the `SYSTEM` display shares (P12.T6).
///
/// Without it an observed 50 ly query builds every system's stars, 25 times the plain query's cost
/// (P12.T3 as built: 240 ms against 9.45 ms cold); with it a repeated query builds none.
#[derive(Debug, Clone, Copy)]
pub struct SystemStarsHandle<'a> {
    cache: &'a SharedSystemCache,
    key: GalaxyKey,
}

impl SystemStarsHandle<'_> {
    /// Which galaxy's systems this handle lends.
    #[must_use]
    pub fn galaxy(&self) -> GalaxyKey {
        self.key
    }
}

impl StarsCache for SystemStarsHandle<'_> {
    /// Lends the stars of `record`'s system, building them if the cache has not got them
    /// ([`SharedSystemCache::get_or_build`]).
    ///
    /// `f` runs with no lock held, on stars this call holds an `Arc` of, so the cache may evict
    /// anything meanwhile without touching what `f` sees.
    ///
    /// # Panics
    ///
    /// As [`SharedSystemCache::get_or_build`]: if `galaxy` is not the handle's, and for a rogue
    /// planet or a member of the galactic centre, which the observed query never asks for.
    fn with_stars<R>(
        &mut self,
        galaxy: &Galaxy,
        record: &SystemRecord,
        f: impl FnOnce(&SystemStars) -> R,
    ) -> R {
        f(&self.cache.get_or_build(self.key, galaxy, record))
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

    /// The brief model of the system `record` of `galaxy`, the galaxy `key` names: the cache's, or
    /// built and stored.
    ///
    /// The record comes from a range query's hit, so a grid system needs no resolving. Building
    /// one costs its primary's main sequence or its track ([`BriefModel::new`]), so this belongs
    /// on a pool job. A catalogue feature's member is routed through its member record
    /// ([`BriefModel::of_record`]), at its cluster's composition; the server keeps no feature
    /// interiors yet, so each member's miss builds its feature's interior again.
    ///
    /// # Panics
    ///
    /// - If `galaxy`'s seed is not the seed of `key`, as [`SharedSystemCache::get_or_generate`].
    /// - For a rogue planet, which has no stellar state, as [`BriefModel::new`]: the range
    ///   handler asks for no brief of one.
    /// - For a member of the galactic centre, whose stars are not generated yet, as
    ///   [`BriefModel::of_record`]: the range handler's sources place none.
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
        let model = Arc::new(BriefModel::of_record(galaxy, &NoInteriorCache, record));
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

    /// A free-floating brown dwarf takes the stellar stage as a single object and is stored (plan
    /// 13, P13.T5.a); a rogue planet has no stars, so it is refused with `LayerNotGenerated` and
    /// not stored.
    #[test]
    fn a_brown_dwarf_is_generated_and_a_rogue_planet_refused() {
        use hyperion_sim::coords::GalacticPosition;
        let cache = SharedSystemCache::new(64 << 20);
        for layer in [Layer::BrownDwarf, Layer::RoguePlanet] {
            let mut records = Vec::new();
            // Cells along the solar circle until one holds an object.
            let found = (0..64)
                .find_map(|i| {
                    let at =
                        GalacticPosition::from_light_years([16.0 * f64::from(i), 26_000.0, 0.0])
                            .expect("in the cube");
                    let key = CellKey::containing(layer, &at).expect("in the cube");
                    generate_cell(galaxy(), key, &mut records);
                    records.first().copied()
                })
                .expect("the solar circle holds free-floating objects");
            let answer = cache.get_or_generate(key(), galaxy(), found.id());
            if layer == Layer::BrownDwarf {
                let stars = answer.expect("a brown dwarf takes the stellar stage");
                assert_eq!(stars.star_count(), 1);
                assert_eq!(*stars.record(), found);
            } else {
                assert_eq!(answer, Err(ResolveSystemError::LayerNotGenerated(layer)));
            }
        }
        // The brown dwarf is stored; the rogue planet is not.
        assert_eq!(cache.counters().entries(), 1);
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

    /// The observed query's handle lends what the cache stores for the system's ID, and stores
    /// what it builds under it, so that the handle and `system_summary` share every entry.
    #[test]
    fn the_stars_handle_shares_the_entries_of_the_systems_ids() {
        let cache = SharedSystemCache::new(64 << 20);
        let [first, second] = systems(2)[..] else {
            unreachable!("two systems asked for")
        };
        let summarised = cache.get_or_generate(key(), galaxy(), first).unwrap();
        let record = resolve(galaxy(), first).unwrap();
        let mut handle = cache.handle(key());
        assert_eq!(handle.galaxy(), key());
        let lent = handle.with_stars(galaxy(), &record, SystemStars::clone);
        assert_eq!(lent, *summarised);
        assert_eq!(cache.counters().hits(), 1);

        let other = resolve(galaxy(), second).unwrap();
        let built = handle.with_stars(galaxy(), &other, SystemStars::clone);
        assert_eq!(built, SystemStars::generate(galaxy(), &other));
        let stored = cache.get_or_generate(key(), galaxy(), second).unwrap();
        assert_eq!(*stored, built);
        assert_eq!((cache.counters().hits(), cache.counters().misses()), (2, 2));
    }

    /// A catalogue feature's member, one of each stellar band a feature near the Sun has, is
    /// generated through its member record, at its cluster's composition, by both caches: its
    /// stars are `MemberRecord::stars` and its brief model `BriefModel::of_member`, not the grid
    /// constructors' (whose metallicity draw a member, with no density component, trips in debug
    /// builds).
    #[test]
    #[ignore = "slow: a full-potential galaxy and a feature interior"]
    fn a_feature_member_is_generated_through_its_member_record() {
        use hyperion_sim::coords::GalacticPosition;
        use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, NoFeatureCache};
        use hyperion_sim::galaxy::features::members::FeatureInterior;
        use hyperion_sim::galaxy::imf::MassBand;
        use hyperion_sim::galaxy::params::GalaxyParams;
        use hyperion_sim::units::LightYears;

        const MEMBER_SEED: u64 = 0x1203_7000_0000_0000;
        let galaxy = Galaxy::from_params(Seed::new(MEMBER_SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
            .with_full_potential();
        let key = GalaxyKey::new(MEMBER_SEED, GENERATOR_VERSION);
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
        let interior =
            FeatureCatalogue::near(&galaxy, &sun, LightYears::new(3_000.0), &NoFeatureCache)
                .find_map(|f| FeatureInterior::of(&galaxy, &f))
                .expect("a feature with members lies within 3,000 ly of the Sun");
        let systems = SharedSystemCache::new(64 << 20);
        let briefs = SharedBriefCache::new(64 << 20);
        let mut members = Vec::new();
        let mut checked = 0;
        for band in [MassBand::A, MassBand::C, MassBand::E] {
            let found = interior.grid().owned_cells().find_map(|c| {
                interior.members_in_cell(&galaxy, band, c, &mut members);
                members.first().map(|(m, _)| *m)
            });
            let Some(member) = found else { continue };
            let record = member.record();
            let stars = systems
                .get_or_generate(key, &galaxy, record.id())
                .expect("a member resolves");
            assert_eq!(*stars, member.stars(&galaxy));
            assert_eq!(stars.primary().composition(), member.composition());
            assert_eq!(
                *briefs.get_or_build(key, &galaxy, record),
                BriefModel::of_member(&galaxy, &member)
            );
            checked += 1;
        }
        assert!(checked >= 1, "the feature has no member in bands A, C or E");
    }
}
