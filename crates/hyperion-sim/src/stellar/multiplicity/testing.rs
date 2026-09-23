//! Samples of system records for the hierarchy's and the positions' tests.

use hyperion_testkit::lcg::Lcg;

use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::imf::{MASS_LIMIT_HI, MASS_LIMIT_LO};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, SystemOrigin, SystemRecord};
use crate::id::{Layer, SystemId};
use crate::units::{SolarMasses, Years};

/// The seed of the galaxy every test here draws in.
const SEED: u64 = 0x0b11_0002_0000_5eed;

/// The sample size of the property tests (P11.T2 and T3.b): 10⁴ hierarchies.
pub(super) const SAMPLE: u32 = 10_000;

/// The Milky Way fixture.
pub(super) fn galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// The Sun-like point: in the plane, 26,000 ly out on the +y axis, clear of the bar.
pub(super) fn sunlike() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("inside the root cube")
}

/// A point 6,000 ly from the centre in the plane, where the Galactic tide is much stronger than at
/// the Sun-like point and every tidal radius smaller.
pub(super) fn inner_disc() -> GalacticPosition {
    GalacticPosition::from_light_years([6_000.0, 0.0, 0.0]).expect("inside the root cube")
}

/// `n` distinct candidate IDs of layer C, running through consecutive cells along +x.
fn ids(n: u32) -> impl Iterator<Item = SystemId> {
    let first = CellKey::new(Layer::C, [0, 812, 0]).expect("a cell near the solar circle");
    let capacity = first.index_capacity();
    (0..n).map(move |i| {
        let along = i32::try_from(i / capacity).expect("a few cells");
        let key = CellKey::new(Layer::C, [along, 812, 0]).expect("a cell near the first");
        key.candidate_id(i % capacity)
            .expect("inside the index field")
    })
}

/// A grid record of `id` at `at` whose primary formed with `mass` M☉, a Gyr ago.
pub(super) fn record(
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

/// `n` records at `at` whose primaries are drawn from the galaxy's mass function over the whole
/// stellar range, 0.08–150 M☉, by a test generator salted with `salt`.
pub(super) fn imf_records(
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

/// `n` records at `at` whose primaries all formed with `mass` M☉.
pub(super) fn records_of_mass(
    galaxy: &Galaxy,
    n: u32,
    at: &GalacticPosition,
    mass: f64,
) -> Vec<SystemRecord> {
    ids(n).map(|id| record(galaxy, id, at, mass)).collect()
}
