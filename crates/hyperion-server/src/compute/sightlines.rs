//! The cache of lines of sight the `extinction` request has marched (plan 07, P07.T10.c).
//!
//! A line of sight is a pure function of the galaxy, its two end points and the quality it is
//! marched at (plan 07, design note 14), and the same bits whichever end it is read from (design
//! note 15). So [`SharedSightlineCache`] keys each by the galaxy and the unordered pair of end
//! points, with the one quality the server marches at, the sim's [`SIGHTLINE_QUALITY`], as design
//! note 17 asks: a chart that asks again for the stars it showed a moment ago is answered without a
//! step. The sim owns that budget, which the sky's band profiles share (`decision-gg-port-d0.md`
//! §5); a budget rather than full quality, so that a line's cost is bounded whatever its length.
//!
//! Once plan 09's feature members are served (P09.T40.a's gate), a line also takes the features'
//! gas at the request's time ([`LineGas::Features`]): superbubbles as holes, clouds and embedded
//! regions as Plummer balls, which change with time as a nursery ages. Such a line is keyed by its
//! time as well, so that no line of one time is lent for another; a line through the smooth field
//! alone keeps the key it had before.

use std::fmt;
use std::sync::Arc;

use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::catalogue::FeatureCellCache;
use hyperion_sim::galaxy::features::gas_overlay::FeatureGas;
use hyperion_sim::galaxy::gas::extinction::{
    NoiseMode, Quality, SIGHTLINE_QUALITY, Sightline, sightline,
};
use hyperion_sim::galaxy::gas::modifiers::{GasModifier, GasModifierSource};
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::Years;

use super::GalaxyKey;
use crate::cache::{HeapBytes, LruCounters, SharedByteLru};

/// Lattice normals a job's [`NoiseCache`] holds: the sim's own suggestion for one line after
/// another (plan 07's Risks, "For P07.T10").
const NOISE_CACHE_ENTRIES: usize = 4_096;

/// One end of a line, as bits: its light-year cell and its offset's bits, which
/// [`GalacticPosition::new`] keeps canonical (no −0), so that equal positions have equal keys.
type EndKey = ([i32; 3], [u64; 3]);

/// What names a line in the cache: the galaxy, its two ends in canonical order, the quality, and
/// the gas it was marched through.
///
/// The noise mode is not in it because [`SightlineMarcher::line`] fixes it (`Realised`): a marcher
/// that took it as an argument would have to add it here, or lines marched under one setting would
/// be served for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SightlineKey {
    galaxy: GalaxyKey,
    ends: [EndKey; 2],
    quality: Quality,
    gas: GasKey,
}

/// The gas a cached line was marched through, as its key holds it: the smooth field alone, which
/// is the same at every time, or the features' gas at one time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GasKey {
    Smooth,
    FeaturesAt(UniverseTime),
}

impl SightlineKey {
    #[must_use]
    fn new(
        galaxy: GalaxyKey,
        a: &GalacticPosition,
        b: &GalacticPosition,
        quality: Quality,
        gas: GasKey,
    ) -> Self {
        let end = |p: &GalacticPosition| -> EndKey {
            (p.cell().to_array(), p.offset_metres().map(f64::to_bits))
        };
        let (a, b) = (end(a), end(b));
        Self {
            galaxy,
            ends: if b < a { [b, a] } else { [a, b] },
            quality,
            gas,
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
    /// this cache or marched in the seed's own gas at [`SIGHTLINE_QUALITY`] with the modifiers of
    /// `gas`, and stored.
    ///
    /// The marcher owns the job's [`NoiseCache`], so it belongs on the pool job that runs the
    /// lines, as the sim's cache is not shared between threads.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of `key`, since its lines would then be stored under
    /// another galaxy's key.
    #[must_use]
    pub fn marcher<'a>(
        &'a self,
        key: GalaxyKey,
        galaxy: &'a Galaxy,
        gas: LineGas<'a>,
    ) -> SightlineMarcher<'a> {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a sightline cache entry is of one galaxy, and this is another's"
        );
        SightlineMarcher {
            cache: self,
            key,
            galaxy,
            gas,
            modifiers: Vec::new(),
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

/// The gas a marcher's lines take beside the galaxy's smooth field (plan 09, P09.T40.a).
#[derive(Clone, Copy)]
pub enum LineGas<'a> {
    /// None: plan 07's field alone, the same at every time. The `extinction` request's until plan
    /// 09's feature members are served.
    Smooth,
    /// Plan 09's features' gas at `t` ([`FeatureGas`]), its features looked up through `cells`.
    Features {
        /// Where the features' cells are looked up.
        cells: &'a dyn FeatureCellCache,
        /// The time the features are evaluated at.
        t: UniverseTime,
    },
}

impl LineGas<'_> {
    /// What a line marched through this gas is keyed by.
    #[must_use]
    fn key(self) -> GasKey {
        match self {
            Self::Smooth => GasKey::Smooth,
            Self::Features { t, .. } => GasKey::FeaturesAt(t),
        }
    }
}

impl fmt::Debug for LineGas<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Smooth => f.write_str("Smooth"),
            Self::Features { t, .. } => f
                .debug_struct("Features")
                .field("t", t)
                .finish_non_exhaustive(),
        }
    }
}

/// Marches lines of sight through one galaxy for one pool job, through a [`SharedSightlineCache`].
#[derive(Debug)]
pub struct SightlineMarcher<'a> {
    cache: &'a SharedSightlineCache,
    key: GalaxyKey,
    galaxy: &'a Galaxy,
    gas: LineGas<'a>,
    /// The features' modifiers near the line being marched, refilled for each line.
    modifiers: Vec<GasModifier>,
    noise: NoiseCache,
}

impl SightlineMarcher<'_> {
    /// What lies between `a` and `b`: the cache's line, or the sim's [`sightline`] in
    /// [`NoiseMode::Realised`] at [`SIGHTLINE_QUALITY`] through the marcher's gas, stored. The
    /// same whichever way round the ends are given, since the features' modifiers near a segment
    /// are a function of its unordered ends.
    pub fn line(&mut self, a: &GalacticPosition, b: &GalacticPosition) -> Sightline {
        let key = SightlineKey::new(self.key, a, b, SIGHTLINE_QUALITY, self.gas.key());
        if let Some(line) = self.cache.lines.get(&key) {
            return *line;
        }
        self.modifiers.clear();
        if let LineGas::Features { cells, t } = self.gas {
            FeatureGas::new(
                self.galaxy,
                cells,
                Years::new(t.since_epoch().as_julian_years_f64()),
            )
            .modifiers_near_segment(a, b, &mut self.modifiers);
        }
        let line = sightline(
            self.galaxy.gas(),
            a,
            b,
            NoiseMode::Realised,
            SIGHTLINE_QUALITY,
            &self.modifiers,
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
    use std::cell::RefCell;

    use hyperion_sim::units::LightYears;
    use hyperion_sim::{GENERATOR_VERSION, Seed};
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::cache::ENTRY_OVERHEAD_BYTES;
    use crate::compute::SharedFeatureCellCache;

    fn at(ly: [f64; 3]) -> GalacticPosition {
        GalacticPosition::from_light_years(ly).expect("in the cube")
    }

    #[test]
    fn a_line_is_the_sims_either_way_round_and_the_second_ask_is_a_hit() {
        let galaxy = Galaxy::new(Seed::new(0x4d2));
        let key = GalaxyKey::new(0x4d2, GENERATOR_VERSION);
        let (sun, star) = (at([0.0, 26_000.0, 0.0]), at([1_500.0, 24_000.0, 200.0]));
        let cache = SharedSightlineCache::new(1 << 20);
        let mut marcher = cache.marcher(key, &galaxy, LineGas::Smooth);
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
        assert_eq!(
            none.marcher(key, &galaxy, LineGas::Smooth)
                .line(&star, &sun),
            there
        );
        assert_eq!(none.counters().entries(), 0);
    }

    #[test]
    fn a_line_is_the_same_whatever_was_marched_before_it() {
        let galaxy = Galaxy::new(Seed::new(0x4d2));
        let key = GalaxyKey::new(0x4d2, GENERATOR_VERSION);
        let (sun, star, far) = (
            at([0.0, 26_000.0, 0.0]),
            at([1_500.0, 24_000.0, 200.0]),
            at([-4_000.0, 20_000.0, -300.0]),
        );
        let inner = at([2_000.0, 9_000.0, 100.0]);
        // One pair both ways round, so that a line asked one way and then the other must agree.
        let pairs = [
            (sun, star),
            (star, sun),
            (sun, far),
            (star, far),
            (far, inner),
        ];
        // The testkit's check marches every line forwards, backwards, in a fixed permutation and
        // alone, which is what the determinism rules ask of anything behind a cache. Three caches:
        // one kept warm across the passes, its lines marched by one job's marcher and so through one
        // noise cache; one built fresh for every line; and one that holds a single line, so that
        // every other line evicts.
        let warm = SharedSightlineCache::new(1 << 20);
        let marcher = RefCell::new(warm.marcher(key, &galaxy, LineGas::Smooth));
        assert_order_independent(&pairs, |(a, b)| marcher.borrow_mut().line(a, b));
        assert_order_independent(&pairs, |(a, b)| {
            SharedSightlineCache::new(1 << 20)
                .marcher(key, &galaxy, LineGas::Smooth)
                .line(a, b)
        });
        let one_line = SharedSightlineCache::new(ENTRY_OVERHEAD_BYTES + size_of::<Sightline>());
        let marcher = RefCell::new(one_line.marcher(key, &galaxy, LineGas::Smooth));
        assert_order_independent(&pairs, |(a, b)| marcher.borrow_mut().line(a, b));
        assert!(
            one_line.counters().evictions() > 0,
            "the one-line cache evicted nothing: {:?}",
            one_line.counters()
        );
    }

    #[test]
    fn the_key_ignores_the_order_of_the_ends_and_nothing_else() {
        let key = GalaxyKey::new(1, GENERATOR_VERSION);
        let (a, b) = (at([1.5, 2.0, 3.0]), at([-4.0, 5.25, 6.0]));
        assert_eq!(
            SightlineKey::new(key, &a, &b, SIGHTLINE_QUALITY, GasKey::Smooth),
            SightlineKey::new(key, &b, &a, SIGHTLINE_QUALITY, GasKey::Smooth)
        );
        assert_ne!(
            SightlineKey::new(key, &a, &b, SIGHTLINE_QUALITY, GasKey::Smooth),
            SightlineKey::new(key, &a, &a, SIGHTLINE_QUALITY, GasKey::Smooth)
        );
        assert_ne!(
            SightlineKey::new(key, &a, &b, SIGHTLINE_QUALITY, GasKey::Smooth),
            SightlineKey::new(key, &a, &b, Quality::Full, GasKey::Smooth)
        );
        assert_ne!(
            SightlineKey::new(key, &a, &b, SIGHTLINE_QUALITY, GasKey::Smooth),
            SightlineKey::new(
                GalaxyKey::new(2, GENERATOR_VERSION),
                &a,
                &b,
                SIGHTLINE_QUALITY,
                GasKey::Smooth
            )
        );
        // A line through the features' gas is another line from the smooth field's, and one at
        // another time another again.
        let at = |years| GasKey::FeaturesAt(UniverseTime::from_julian_years(years).unwrap());
        assert_ne!(
            SightlineKey::new(key, &a, &b, SIGHTLINE_QUALITY, GasKey::Smooth),
            SightlineKey::new(key, &a, &b, SIGHTLINE_QUALITY, at(0))
        );
        assert_ne!(
            SightlineKey::new(key, &a, &b, SIGHTLINE_QUALITY, at(0)),
            SightlineKey::new(key, &a, &b, SIGHTLINE_QUALITY, at(100))
        );
        assert_eq!(
            SightlineKey::new(key, &a, &b, SIGHTLINE_QUALITY, at(100)),
            SightlineKey::new(key, &b, &a, SIGHTLINE_QUALITY, at(100))
        );
    }

    /// Through the features' gas, a line is the sim's own `sightline` with the modifiers
    /// `FeatureGas` gives at the line's time, either way round; a line through a nursery whose gas
    /// changes as it ages differs between two times, each its own entry; and the smooth field's
    /// line differs from both (P09.T40.a).
    #[test]
    fn a_line_through_the_features_gas_is_the_sim_s_at_its_own_time() {
        use hyperion_sim::galaxy::features::catalogue::{
            FeatureCatalogue, FeatureMarks, NoFeatureCache,
        };

        let galaxy = Galaxy::new(Seed::new(0x4d2));
        let key = GalaxyKey::new(0x4d2, GENERATOR_VERSION);
        let sun = at([0.0, 26_000.0, 0.0]);
        // A nursery within reach of the Sun-like point whose gas changes over the clock window.
        let years = |y: f64| Years::new(y);
        let modifiers = |t: f64, b: &GalacticPosition| {
            let mut out = Vec::new();
            FeatureGas::new(&galaxy, &NoFeatureCache, years(t))
                .modifiers_near_segment(&sun, b, &mut out);
            out
        };
        let nursery =
            FeatureCatalogue::near(&galaxy, &sun, LightYears::new(3_000.0), &NoFeatureCache)
                .filter(|feature| matches!(feature.marks(), FeatureMarks::Nursery(_)))
                .map(|feature| *feature.position())
                .find(|to| {
                    let (now, later) = (modifiers(0.0, to), modifiers(1_000.0, to));
                    !now.is_empty() && now != later
                })
                .expect("a nursery near the Sun whose gas changes within a thousand years");
        let cells = SharedFeatureCellCache::new(64 << 20);
        let handle = cells.handle(key);
        let cache = SharedSightlineCache::new(1 << 20);
        let mut lines = Vec::new();
        for t in [0_i32, 1_000] {
            let time = UniverseTime::from_julian_years(i64::from(t)).unwrap();
            let gas = LineGas::Features {
                cells: &handle,
                t: time,
            };
            let mut marcher = cache.marcher(key, &galaxy, gas);
            let there = marcher.line(&sun, &nursery);
            let expected = sightline(
                galaxy.gas(),
                &sun,
                &nursery,
                NoiseMode::Realised,
                SIGHTLINE_QUALITY,
                &modifiers(f64::from(t), &nursery),
                &mut NoiseCache::with_capacity(16),
            );
            assert_eq!(there, expected, "at {t} years");
            assert_eq!(marcher.line(&nursery, &sun), there, "either way round");
            lines.push(there);
        }
        assert_ne!(lines[0], lines[1], "the nursery's gas has changed");
        let smooth = cache
            .marcher(key, &galaxy, LineGas::Smooth)
            .line(&sun, &nursery);
        assert!(lines.iter().all(|line| *line != smooth));
        assert_eq!(
            cache.counters().entries(),
            3,
            "a line a time, and the smooth one"
        );
    }
}
