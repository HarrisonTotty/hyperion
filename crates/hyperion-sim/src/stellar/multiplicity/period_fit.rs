//! What the fit of the direct companions' period correction measures (plan 11, ruling 81.3), for
//! `hyperion-fit`'s `period_correction` task and the sim's own check of the committed table.
//!
//! The correction ([`COMMITTED`]) multiplies Moe and Di Stefano's period law so that the periods
//! of the direct companions that survive rejection follow the law itself. The fit draws, at each
//! row's primary mass ([`CORRECTION_MASSES`]), the hierarchies of a fixed sample of systems at the
//! Sun-like point of the Milky Way fixture ([`galaxy`], [`record`]) under a trial table
//! ([`draw_with`]), counts the direct companions' periods into the table's eight bins
//! ([`count_bins`]) and compares their shares with the law's ([`target_shares`]). The iteration
//! itself is the task's. Every figure here is a pure function of its arguments, so the task may
//! draw the sample in any chunks on any threads.
//!
//! # Examples
//!
//! ```
//! use hyperion_sim::stellar::multiplicity::period_fit::{
//!     COMMITTED, count_bins, direct_log_periods, draw_with, galaxy, record, shares,
//!     target_shares,
//! };
//!
//! let galaxy = galaxy();
//! let mut counts = [0; 8];
//! for index in 0..50 {
//!     let h = draw_with(&galaxy, &record(&galaxy, index, 28.0), &COMMITTED);
//!     count_bins(&direct_log_periods(&h), &mut counts);
//! }
//! // Most 28 M☉ primaries keep a direct companion; the shares sum to 1, as the law's do.
//! assert!(counts.iter().sum::<u64>() > 25);
//! assert!((shares(&counts).iter().sum::<f64>() - 1.0).abs() < 1e-12);
//! assert!((target_shares(28.0).iter().sum::<f64>() - 1.0).abs() < 1e-12);
//! ```

use super::SystemHierarchy;
use super::direct::{self, DirectPeriods};
use super::hierarchy::{self, HierarchyNode, MultiplicityContext, RedrawAttempt, StarIndex};
use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, SystemOrigin, SystemRecord};
use crate::id::{Layer, SystemId};
use crate::math;
use crate::units::{Days, SolarMasses, Years};

/// A correction table: a row for each of [`CORRECTION_MASSES`], a factor for each of the eight
/// period bins between [`BIN_EDGES`].
pub type CorrectionTable = [[f64; 8]; 4];

/// The primary masses, M☉, of the table's rows: Moe and Di Stefano's mean masses of the A/late-B,
/// mid-B, early-B and O-type intervals.
pub const CORRECTION_MASSES: [f64; 4] = direct::CORRECTION_MASSES;

/// The edges of the table's period bins, as x = log₁₀(P ÷ 1 d): 0.2–1, 1–2, …, 7–8.
pub const BIN_EDGES: [f64; 9] = [0.2, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

/// The table the generator draws with: `hyperion-fit`'s `period_correction` task's,
/// [`tables::period_correction`](crate::tables::period_correction).
pub const COMMITTED: CorrectionTable = crate::tables::period_correction::PERIOD_CORRECTION;

/// The seed of the fit's galaxy, the Milky Way fixture.
pub const GALAXY_SEED: u64 = 0x0b11_0002_0000_5eed;

/// The fit's galaxy: the Milky Way fixture under [`GALAXY_SEED`].
///
/// # Panics
///
/// Never: the fixture's gas is mostly neutral.
#[must_use]
pub fn galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(GALAXY_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// The Sun-like point: in the plane, 26,000 ly out on the +y axis, clear of the bar.
///
/// # Panics
///
/// Never: the point lies inside the root cube.
#[must_use]
pub fn position() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("inside the root cube")
}

/// The `index`-th distinct candidate ID of layer C, running through consecutive cells along +x
/// from a cell near the solar circle.
pub(crate) fn candidate(index: u32) -> SystemId {
    let first = CellKey::new(Layer::C, [0, 812, 0]).expect("a cell near the solar circle");
    let capacity = first.index_capacity();
    let along = i32::try_from(index / capacity).expect("a few cells");
    let key = CellKey::new(Layer::C, [along, 812, 0]).expect("a cell near the first");
    key.candidate_id(index % capacity)
        .expect("inside the index field")
}

/// A grid record of `id` at `at` whose primary formed with `mass` M☉, a Gyr ago, in the galaxy's
/// first component.
pub(crate) fn grid_record(
    galaxy: &Galaxy,
    id: SystemId,
    at: &GalacticPosition,
    mass: f64,
) -> SystemRecord {
    let component = galaxy
        .fields()
        .component_id(0)
        .expect("the fixture has components");
    SystemRecord::from_parts(
        id,
        *at,
        SystemOrigin::Grid(component),
        galaxy.fields().component(component).population(),
        SolarMasses::new(mass),
        Years::new(1e9),
    )
}

/// The fit sample's `index`-th system at the Sun-like point ([`position`]), whose primary formed
/// with `mass` M☉ a Gyr ago.
///
/// # Panics
///
/// If `index` runs past the few cells along +x that the sample walks, some 10⁸ systems.
#[must_use]
pub fn record(galaxy: &Galaxy, index: u32, mass: f64) -> SystemRecord {
    grid_record(galaxy, candidate(index), &position(), mass)
}

/// The hierarchy the generator would draw for `record`, free and at the first attempt, with the
/// direct companions' period law corrected by `correction` instead of [`COMMITTED`].
#[must_use]
pub fn draw_with(
    galaxy: &Galaxy,
    record: &SystemRecord,
    correction: &CorrectionTable,
) -> SystemHierarchy {
    hierarchy::draw_hierarchy_with(
        galaxy,
        record,
        MultiplicityContext::Free,
        RedrawAttempt::FIRST,
        correction,
    )
}

/// For a record whose primary the direct construction draws, what [`draw_with`] does to its
/// direct companions under `correction`: the companions it places, the periods it tries for them,
/// and the companions it drops. `None` for a primary the spine construction draws.
#[must_use]
pub fn direct_tries(
    galaxy: &Galaxy,
    record: &SystemRecord,
    correction: &CorrectionTable,
) -> Option<(usize, u64, u8)> {
    hierarchy::direct_tries(galaxy, record, correction)
}

/// The periods of the companions of `h` that orbit its primary directly, as x = log₁₀(P ÷ 1 d).
#[must_use]
pub fn direct_log_periods(h: &SystemHierarchy) -> Vec<f64> {
    h.pairs()
        .filter(|(pair, _)| {
            let HierarchyNode::Pair { inner, .. } = *h.node(*pair) else {
                unreachable!("pairs are pairs")
            };
            h.first_star(inner) == StarIndex::PRIMARY
        })
        .map(|(_, orbit)| math::log10(Days::from(orbit.period()).value()))
        .collect()
}

/// Adds each of `periods` (x = log₁₀(P ÷ 1 d)) to its bin's count in `counts`: the bin whose
/// lower edge of [`BIN_EDGES`] from 1 up it has reached, the first below 1.
pub fn count_bins(periods: &[f64], counts: &mut [u64; 8]) {
    for &x in periods {
        let bin = BIN_EDGES[1..8].iter().filter(|&&edge| x >= edge).count();
        counts[bin] += 1;
    }
}

/// Each bin's share of `counts`.
#[must_use]
pub fn shares(counts: &[u64; 8]) -> [f64; 8] {
    #[expect(
        clippy::cast_precision_loss,
        reason = "counts of a few 10⁴, exact as floats"
    )]
    let counts = counts.map(|c| c as f64);
    let total: f64 = counts.iter().sum();
    counts.map(|c| c / total)
}

/// The shares the eight bins should hold at primary mass `m` (M☉): Moe and Di Stefano's own law,
/// uncorrected and normalised over 0.2–8.
#[must_use]
pub fn target_shares(m: f64) -> [f64; 8] {
    let law = DirectPeriods::new(SolarMasses::new(m), &[[1.0; 8]; 4]);
    std::array::from_fn(|i| law.share(BIN_EDGES[i], BIN_EDGES[i + 1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fit's check of the table as committed (ruling 81.3): at each row's mass, the periods of
    /// the direct companions of the fit's own sample of 20,000 systems fill the eight bins in Moe
    /// and Di Stefano's shares, each to 0.2% of its share, the plan's figure after twelve
    /// iterations. `hyperion-fit`'s `period_correction` task is the fit and writes
    /// `tables/period_correction.rs`; this is the table's staleness guard, at a twenty-fourth of
    /// the fit's cost. P11.T1.d's refit narrowed the 7 M☉ row's interim 0.2–2% back to the
    /// others' 0.2%.
    #[test]
    #[ignore = "slow: the direct companions of 80,000 hierarchies, about 10 s"]
    fn the_period_correction_gives_its_bin_shares() {
        const SAMPLE: u32 = 20_000;
        let galaxy = galaxy();
        let mut failures = Vec::new();
        for m in CORRECTION_MASSES {
            let target = target_shares(m);
            let mut counts = [0; 8];
            for index in 0..SAMPLE {
                let h = draw_with(&galaxy, &record(&galaxy, index, m), &COMMITTED);
                count_bins(&direct_log_periods(&h), &mut counts);
            }
            let measured = shares(&counts);
            let worst = (0..8)
                .map(|b| (measured[b] / target[b] - 1.0).abs())
                .fold(0.0, f64::max);
            println!(
                "{m} M☉: {} direct companions, worst relative miss {worst:.4}; shares \
                 {measured:.4?} against {target:.4?}",
                counts.iter().sum::<u64>()
            );
            let window = 0.0..=0.002;
            if !window.contains(&worst) {
                failures.push(format!("{m} M☉ misses by {worst:.4}, outside {window:?}"));
            }
        }
        assert!(failures.is_empty(), "{failures:?}");
    }
}
