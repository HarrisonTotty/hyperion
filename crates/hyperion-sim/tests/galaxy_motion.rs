//! Goldens of the Kepler propagator about the central black hole (plan 09, P09.T28.a).
//!
//! The propagator is, with the Eddington inversion, the piece of the centre most exposed to a
//! platform difference (plan 09, "Verification"), so its states are pinned as bits and checked on
//! every CI target, x86-64, `AArch64` and wasm32. Nothing generated reads it until P09.T28.b wires
//! it into the drift hook: this golden pins the propagator alone and moves no system.

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::coords::GalacticVelocity;
use hyperion_sim::galaxy::PointLy;
use hyperion_sim::galaxy::motion::KeplerOrbit;
use hyperion_sim::units::consts::SECONDS_PER_JULIAN_YEAR;
use hyperion_sim::units::{GravitationalParameter, Seconds, SolarMasses};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// One fixture state: position relative to the black hole (ly), velocity (km/s), the mass orbited
/// (M☉), and what it is.
struct Case {
    position: [f64; 3],
    velocity_km_s: [f64; 3],
    mass: f64,
    what: &'static str,
}

/// Bound orbits from circular to e ≈ 0.9999 at 0.001–10 ly, S2 near apocentre, nearly and exactly
/// (to rounding) parabolic states, hyperbolic flybys, a retrograde orbit and one tilted out of
/// every plane.
const CASES: [Case; 12] = [
    Case {
        position: [1.0, 0.0, 0.0],
        velocity_km_s: [0.0, 245.51, 0.0],
        mass: 4.297e6,
        what: "near-circular at 1 ly",
    },
    Case {
        position: [0.030_8, 0.001, -0.000_5],
        velocity_km_s: [-20.0, 470.0, 35.0],
        mass: 4.297e6,
        what: "S2-like near apocentre",
    },
    Case {
        position: [0.001, 0.0, 0.0],
        velocity_km_s: [0.0, 30.0, 5.0],
        mass: 4.297e6,
        what: "e near 1 at 0.001 ly",
    },
    Case {
        position: [3.0, -2.0, 1.5],
        velocity_km_s: [40.0, 60.0, -25.0],
        mass: 4.8e6,
        what: "eccentric at 4 ly with enclosed stars",
    },
    Case {
        position: [9.5, 1.0, 0.2],
        velocity_km_s: [-30.0, 70.0, 10.0],
        mass: 8.6e6,
        what: "near the influence radius",
    },
    Case {
        position: [0.1, 0.0, 0.0],
        velocity_km_s: [0.0, -600.0, 0.0],
        mass: 4.297e6,
        what: "retrograde at 0.1 ly",
    },
    Case {
        position: [0.2, 0.3, -0.1],
        velocity_km_s: [-150.0, 250.0, 330.0],
        mass: 4.297e6,
        what: "tilted, bound",
    },
    Case {
        position: [0.5, 0.0, 0.0],
        velocity_km_s: [0.0, 0.0, 491.0],
        mass: 4.297e6,
        what: "nearly parabolic, bound",
    },
    Case {
        position: [0.5, 0.0, 0.0],
        velocity_km_s: [0.0, 0.0, 491.05],
        mass: 4.297e6,
        what: "nearly parabolic, unbound",
    },
    Case {
        position: [2.0, 1.0, 0.0],
        velocity_km_s: [-900.0, -300.0, 100.0],
        mass: 4.297e6,
        what: "hyperbolic, inbound",
    },
    Case {
        position: [0.05, 0.02, 0.01],
        velocity_km_s: [2_500.0, 1_000.0, -800.0],
        mass: 4.297e6,
        what: "hypervelocity, outbound",
    },
    Case {
        position: [-0.4, 0.7, 0.9],
        velocity_km_s: [60.0, -45.0, 20.0],
        mass: 5.2e6,
        what: "slow, deep plunge",
    },
];

/// Times from the epoch, Julian years: across the clock window, both signs.
const TIMES_YEARS: [f64; 6] = [-1_000.0, -16.0, -0.25, 0.25, 16.0, 1_000.0];

#[test]
fn kepler_propagator_states_are_pinned() {
    let mut writer = GoldenWriter::new();
    writer.header(GENERATOR_VERSION.get());
    writer.line("# KeplerOrbit::propagate about the central black hole (plan 09, P09.T28.a).");
    writer.line("# Per case and time: position (ly) and velocity (m/s) on the galactic axes.");
    for (n, case) in CASES.iter().enumerate() {
        let [x, y, z] = case.position;
        let orbit = KeplerOrbit::from_state(
            PointLy::new(x, y, z),
            GalacticVelocity::new(case.velocity_km_s.map(|v| v * 1e3)),
            GravitationalParameter::from_solar_masses(SolarMasses::new(case.mass)),
        )
        .expect("fixture states are valid");
        writer.line(&format!("# case[{n:02}]: {}", case.what));
        writer.f64(&format!("case[{n:02}].eccentricity"), orbit.eccentricity());
        writer.f64(
            &format!("case[{n:02}].pericentre_ly"),
            orbit.pericentre().value(),
        );
        for (k, years) in TIMES_YEARS.into_iter().enumerate() {
            let state = orbit.propagate(Seconds::new(years * SECONDS_PER_JULIAN_YEAR));
            let position = state.position();
            let velocity = state.velocity().metres_per_second();
            let label = format!("case[{n:02}].t[{k}]");
            for (axis, value) in ["x", "y", "z"]
                .into_iter()
                .zip([position.x, position.y, position.z])
            {
                writer.f64(&format!("{label}.{axis}"), value);
            }
            for (axis, value) in ["vx", "vy", "vz"].into_iter().zip(velocity) {
                writer.f64(&format!("{label}.{axis}"), value);
            }
        }
    }
    golden!("galaxy/motion/kepler", writer.as_str());
}
