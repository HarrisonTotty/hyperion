//! Samples of system records for the hierarchy's and the positions' tests.

use hyperion_testkit::lcg::Lcg;

use super::period_fit;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::imf::{MASS_LIMIT_HI, MASS_LIMIT_LO};
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use crate::id::{Layer, SystemId};

/// The sample size of the property tests (P11.T2 and T3.b): 10⁴ hierarchies.
pub(crate) const SAMPLE: u32 = 10_000;

/// The Milky Way fixture, the period correction's fit's galaxy.
pub(crate) fn galaxy() -> Galaxy {
    period_fit::galaxy()
}

/// The Sun-like point: in the plane, 26,000 ly out on the +y axis, clear of the bar.
pub(crate) fn sunlike() -> GalacticPosition {
    period_fit::position()
}

/// A point 6,000 ly from the centre in the plane, where the Galactic tide is much stronger than at
/// the Sun-like point and every tidal radius smaller.
pub(super) fn inner_disc() -> GalacticPosition {
    GalacticPosition::from_light_years([6_000.0, 0.0, 0.0]).expect("inside the root cube")
}

/// `n` distinct candidate IDs of layer C, running through consecutive cells along +x.
fn ids(n: u32) -> impl Iterator<Item = SystemId> {
    (0..n).map(period_fit::candidate)
}

/// A grid record of `id` at `at` whose primary formed with `mass` M☉, a Gyr ago.
pub(super) fn record(
    galaxy: &Galaxy,
    id: SystemId,
    at: &GalacticPosition,
    mass: f64,
) -> SystemRecord {
    period_fit::grid_record(galaxy, id, at, mass)
}

/// `n` records at `at` whose primaries are drawn from the galaxy's mass function over the whole
/// stellar range, 0.08–150 M☉, by a test generator salted with `salt`.
pub(crate) fn imf_records(
    galaxy: &Galaxy,
    n: u32,
    at: &GalacticPosition,
    salt: u64,
) -> Vec<SystemRecord> {
    let mut lcg = Lcg::new(salt);
    let imf = galaxy.mass_function();
    ids(n)
        .map(|id| {
            let mass = imf.quantile_in(MASS_LIMIT_LO, MASS_LIMIT_HI, lcg.next_f64());
            record(galaxy, id, at, mass)
        })
        .collect()
}

/// `n` records at `at` whose primaries are log-uniform on `lo`–`hi` M☉: every mass range
/// equally, for tests of the rare massive primaries.
pub(super) fn log_uniform_records(
    galaxy: &Galaxy,
    n: u32,
    at: &GalacticPosition,
    (lo, hi): (f64, f64),
    salt: u64,
) -> Vec<SystemRecord> {
    let mut lcg = Lcg::new(salt);
    ids(n)
        .map(|id| {
            let mass = lo * crate::math::powf(hi / lo, lcg.next_f64());
            record(galaxy, id, at, mass)
        })
        .collect()
}

/// The first `n` records of `layer` in the cells nearest `at`: the cell holding it, then each
/// shell of cells about that cell in turn, each cell's records in order, a neighbourhood's systems
/// as placement makes them (P11.T16's probe's sample). Fewer if 40 shells hold fewer.
pub(crate) fn records_near(
    galaxy: &Galaxy,
    layer: Layer,
    at: &GalacticPosition,
    n: usize,
) -> Vec<SystemRecord> {
    let centre = CellKey::containing(layer, at)
        .expect("a point inside the root cube")
        .gen_cell()
        .to_array();
    let mut records = Vec::with_capacity(n);
    let mut cell = Vec::new();
    for r in 0_i32..40 {
        for i in -r..=r {
            for j in -r..=r {
                for k in -r..=r {
                    if i.abs().max(j.abs()).max(k.abs()) != r {
                        continue;
                    }
                    let Ok(key) =
                        CellKey::new(layer, [centre[0] + i, centre[1] + j, centre[2] + k])
                    else {
                        continue;
                    };
                    generate_cell(galaxy, key, &mut cell);
                    records.extend(cell.iter().take(n - records.len()));
                    if records.len() == n {
                        return records;
                    }
                }
            }
        }
    }
    records
}

/// `n` records at `at` whose primaries all formed with `mass` M☉.
pub(super) fn records_of_mass(
    galaxy: &Galaxy,
    n: u32,
    at: &GalacticPosition,
    mass: f64,
) -> Vec<SystemRecord> {
    ids(n).map(|id| record(galaxy, id, at, mass)).collect()
}
