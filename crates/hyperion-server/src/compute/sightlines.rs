//! The cache of lines of sight the `extinction` request has marched (plan 07, P07.T10.c).
//!
//! A line of sight is a pure function of the galaxy, its two end points and the quality it is
//! marched at (plan 07, design note 14), and the same bits whichever end it is read from (design
//! note 15). So [`SharedSightlineCache`] keys each by the galaxy and the unordered pair of end
//! points, with the one quality the server marches at, [`SIGHTLINE_QUALITY`], as design note 17
//! asks: a chart that asks again for the stars it showed a moment ago is answered without a step.

use std::num::NonZeroU32;
use std::sync::Arc;

use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::extinction::{NoiseMode, Quality, Sightline, sightline};
use hyperion_sim::galaxy::gas::noise::NoiseCache;

use super::GalaxyKey;
use crate::cache::{HeapBytes, LruCounters, SharedByteLru};

/// The most steps one line of the `extinction` request is marched in (plan 07, P07.T10.c).
///
/// A budget rather than full quality, so that a line's cost is bounded whatever its length: 64
/// lines of 256 steps are a few milliseconds of one worker (plan 07's Risks, "Cost on long-range
/// charts").
pub const SIGHTLINE_QUALITY: Quality = Quality::Budget(NonZeroU32::new(256).expect("256 is not 0"));

/// Lattice normals a job's [`NoiseCache`] holds: the sim's own suggestion for one line after
/// another (plan 07's Risks, "For P07.T10").
const NOISE_CACHE_ENTRIES: usize = 4_096;

/// One end of a line, as bits: its light-year cell and its offset's bits, which
/// [`GalacticPosition::new`] keeps canonical (no −0), so that equal positions have equal keys.
type EndKey = ([i32; 3], [u64; 3]);

/// What names a line in the cache: the galaxy, its two ends in canonical order, and the quality.
///
/// The noise mode and the modifiers are not in it because [`SightlineMarcher::line`] fixes both
/// (`Realised`, none): a marcher that took either as an argument would have to add it here, or
/// lines marched under one setting would be served for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SightlineKey {
    galaxy: GalaxyKey,
    ends: [EndKey; 2],
    quality: Quality,
}

impl SightlineKey {
    #[must_use]
    fn new(
        galaxy: GalaxyKey,
        a: &GalacticPosition,
        b: &GalacticPosition,
        quality: Quality,
    ) -> Self {
        let end = |p: &GalacticPosition| -> EndKey {
            (p.cell().to_array(), p.offset_metres().map(f64::to_bits))
        };
        let (a, b) = (end(a), end(b));
        Self {
            galaxy,
            ends: if b < a { [b, a] } else { [a, b] },
            quality,
        }
    }
}

impl HeapBytes for Sightline {
    /// Nothing: a line of sight is four numbers.
    fn heap_bytes(&self) -> usize {
        0
    }
}

/// Every line of sight the server has marched, over every galaxy it holds, in one byte budget.
#[derive(Debug)]
pub struct SharedSightlineCache {
    lines: SharedByteLru<SightlineKey, Sightline>,
}

impl SharedSightlineCache {
    /// An empty cache that holds at most `budget_bytes` of charged lines. Zero caches nothing.
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            lines: SharedByteLru::new(budget_bytes),
        }
    }

    /// A marcher for one job: the lines between points of `galaxy`, the galaxy `key` names, from
    /// this cache or marched in the seed's own gas at [`SIGHTLINE_QUALITY`] with no modifiers, and
    /// stored.
    ///
    /// The marcher owns the job's [`NoiseCache`], so it belongs on the pool job that runs the
    /// lines, as the sim's cache is not shared between threads.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of `key`, since its lines would then be stored under
    /// another galaxy's key.
    #[must_use]
    pub fn marcher<'a>(&'a self, key: GalaxyKey, galaxy: &'a Galaxy) -> SightlineMarcher<'a> {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a sightline cache entry is of one galaxy, and this is another's"
        );
        SightlineMarcher {
            cache: self,
            key,
            galaxy,
            noise: NoiseCache::with_capacity(NOISE_CACHE_ENTRIES),
        }
    }

    /// The lines held, the bytes they are charged, and the hits, misses, evictions and refusals
    /// so far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.lines.counters()
    }
}

/// Marches lines of sight through one galaxy for one pool job, through a [`SharedSightlineCache`].
#[derive(Debug)]
pub struct SightlineMarcher<'a> {
    cache: &'a SharedSightlineCache,
    key: GalaxyKey,
    galaxy: &'a Galaxy,
    noise: NoiseCache,
}

impl SightlineMarcher<'_> {
    /// What lies between `a` and `b`: the cache's line, or the sim's [`sightline`] in
    /// [`NoiseMode::Realised`] at [`SIGHTLINE_QUALITY`], stored. The same whichever way round the
    /// ends are given.
    pub fn line(&mut self, a: &GalacticPosition, b: &GalacticPosition) -> Sightline {
        let key = SightlineKey::new(self.key, a, b, SIGHTLINE_QUALITY);
        if let Some(line) = self.cache.lines.get(&key) {
            return *line;
        }
        let line = sightline(
            self.galaxy.gas(),
            a,
            b,
            NoiseMode::Realised,
            SIGHTLINE_QUALITY,
            &[],
            &mut self.noise,
        );
        // A line is far smaller than any budget but zero, which stores nothing; either way the
        // caller has its line.
        drop(self.cache.lines.insert(key, Arc::new(line)));
        line
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::{GENERATOR_VERSION, Seed};

    use super::*;

    fn at(ly: [f64; 3]) -> GalacticPosition {
        GalacticPosition::from_light_years(ly).expect("in the cube")
    }

    #[test]
    fn a_line_is_the_sims_either_way_round_and_the_second_ask_is_a_hit() {
        let galaxy = Galaxy::new(Seed::new(0x4d2));
        let key = GalaxyKey::new(0x4d2, GENERATOR_VERSION);
        let (sun, star) = (at([0.0, 26_000.0, 0.0]), at([1_500.0, 24_000.0, 200.0]));
        let cache = SharedSightlineCache::new(1 << 20);
        let mut marcher = cache.marcher(key, &galaxy);
        let there = marcher.line(&sun, &star);
        let expected = sightline(
            galaxy.gas(),
            &sun,
            &star,
            NoiseMode::Realised,
            SIGHTLINE_QUALITY,
            &[],
            &mut NoiseCache::with_capacity(16),
        );
        assert_eq!(there, expected);
        assert!(there.a_v().value() > 0.0);
        let back = marcher.line(&star, &sun);
        assert_eq!(back, there);
        let counters = cache.counters();
        assert_eq!((counters.entries(), counters.hits()), (1, 1));
        // Nothing cached is the same answer.
        let none = SharedSightlineCache::new(0);
        assert_eq!(none.marcher(key, &galaxy).line(&star, &sun), there);
        assert_eq!(none.counters().entries(), 0);
    }

    #[test]
    fn the_key_ignores_the_order_of_the_ends_and_nothing_else() {
        let key = GalaxyKey::new(1, GENERATOR_VERSION);
        let (a, b) = (at([1.5, 2.0, 3.0]), at([-4.0, 5.25, 6.0]));
        assert_eq!(
            SightlineKey::new(key, &a, &b, SIGHTLINE_QUALITY),
            SightlineKey::new(key, &b, &a, SIGHTLINE_QUALITY)
        );
        assert_ne!(
            SightlineKey::new(key, &a, &b, SIGHTLINE_QUALITY),
            SightlineKey::new(key, &a, &a, SIGHTLINE_QUALITY)
        );
        assert_ne!(
            SightlineKey::new(key, &a, &b, SIGHTLINE_QUALITY),
            SightlineKey::new(key, &a, &b, Quality::Full)
        );
        assert_ne!(
            SightlineKey::new(key, &a, &b, SIGHTLINE_QUALITY),
            SightlineKey::new(
                GalaxyKey::new(2, GENERATOR_VERSION),
                &a,
                &b,
                SIGHTLINE_QUALITY
            )
        );
    }
}
