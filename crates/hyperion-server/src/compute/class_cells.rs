//! The catalogue classes' cells as the range query reads them, shared between queries (plan 09,
//! P09.T40).
//!
//! A class host is found in the class cells within the class's 4,096 ly reach of a query's sphere:
//! some 2,900 512 ly cells a class about a 50 ly sphere. What a hit is made from in each cell is
//! the sim's [`ClassCellSystems`], epoch state, a pure function of the galaxy and the cell. So
//! [`SharedClassCellCache`] keeps it, a [`SharedByteLru`] over `(GalaxyKey, CatalogueCellKey)`
//! charged each cell's heap bytes, and a query near another reads its cells warm. A cold query's
//! cells are made ahead of its hits by its jobs, several at once
//! ([`served_range_query`](super::served_range_query)); a cell is otherwise made by the job whose
//! query first reads it. A cell evicted is made again, and changes no answer.
//!
//! A placed class's cell keeps at most the places a quarter of the budget holds
//! ([`SharedClassCellCache::most_kept_places`]): a denser one holds only its count, and each query
//! reads its places again as it walks it, rather than making it whole. A cold range query at the
//! black hole made the eight stellar merger cells there whole, 557 MiB each, beside the slow
//! accreting white dwarfs' eight of 93 MiB, and the server peaked at 5.40–5.55 GiB; with every cell
//! past the whole budget read as the query walks it, at 0.94–0.99 GiB, the cells of 8 to 32 MiB
//! about the centre made side by side by its jobs; past a quarter, at 329–331 MiB, the answer the
//! same throughout (a 0.78 ly sphere from layer D, two or three runs each; pod, 32 vCPU, release,
//! 2026-11-05; `tests/centre_memory.rs`; the mergers' balance, part 3).

use std::cell::RefCell;
use std::sync::Arc;

use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
#[cfg(doc)]
use hyperion_sim::galaxy::catalogue_classes::CachedClassSource;
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCellCache, CatalogueClassSource, ClassCellSystems,
};

use super::GalaxyKey;
use crate::cache::{HeapBytes, LruCounters, SharedByteLru};

/// The class cells a job makes ahead of a query before it stores them under one hold of the
/// cache's lock, as the grid's cell jobs store theirs (`cells.rs`).
const MADE_CHUNK_CELLS: usize = 128;

impl HeapBytes for ClassCellSystems {
    /// The sim's own count of what the cell's systems own on the heap
    /// ([`ClassCellSystems::heap_bytes`]).
    fn heap_bytes(&self) -> usize {
        Self::heap_bytes(self)
    }
}

/// The catalogue classes' cells of every galaxy the server holds, in one byte budget (P09.T40).
#[derive(Debug)]
pub struct SharedClassCellCache {
    cells: SharedByteLru<(GalaxyKey, CatalogueCellKey), ClassCellSystems>,
    /// The most places a placed class's cell keeps: as many as a quarter of the budget holds.
    most_places: u32,
}

impl SharedClassCellCache {
    /// An empty cache that holds at most `budget_bytes` of charged cells.
    ///
    /// A budget of zero caches nothing: every cell is made, lent and dropped.
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        let places = budget_bytes / 4 / size_of::<(u32, GalacticPosition)>();
        Self {
            cells: SharedByteLru::new(budget_bytes),
            most_places: u32::try_from(places).unwrap_or(u32::MAX),
        }
    }

    /// The most places a placed class's cell it keeps holds: as many as a quarter of the budget
    /// holds, 48 B each, 174,762 at the server's 32 MiB. A denser cell keeps none, and each query
    /// reads them again as it walks the cell: a cell past a quarter would evict most of what the
    /// cache holds, and one past the whole budget was made whole and dropped, as the eight of 557
    /// MiB about the black hole were (module documentation).
    #[must_use]
    pub fn most_kept_places(&self) -> u32 {
        self.most_places
    }

    /// The sim's [`CatalogueClassCellCache`] over this cache, for the galaxy `galaxy` names.
    #[must_use]
    pub fn handle(&self, galaxy: GalaxyKey) -> ClassCellHandle<'_> {
        ClassCellHandle {
            cache: self,
            galaxy,
        }
    }

    /// The cells held, the bytes they are charged, and the hits, misses, evictions and refusals so
    /// far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.cells.counters()
    }

    /// The cells of `cells` of the galaxy `galaxy` names that the cache does not hold, in order,
    /// asked under one hold of its lock and without counting a lookup or marking a use: what a
    /// served query asks before it makes its class cells ahead of its hits
    /// ([`served_range_query`](super::served_range_query)).
    #[must_use]
    pub fn lacking(
        &self,
        galaxy: GalaxyKey,
        cells: impl IntoIterator<Item = CatalogueCellKey>,
    ) -> Vec<CatalogueCellKey> {
        self.cells
            .absent(cells.into_iter().map(|cell| (galaxy, cell)))
            .into_iter()
            .map(|(_, cell)| cell)
            .collect()
    }

    /// Makes the systems of each of `cells` of `galaxy`, the galaxy `key` names, as a query's hits
    /// make them ([`CachedClassSource::read_ahead`]), and stores them [`MADE_CHUNK_CELLS`] at a
    /// time, each chunk under one hold of the cache's lock: how a served query's jobs make its
    /// class cells ahead of its hits, several at once
    /// ([`served_range_query`](super::served_range_query)). A cell held already is made again and
    /// replaced by its equal; one larger than the whole budget is dropped.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of `key`, as a handle's lookup.
    pub(crate) fn read_ahead(&self, key: GalaxyKey, galaxy: &Galaxy, cells: &[CatalogueCellKey]) {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a class cell cache stores the cells of one galaxy under its key, and this is another's"
        );
        let making = Making {
            made: RefCell::new(Vec::with_capacity(MADE_CHUNK_CELLS)),
            most_places: self.most_places,
        };
        let source = CatalogueClassSource::new().with_cells(&making);
        for chunk in cells.chunks(MADE_CHUNK_CELLS) {
            source.read_ahead(galaxy, chunk);
            let mut made = making.made.borrow_mut();
            // A refused cell is the hits' to make again, as a handle's would be.
            drop(
                self.cells
                    .insert_all(made.drain(..).map(|(cell, systems)| ((key, cell), systems))),
            );
        }
    }
}

/// One job's view of the class cell cache for cells nobody holds: the sim's
/// [`CatalogueClassCellCache`], which makes each cell it is asked for, lends it, and keeps it for
/// [`SharedClassCellCache::read_ahead`] to store with the others a chunk at a time.
struct Making {
    /// The cells made and not yet stored.
    made: RefCell<Vec<(CatalogueCellKey, Arc<ClassCellSystems>)>>,
    /// The cache's most kept places, so that a cell made ahead is the one its hits would make.
    most_places: u32,
}

impl CatalogueClassCellCache for Making {
    fn systems(
        &self,
        _galaxy: &Galaxy,
        key: CatalogueCellKey,
        make: &mut dyn FnMut() -> ClassCellSystems,
    ) -> Arc<ClassCellSystems> {
        let systems = Arc::new(make());
        self.made.borrow_mut().push((key, Arc::clone(&systems)));
        systems
    }

    fn most_kept_places(&self) -> u32 {
        self.most_places
    }
}

/// One galaxy's view of the [`SharedClassCellCache`]: the sim's [`CatalogueClassCellCache`], over
/// the cache every request shares.
#[derive(Debug, Clone, Copy)]
pub struct ClassCellHandle<'a> {
    cache: &'a SharedClassCellCache,
    galaxy: GalaxyKey,
}

impl CatalogueClassCellCache for ClassCellHandle<'_> {
    /// Lends the cell's systems the cache holds, or those `make` returns, then held.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of this handle's key, since its cells would then be lent
    /// under another galaxy's key.
    fn systems(
        &self,
        galaxy: &Galaxy,
        key: CatalogueCellKey,
        make: &mut dyn FnMut() -> ClassCellSystems,
    ) -> Arc<ClassCellSystems> {
        assert_eq!(
            galaxy.seed().get(),
            self.galaxy.seed(),
            "a class cell cache handle lends the cells of one galaxy, and this is another's"
        );
        let entry_key = (self.galaxy, key);
        if let Some(systems) = self.cache.cells.get(&entry_key) {
            return systems;
        }
        let systems = Arc::new(make());
        // A cell larger than the whole budget is handed back and still lent; one another job stored
        // meanwhile is the same cell, and either is lent right.
        let _ = self.cache.cells.insert(entry_key, Arc::clone(&systems));
        systems
    }

    /// As many as the whole budget holds ([`SharedClassCellCache::most_kept_places`]).
    fn most_kept_places(&self) -> u32 {
        self.cache.most_places
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use hyperion_sim::GeneratorVersion;
    use hyperion_sim::Seed;
    use hyperion_sim::coords::GalacticPosition;
    use hyperion_sim::galaxy::catalogue_classes::{CatalogueClassSource, NoClassCellCache};
    use hyperion_sim::galaxy::imf::MassFunctionKind;
    use hyperion_sim::galaxy::params::GalaxyParams;
    use hyperion_sim::galaxy::query::{LayerSet, QuerySphere, SystemHit, SystemSource};
    use hyperion_sim::id::{Layer, SystemIdKind};
    use hyperion_sim::time::UniverseTime;
    use hyperion_sim::units::LightYears;

    use super::*;
    use crate::convert::origins::SEED;

    /// The server tests' galaxy under generator version 27's rules, every hold at 27 in force.
    fn galaxy_27() -> &'static Galaxy {
        static GALAXY: std::sync::OnceLock<Galaxy> = std::sync::OnceLock::new();
        GALAXY.get_or_init(|| {
            let seed = Seed::new(SEED);
            Galaxy::from_params_at_version(
                seed,
                GalaxyParams::from_seed(seed, MassFunctionKind::default()),
                GeneratorVersion::new(27),
            )
            .expect("a drawn galaxy builds")
        })
    }

    fn key() -> GalaxyKey {
        GalaxyKey::new(SEED, GeneratorVersion::new(27))
    }

    /// The classes' hits in `sphere`, their cells read through `cells`.
    fn hits(cells: &dyn CatalogueClassCellCache, sphere: &QuerySphere) -> Vec<SystemHit> {
        let stellar: LayerSet = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
            .into_iter()
            .collect();
        let mut out = Vec::new();
        CatalogueClassSource::new()
            .with_cells(cells)
            .systems_in_sphere(galaxy_27(), sphere, stellar, &mut out);
        out
    }

    /// Eviction never changes the classes' hits: queries in the outer disc and about the first
    /// class host they find give the same hits through no cache, a cache too small for one cell,
    /// one that evicts all the time and one that keeps every cell, cold and warm; and a warm query
    /// makes no cell.
    #[test]
    fn eviction_never_changes_the_classes_hits() {
        let outer = GalacticPosition::from_light_years([0.0, 48_000.0, 0.0]).expect("in the cube");
        let wide = QuerySphere::new(
            outer,
            LightYears::new(300.0),
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .expect("a sphere");
        let found = hits(&NoClassCellCache, &wide);
        let host = found
            .iter()
            .find(|hit| matches!(hit.id().kind(), SystemIdKind::Catalogue(_)))
            .expect("a class host within 300 ly of the outer point");
        let near = QuerySphere::new(
            *host.position(),
            LightYears::new(1.0),
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .expect("a sphere");
        let spheres = [wide, near];
        let expected: Vec<_> = spheres.iter().map(|s| hits(&NoClassCellCache, s)).collect();
        assert!(expected[1].iter().any(|hit| hit.id() == host.id()));
        for budget in [0, 2_000, 64 << 20] {
            let cache = SharedClassCellCache::new(budget);
            for pass in ["cold", "warm"] {
                for (sphere, expected) in spheres.iter().zip(&expected) {
                    assert_eq!(
                        &hits(&cache.handle(key()), sphere),
                        expected,
                        "{budget} {pass}"
                    );
                }
            }
        }
        let roomy = SharedClassCellCache::new(64 << 20);
        drop(hits(&roomy.handle(key()), &near));
        let before = roomy.counters();
        drop(hits(&roomy.handle(key()), &near));
        let after = roomy.counters();
        assert_eq!(
            after.misses(),
            before.misses(),
            "a warm query makes no cell"
        );
        assert!(after.hits() > before.hits());
        let classes: BTreeSet<_> = expected
            .iter()
            .flatten()
            .filter_map(|hit| match hit.id().kind() {
                SystemIdKind::Catalogue(id) => Some(id.class()),
                _ => None,
            })
            .collect();
        assert!(!classes.is_empty());
    }

    /// A handle refuses another galaxy's cells.
    #[test]
    #[should_panic(expected = "a class cell cache handle lends the cells of one galaxy")]
    fn a_handle_refuses_another_galaxy() {
        let cache = SharedClassCellCache::new(1 << 20);
        let other = GalaxyKey::new(SEED + 1, GeneratorVersion::new(27));
        let key = CatalogueCellKey::new(
            hyperion_sim::galaxy::catalogue_classes::ClassId::CORE_COLLAPSE,
            [0, 50, 0],
        )
        .expect("a cell");
        drop(
            cache
                .handle(other)
                .systems(galaxy_27(), key, &mut || -> ClassCellSystems {
                    unreachable!("never made")
                }),
        );
    }

    /// A placed class's cell keeps at most the places a quarter of the budget holds, 48 B each:
    /// 174,762 at the server's 32 MiB, none at a budget of nothing (the mergers' balance, part 3).
    #[test]
    fn a_cell_keeps_at_most_the_places_its_budget_holds() {
        assert_eq!(size_of::<(u32, GalacticPosition)>(), 48);
        let budget = crate::config::DEFAULT_CLASS_CELL_CACHE_MIB << 20;
        let cache = SharedClassCellCache::new(budget);
        assert_eq!(cache.most_kept_places(), 174_762);
        let key = GalaxyKey::new(9, hyperion_sim::GENERATOR_VERSION);
        assert_eq!(cache.handle(key).most_kept_places(), 174_762);
        assert_eq!(SharedClassCellCache::new(0).most_kept_places(), 0);
    }
}
