//! The cross-language fixture for evolving orbits (plan 14, P14.T45.c): drifting records of the
//! RM1 fixture's golden systems and the state each gives at times inside its drift cell, which
//! the client's `orbit.ts` tests read to hold its drift formula to the server's.
//!
//! The line format is documented in the file's own header, written below; the client's parser
//! reads only lines that begin with `drift[`.

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::{self, SystemContext};
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The universe of plan 14's golden systems (P14.T32): the Milky Way fixture at this seed.
const SYSTEMS_SEED: u64 = 0x5eed_0000_0014_0032;

/// `solar_like`, `wide_binary` and `close_binary`, by plan 14's golden IDs.
const SYSTEMS: [u64; 3] = [
    0x4200_6cba_0000_0009,
    0x41ff_ecae_0000_0004,
    0x4200_2cb2_0000_0009,
];

/// The most records taken from each system, in index order.
const PER_SYSTEM: usize = 4;

const HEADER: [&str; 12] = [
    "# Drifting orbits and the state each gives at one time inside its drift cell, from",
    "# hyperion_sim's records of the RM1 fixture's golden systems (plan 14, P14.T45), for the",
    "# client's orbit.ts tests. One state per line after the label `drift[NN] = `, 21 fields",
    "# separated by single spaces:",
    "#   a e i node peri m0 period mu ref_s ref_ns a_rate e_rate n_rate t_s t_ns x y z vx vy vz",
    "# The elements are orbit.ts's and hold at the reference time (ref_s, ref_ns). a_rate: m/s.",
    "# e_rate: 1/s. n_rate: rad/s^2. With dt = (t_s - ref_s) + (t_ns - ref_ns) / 1e9: a + a_rate",
    "# dt, e + e_rate dt, the mean anomaly m0 + 2 pi f + 0.5 n_rate dt dt (f the elements' own",
    "# fraction of a period), and the mean motion 2 pi / period + n_rate dt in the velocity.",
    "# x y z: position relative to the parent, m. vx vy vz: m/s. Floats are Rust's shortest",
    "# round-trip form. Lines not starting `drift[` are comments.",
    "#",
];

fn finite(value: f64) -> String {
    assert!(value.is_finite(), "every fixture value is finite");
    format!("{value:e}")
}

#[test]
fn drifting_states_are_pinned_for_the_client() {
    let galaxy = Galaxy::from_params(Seed::new(SYSTEMS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let mut writer = GoldenWriter::new();
    writer.header(GENERATOR_VERSION.get());
    for line in HEADER {
        writer.line(line);
    }
    let mut n = 0;
    let century = UniverseTime::from_julian_years(100).expect("in the window");
    for raw in SYSTEMS {
        let id = SystemId::from_raw(raw).expect("a pinned ID is well formed");
        let ctx = SystemContext::for_system(&galaxy, id).expect("a pinned ID names a system");
        let system = planetary::generate(galaxy.seed(), &ctx);
        let snapshot = system.snapshot_at(&ctx, century);
        let drifting = snapshot
            .bodies()
            .iter()
            .filter_map(|record| record.orbit().ok().filter(|orbit| orbit.drift().is_some()))
            .take(PER_SYSTEM);
        for orbit in drifting {
            let drift = orbit.drift().expect("filtered");
            let reference = drift.reference();
            let until = orbit.valid_until().expect("a cell ends inside the window");
            let last = until
                .checked_sub(Span::new(0, 1).expect("a nanosecond"))
                .expect("on the clock");
            for t in [century, last] {
                let (position, velocity) = orbit.trajectory().relative_state_at(t);
                let elements = orbit.elements();
                let fields = [
                    elements.semi_major_axis().value(),
                    elements.eccentricity().value(),
                    elements.inclination().value(),
                    elements.ascending_node().value(),
                    elements.argument_of_periapsis().value(),
                    elements.mean_anomaly_at_epoch().value(),
                    elements.period().value(),
                    elements.gravitational_parameter().value(),
                ]
                .map(finite)
                .join(" ");
                let rates = [
                    drift.semi_major_axis_rate_m_per_s(),
                    drift.eccentricity_rate_per_s(),
                    drift.mean_motion_rate_rad_per_s2(),
                ]
                .map(finite)
                .join(" ");
                let state = position
                    .metres()
                    .into_iter()
                    .chain(velocity.metres_per_second())
                    .map(finite)
                    .collect::<Vec<_>>()
                    .join(" ");
                writer.line(&format!(
                    "drift[{n:02}] = {fields} {} {} {rates} {} {} {state}",
                    reference.seconds(),
                    reference.subsec_nanos(),
                    t.seconds(),
                    t.subsec_nanos()
                ));
                n += 1;
            }
        }
    }
    assert!(n >= 8, "{n} drifting states");
    golden!("orbit/drifting_states", writer.as_str());
}
