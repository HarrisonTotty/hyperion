//! Evolving orbits on the RM1 fixture (plan 14, P14.T45.a): `close_binary`'s moon `.0201`, which
//! recedes about 8 cm a year, carries a drift and a record that holds at most one drift cell.

use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::drift::DRIFT_CELL_MAX_LOG2;
use hyperion_sim::planetary::{self, BodyIndex, SystemContext};
use hyperion_sim::time::{Span, UniverseTime};

/// The universe of plan 14's golden systems (P14.T32): the Milky Way fixture at this seed.
const SYSTEMS_SEED: u64 = 0x5eed_0000_0014_0032;

/// `close_binary`, by plan 14's golden name and ID.
const CLOSE_BINARY: u64 = 0x4200_2cb2_0000_0009;

#[test]
fn the_rm1_receding_moon_drifts_and_holds_at_most_one_cell() {
    let galaxy = Galaxy::from_params(Seed::new(SYSTEMS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let id = SystemId::from_raw(CLOSE_BINARY).expect("a pinned ID is well formed");
    let ctx = SystemContext::for_system(&galaxy, id).expect("a pinned ID names a system");
    let system = planetary::generate(galaxy.seed(), &ctx);
    let moon = BodyIndex::try_from(0x0201_u16).expect("planet 2's first moon");
    let cell = Span::from_seconds(1 << DRIFT_CELL_MAX_LOG2);
    for years in [0, 100, -999] {
        let t = UniverseTime::from_julian_years(years).expect("in the window");
        let record = system.body_at(&ctx, moon, t).expect("the moon exists");
        let orbit = record.orbit().ok().expect("present, with an orbit");
        let drift = orbit.drift().expect("a receding moon drifts");
        assert!(drift.semi_major_axis_rate_m_per_s() > 0.0, "it recedes");
        assert!(drift.reference() <= t);
        let until = orbit.valid_until().expect("a cell ends inside the window");
        assert!(
            until > t && until <= t.checked_add(cell).unwrap(),
            "{until} at {t}"
        );
    }
}
