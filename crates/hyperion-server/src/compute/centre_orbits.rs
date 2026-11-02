//! The galactic centre's cells of members and their orbits as the range query reads them, shared
//! between queries (plan 09, P09.T37.b: P09.T40.a's `CentreOrbitCache`).
//!
//! A query near the black hole reads the centre's members cell by cell and band by band, each with
//! its trajectory ([`CentreCellOrbits`]: some 230 bytes a member, the sky's census's 32 among
//! them, and some 640 more for its exact orbit from version 25), a pure function of the galaxy, the band and the cell. So
//! [`SharedCentreOrbitCache`] keeps them, a [`SharedByteLru`] over `(GalaxyKey, MassBand,
//! NestedCell)` charged each cell's heap bytes, as the sim's `KeepCentreOrbits` keeps one galaxy's
//! but within a byte budget: a warm query about the centre places and prepares none of them again.
//! A cell is made by the job whose query first reads it; a cell evicted is made again, and changes
//! no answer.

use std::sync::Arc;

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::centre::members::CentrePlacement;
use hyperion_sim::galaxy::features::centre::{CentreCellOrbits, CentreOrbitCache};
use hyperion_sim::galaxy::features::nested::NestedCell;
use hyperion_sim::galaxy::imf::MassBand;

use super::GalaxyKey;
use crate::cache::{HeapBytes, LruCounters, SharedByteLru};

impl HeapBytes for CentreCellOrbits {
    /// The sim's own count of what the cell's trajectories own on the heap
    /// ([`CentreCellOrbits::heap_bytes`]).
    fn heap_bytes(&self) -> usize {
        Self::heap_bytes(self)
    }
}

/// What names a cell's orbits in the cache: its galaxy, its band and its cell.
type CentreOrbitKey = (GalaxyKey, MassBand, NestedCell);

/// The galactic centre's cells of members and their orbits of every galaxy the server holds, in
/// one byte budget (module documentation).
#[derive(Debug)]
pub struct SharedCentreOrbitCache {
    cells: SharedByteLru<CentreOrbitKey, CentreCellOrbits>,
}

impl SharedCentreOrbitCache {
    /// An empty cache that holds at most `budget_bytes` of charged cells.
    ///
    /// A budget of zero caches nothing: every cell is placed, lent and dropped.
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            cells: SharedByteLru::new(budget_bytes),
        }
    }

    /// The sim's [`CentreOrbitCache`] over this cache, for the galaxy `galaxy` names.
    #[must_use]
    pub fn handle(&self, galaxy: GalaxyKey) -> CentreOrbitHandle<'_> {
        CentreOrbitHandle {
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
}

/// One galaxy's view of the [`SharedCentreOrbitCache`]: the sim's [`CentreOrbitCache`], over the
/// cache every request shares.
#[derive(Debug, Clone, Copy)]
pub struct CentreOrbitHandle<'a> {
    cache: &'a SharedCentreOrbitCache,
    galaxy: GalaxyKey,
}

impl CentreOrbitCache for CentreOrbitHandle<'_> {
    /// Lends the cell's orbits the cache holds, or those [`CentreCellOrbits::new`] places, then
    /// held.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of this handle's key, since its cells would then be lent
    /// under another galaxy's key.
    fn orbits(
        &self,
        galaxy: &Galaxy,
        centre: &CentrePlacement<'_>,
        band: MassBand,
        cell: NestedCell,
    ) -> Arc<CentreCellOrbits> {
        assert_eq!(
            galaxy.seed().get(),
            self.galaxy.seed(),
            "a centre orbit cache handle lends the cells of one galaxy, and this is another's"
        );
        let key = (self.galaxy, band, cell);
        if let Some(orbits) = self.cache.cells.get(&key) {
            return orbits;
        }
        let orbits = Arc::new(CentreCellOrbits::new(galaxy, centre, band, cell));
        // A cell larger than the whole budget is handed back and still lent; one another job stored
        // meanwhile is the same cell, and either is lent right.
        let _ = self.cache.cells.insert(key, Arc::clone(&orbits));
        orbits
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::GENERATOR_VERSION;
    use hyperion_sim::coords::GalacticPosition;
    use hyperion_sim::galaxy::features::centre::{CentreMemberSource, NoCentreOrbitCache};
    use hyperion_sim::galaxy::query::{LayerSet, QuerySphere, SystemSource};
    use hyperion_sim::id::Layer;
    use hyperion_sim::time::UniverseTime;
    use hyperion_sim::units::LightYears;

    use super::*;
    use crate::convert::origins::{SEED, galaxy};

    /// A year on, the centre's hits within 0.05 ly of the black hole are the uncached source's,
    /// through a cache with room, through one that keeps nothing, and warm; the warm query places
    /// no cell again, and another galaxy's key is refused. (A century on, the full scan's 0.39 ly
    /// holds some 60,000 members, too many for the fast suite.)
    #[test]
    fn the_centre_s_hits_are_the_same_whatever_the_cache_holds() {
        let galaxy = galaxy();
        let t = UniverseTime::from_julian_years(1).expect("in the clock window");
        let sphere = QuerySphere::new(
            GalacticPosition::ORIGIN,
            LightYears::new(0.05),
            t,
            LightYears::ZERO,
        )
        .expect("a positive radius");
        let layers = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
            .into_iter()
            .fold(LayerSet::EMPTY, LayerSet::with);
        let hits_through = |orbits: &dyn CentreOrbitCache| {
            let source = CentreMemberSource::new(galaxy, orbits).expect("the centre builds");
            let mut hits = Vec::new();
            source.systems_in_sphere(galaxy, &sphere, layers, &mut hits);
            hits
        };
        let expected = hits_through(&NoCentreOrbitCache);
        assert!(expected.len() > 10, "{} hits", expected.len());
        let key = GalaxyKey::new(SEED, GENERATOR_VERSION);
        let roomy = SharedCentreOrbitCache::new(256 << 20);
        assert_eq!(hits_through(&roomy.handle(key)), expected, "cold");
        let cold = roomy.counters();
        assert!(cold.entries() > 0 && cold.hits() == 0);
        assert_eq!(hits_through(&roomy.handle(key)), expected, "warm");
        let warm = roomy.counters();
        assert_eq!(
            (warm.entries(), warm.misses()),
            (cold.entries(), cold.misses()),
            "the warm query places no cell again"
        );
        let none = SharedCentreOrbitCache::new(0);
        assert_eq!(hits_through(&none.handle(key)), expected, "no room");
        assert_eq!(none.counters().entries(), 0);
        let other = GalaxyKey::new(SEED + 1, GENERATOR_VERSION);
        let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            hits_through(&roomy.handle(other))
        }));
        assert!(refused.is_err(), "another galaxy's key");
    }
}
