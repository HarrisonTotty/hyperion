//! The cross-language fixture for the client's orbit mathematics (plan 14, P14.T39).
//!
//! The server sends orbits as elements and the `SYSTEM` display propagates them itself between
//! requests (plan 14, D18). This golden pins 32 orbits and the state each gives at one time, so
//! that the client's `orbit.ts` tests can parse it and check their propagation against the
//! server's to 10⁻⁹ relative. The line format is documented in the file's own header, written
//! below; the client's parser reads only lines that begin with `state[`.
//!
//! The values are part of the generator version like any golden's: a change to `orbit` that moves
//! them moves every binary and planet too.

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::consts::{
    GM_EARTH, GM_JUPITER, GM_SUN, METRES_PER_AU, RADIANS_PER_DEGREE,
};
use hyperion_sim::units::{GravitationalParameter, Metres, Radians};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// Saturn's mass parameter, m³ s⁻² (JPL's value, 3.793 120 7 × 10¹⁶), for a moon's orbit.
const GM_SATURN: f64 = 3.793_120_7e16;

/// One fixture orbit: semi-major axis (m), eccentricity, inclination, ascending node, argument
/// of periapsis and mean anomaly at the epoch (degrees), gravitational parameter (m³ s⁻²), and
/// the time (whole seconds, nanoseconds) of the state.
struct Case {
    a: f64,
    e: f64,
    angles_deg: [f64; 4],
    mu: f64,
    t: (i64, u32),
}

/// A [`Case`] from its fields in order.
const fn case(a: f64, e: f64, angles_deg: [f64; 4], mu: f64, t: (i64, u32)) -> Case {
    Case {
        a,
        e,
        angles_deg,
        mu,
        t,
    }
}

const AU: f64 = METRES_PER_AU;
const YEAR: i64 = 31_557_600;

/// Eccentricities from 0 to 0.999, inclinations from 0 to exactly 180° with 179.9° and 179.99°
/// among them, scales from a white-dwarf binary 32,000 km across to a binary 1,000 au across,
/// times from −8 × 10¹² s to +8 × 10¹² s and at the epoch, a nanosecond either side of a whole
/// second, and minutes from periapsis at e = 0.999.
const CASES: [Case; 32] = [
    // Circular and equatorial: the reference, at the epoch and half a year on.
    case(AU, 0.0, [0.0, 0.0, 0.0, 0.0], GM_SUN, (0, 0)),
    case(AU, 0.0, [0.0, 0.0, 0.0, 90.0], GM_SUN, (15_778_800, 0)),
    // Nearly circular, a day before the epoch.
    case(
        0.387 * AU,
        1e-6,
        [23.4, 10.0, 20.0, 30.0],
        GM_SUN,
        (-86_400, 0),
    ),
    // The Earth and Jupiter, a quarter and a whole century out.
    case(
        1.000_002_61 * AU,
        0.016_7,
        [0.000_05, 348.74, 114.21, 358.62],
        GM_SUN + GM_EARTH,
        (25 * YEAR, 500_000_000),
    ),
    case(
        5.202_9 * AU,
        0.048_4,
        [1.305, 100.47, 273.87, 20.02],
        GM_SUN + GM_JUPITER,
        (-100 * YEAR, 0),
    ),
    // The Moon about the Earth, a nanosecond before a whole second; Io about Jupiter.
    case(
        3.844e8,
        0.054_9,
        [5.145, 125.08, 318.15, 135.27],
        1.012_3 * GM_EARTH,
        (2_360_591, 999_999_999),
    ),
    case(
        4.217e8,
        0.004_1,
        [0.05, 43.98, 84.13, 342.02],
        GM_JUPITER,
        (-123_456, 250_000_000),
    ),
    // A hot Jupiter a thousand years out: 365,000 periods, which needs the exact reduction.
    case(
        0.02 * AU,
        0.1,
        [87.0, 200.0, 45.0, 10.0],
        1.1 * GM_SUN,
        (1_000 * YEAR, 0),
    ),
    // Stellar binaries: close, wide, and one of 50 au nearly retrograde.
    case(
        0.1 * AU,
        0.2,
        [45.0, 300.0, 250.0, 180.0],
        2.0 * GM_SUN,
        (-1_000 * YEAR, 1),
    ),
    case(
        1_000.0 * AU,
        0.3,
        [120.0, 15.0, 95.0, 270.0],
        1.5 * GM_SUN,
        (8_000_000_000_000, 0),
    ),
    case(
        50.0 * AU,
        0.4,
        [179.9, 33.0, 66.0, 99.0],
        GM_SUN,
        (-5_000_000_000, 750_000_000),
    ),
    // Exactly retrograde, polar, and a second either side of the epoch.
    case(2.0 * AU, 0.5, [180.0, 0.0, 30.0, 60.0], GM_SUN, (1, 0)),
    case(10.0 * AU, 0.6, [90.0, 90.0, 90.0, 90.0], GM_SUN, (-1, 0)),
    case(
        0.5 * AU,
        0.7,
        [30.0, 60.0, 300.0, 5.0],
        GM_SUN,
        (987_654_321, 123_456_789),
    ),
    case(
        3.0 * AU,
        0.8,
        [150.0, 250.0, 10.0, 355.0],
        GM_SUN,
        (-987_654_321, 876_543_211),
    ),
    case(17.8 * AU, 0.9, [70.0, 110.0, 200.0, 0.0], GM_SUN, (0, 1)),
    // Eccentric about a white dwarf, an hour on.
    case(
        1.0e7,
        0.95,
        [10.0, 20.0, 30.0, 40.0],
        0.6 * GM_SUN,
        (3_600, 0),
    ),
    // Halley's comet, a period before the epoch.
    case(
        17.83 * AU,
        0.967,
        [162.26, 58.42, 111.33, 38.38],
        GM_SUN,
        (-2_398_867_200, 0),
    ),
    // Close to periapsis at high eccentricity: an hour before, a day after, ten minutes before.
    case(AU, 0.99, [5.0, 80.0, 160.0, 0.0], GM_SUN, (-3_600, 0)),
    case(
        100.0 * AU,
        0.995,
        [60.0, 140.0, 220.0, 0.0],
        GM_SUN,
        (86_400, 0),
    ),
    case(30.0 * AU, 0.999, [0.0, 0.0, 45.0, 0.0], GM_SUN, (-600, 0)),
    case(
        3.0 * AU,
        0.999,
        [179.99, 270.0, 135.0, 180.0],
        GM_SUN,
        (YEAR, 0),
    ),
    // Two white dwarfs a thousand years before merging (plan 11), and a thousand years later.
    case(
        3.215e7,
        0.0,
        [70.0, 35.0, 0.0, 123.0],
        1.4 * GM_SUN,
        (1_000 * YEAR, 123),
    ),
    case(
        1.0e8,
        0.1,
        [25.0, 5.0, 185.0, 300.0],
        1.2 * GM_SUN,
        (-1_000 * YEAR, 999_999_999),
    ),
    case(
        1.0e9,
        0.25,
        [55.0, 155.0, 255.0, 355.0],
        2.5 * GM_SUN,
        (123_456_789_012, 0),
    ),
    // Detached objects and a brown-dwarf companion.
    case(
        500.0 * AU,
        0.75,
        [11.9, 144.0, 311.0, 1.0],
        GM_SUN,
        (-8_000_000_000_000, 0),
    ),
    case(
        506.0 * AU,
        0.85,
        [11.93, 144.31, 311.46, 358.0],
        GM_SUN,
        (4_000_000_000, 0),
    ),
    case(
        20.0 * AU,
        0.65,
        [100.0, 200.0, 300.0, 50.0],
        1.06 * GM_SUN,
        (777_777_777, 777_777_777),
    ),
    // A planet of an M dwarf, a moon of Saturn, a Venus.
    case(
        0.05 * AU,
        0.45,
        [135.0, 225.0, 315.0, 45.0],
        0.12 * GM_SUN,
        (-1_000_000_000, 0),
    ),
    case(
        1.2e9,
        0.35,
        [27.0, 169.0, 186.0, 11.0],
        GM_SATURN,
        (55_555, 0),
    ),
    case(
        0.723 * AU,
        0.15,
        [3.39, 76.68, 54.88, 50.12],
        GM_SUN,
        (-2 * YEAR, 0),
    ),
    // At periapsis at e = 0.999, at the epoch.
    case(AU, 0.999, [90.0, 0.0, 0.0, 0.0], GM_SUN, (0, 0)),
];

const HEADER: [&str; 18] = [
    "# Kepler orbits and the state each gives at one time, from hyperion_sim::orbit (plan 14,",
    "# P14.T2.a), for the client's orbit.ts tests (P14.T39). One orbit per line after the label",
    "# `state[NN] = `, 16 fields separated by single spaces:",
    "#   a e i node peri m0 period mu t_s t_ns x y z vx vy vz",
    "# a: semi-major axis, m. e: eccentricity, 0 <= e < 1. i: inclination, rad, 0..pi, from +z.",
    "# node: longitude of the ascending node, rad, 0..2pi, from +x towards +y. peri: argument of",
    "# periapsis, rad, 0..2pi, from the node in the direction of motion. m0: mean anomaly at the",
    "# epoch (t = 0), rad, 0..2pi. period: s. mu: gravitational parameter, m^3 s^-2, which is",
    "# 4 pi^2 a^3 / period^2 to rounding. t_s, t_ns: the time since the epoch, whole seconds",
    "# (floored, signed) and nanoseconds (0..999999999). x y z: position of the secondary relative",
    "# to the primary, m. vx vy vz: its velocity, m/s. Axes are the system frame's, the galactic.",
    "# The mean anomaly at t is m0 + 2 pi f, f the fraction of a period elapsed. The server reduces",
    "# t_s modulo the period exactly (the truncated remainder: C's fmod, JavaScript's %) before",
    "# adding t_ns / 1e9, and some lines need the same to agree to 1e-9: a hot Jupiter and a",
    "# white-dwarf binary are given a thousand years out. Floats are Rust's shortest round-trip",
    "# form (`{:e}`), so JavaScript's Number() recovers each double exactly; every value is",
    "# finite. Lines not starting `state[` are comments.",
    "#",
];

/// `value` in Rust's shortest round-trip exponential form, which is a one-to-one map of finite
/// doubles to text: the reason this golden may pin floats by their decimal form alone. A NaN or
/// an infinity would not survive `Number()`, so none may be written.
fn finite(value: f64, line: usize) -> String {
    assert!(
        value.is_finite(),
        "state[{line:02}] has a value that is not finite"
    );
    format!("{value:e}")
}

#[test]
fn orbit_states_are_pinned_for_the_client() {
    let mut writer = GoldenWriter::new();
    writer.header(GENERATOR_VERSION.get());
    for line in HEADER {
        writer.line(line);
    }
    for (n, case) in CASES.iter().enumerate() {
        let [inclination, node, periapsis, mean_anomaly] =
            case.angles_deg.map(|deg| deg * RADIANS_PER_DEGREE);
        let orbit = KeplerElements::from_semi_major_axis(
            Metres::new(case.a),
            GravitationalParameter::new(case.mu),
            Eccentricity::new(case.e).expect("fixture eccentricities are in [0, 1)"),
            Orientation::new(
                Radians::new(inclination),
                Radians::new(node),
                Radians::new(periapsis),
            )
            .expect("fixture angles are in range"),
            Radians::new(mean_anomaly),
        )
        .expect("fixture orbits are valid");
        let time = UniverseTime::new(case.t.0, case.t.1).expect("fixture nanoseconds are in range");
        let (position, velocity) = orbit.relative_state_at(time);

        // Each state is on its orbit: the energy matches the semi-major axis.
        let mu = orbit.gravitational_parameter().value();
        let energy = 0.5 * velocity.dot(&velocity) - mu / position.length().value();
        assert!(
            (energy * 2.0 * case.a / mu + 1.0).abs() < 1e-12,
            "state[{n:02}] is off its orbit"
        );

        let elements = [
            orbit.semi_major_axis().value(),
            orbit.eccentricity().value(),
            orbit.inclination().value(),
            orbit.ascending_node().value(),
            orbit.argument_of_periapsis().value(),
            orbit.mean_anomaly_at_epoch().value(),
            orbit.period().value(),
            mu,
        ]
        .map(|value| finite(value, n))
        .join(" ");
        let state = position
            .metres()
            .into_iter()
            .chain(velocity.metres_per_second())
            .map(|value| finite(value, n))
            .collect::<Vec<_>>()
            .join(" ");
        writer.line(&format!(
            "state[{n:02}] = {elements} {} {} {state}",
            time.seconds(),
            time.subsec_nanos()
        ));
    }
    golden!("orbit/states", writer.as_str());
}

#[test]
fn the_fixture_spans_what_the_client_must_handle() {
    let eccentricities: Vec<f64> = CASES.iter().map(|c| c.e).collect();
    assert!(eccentricities.iter().any(|&e| e.abs() < 1e-300));
    assert!(eccentricities.iter().any(|&e| (e - 0.999).abs() < 1e-12));
    let inclinations: Vec<f64> = CASES.iter().map(|c| c.angles_deg[0]).collect();
    assert!(inclinations.iter().any(|&i| i.abs() < 1e-300));
    assert!(inclinations.iter().any(|&i| (i - 180.0).abs() < 1e-12));
    assert!(inclinations.iter().any(|&i| i > 179.0 && i < 180.0));
    assert!(CASES.iter().any(|c| c.t.0 < 0));
    assert!(CASES.iter().any(|c| c.t.0 > 0));
    assert!(CASES.iter().any(|c| c.t == (0, 0)));
}
