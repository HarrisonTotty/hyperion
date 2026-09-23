//! Golden values of the orbit module's functions, bit for bit.
//!
//! `orbit/states` pins bound propagation for the client. This pins the rest, so that a later
//! rewrite of any of them cannot change output without a test failing: both Kepler solvers and
//! Barker's equation, the Roche lobe, Peters's merger time across every panel count of its
//! quadrature (0 to 6 panels in ln t) and its inverse, open orbits in each of their three
//! regimes, and the inverse from a state, bound and open. Nothing generated reads these yet, so
//! they carry generator version 11 as new values (plan 11, P11.T3.a; plan 14, P14.T2).

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::coords::{SystemVector, SystemVelocity};
use hyperion_sim::orbit::{
    Eccentricity, OpenOrbit, Orbit, Orientation, elements_from_state, peters_merger_time,
    peters_separation_for, roche_lobe_radius, solve_barker, solve_kepler, solve_kepler_hyperbolic,
};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::consts::{GM_SUN, METRES_PER_AU};
use hyperion_sim::units::{GravitationalParameter, Metres, Radians, SolarMasses, Years};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

fn orientation(i: f64, node: f64, argument: f64) -> Orientation {
    Orientation::new(Radians::new(i), Radians::new(node), Radians::new(argument))
        .expect("test angles are in range")
}

fn write_vector(writer: &mut GoldenWriter, label: &str, components: [f64; 3]) {
    for (axis, value) in ["x", "y", "z"].iter().zip(components) {
        writer.f64(&format!("{label}.{axis}"), value);
    }
}

#[test]
fn orbit_functions_are_pinned() {
    let mut writer = GoldenWriter::new();
    writer.header(GENERATOR_VERSION.get());
    write_solvers(&mut writer);
    write_binaries(&mut writer);
    write_open_orbits(&mut writer);
    write_inverses(&mut writer);
    golden!("orbit/functions", writer.as_str());
}

/// Both Kepler solvers and Barker's equation.
fn write_solvers(writer: &mut GoldenWriter) {
    writer.line("solve_kepler (mean anomaly, eccentricity)");
    for (m, e) in [
        (0.5, 0.0),
        (1e-6, 0.999),
        (3.0, 0.5),
        (-2.0, 0.9),
        (1.0, 0.999_999),
        (7.5, 0.3),
    ] {
        let anomaly = solve_kepler(Radians::new(m), Eccentricity::new(e).expect("e in range"));
        writer.f64(&format!("E({m}, {e})"), anomaly.value());
    }

    writer.line("solve_kepler_hyperbolic (mean anomaly, eccentricity)");
    for (m, e) in [(0.1, 1.000_001), (5.0, 1.5), (1e6, 3.0), (-250.0, 20.0)] {
        writer.f64(&format!("H({m}, {e})"), solve_kepler_hyperbolic(m, e));
    }

    writer.line("solve_barker (mean anomaly)");
    for m in [1e-8, 4.0 / 3.0, -1e3, 1e9] {
        writer.f64(&format!("D({m})"), solve_barker(m));
    }
}

/// The Roche lobe and Peters's inspiral, across every panel count of its quadrature.
fn write_binaries(writer: &mut GoldenWriter) {
    writer.line("roche_lobe_radius (q, separation 1e9 m)");
    for q in [0.01, 1.0, 3.5, 100.0] {
        writer.f64(
            &format!("r_L({q})"),
            roche_lobe_radius(q, Metres::new(1e9)).value(),
        );
    }

    writer.line("peters_merger_time (1.35 + 0.62 Msun, a = 3e9 m), yr");
    // t₀ = e ÷ √(1 − e²): under ¼ (first panel alone), under 4 (no log panel), then 1 to 6.
    for e in [
        0.0,
        0.1,
        0.24,
        0.5,
        0.97,
        0.99,
        0.9999,
        0.999_999_99,
        1.0 - 1e-12,
        1.0 - 1e-15,
    ] {
        let t = peters_merger_time(
            SolarMasses::new(1.35),
            SolarMasses::new(0.62),
            Metres::new(3e9),
            Eccentricity::new(e).expect("e in range"),
        );
        writer.f64(&format!("T(e = {e})"), t.value());
    }

    writer.line("peters_separation_for, m");
    for (m1, m2, years) in [(0.7, 0.7, 1e3), (1.4, 1.3, 1e8)] {
        let a = peters_separation_for(
            SolarMasses::new(m1),
            SolarMasses::new(m2),
            Years::new(years),
        );
        writer.f64(&format!("a({m1}, {m2}, {years} yr)"), a.value());
    }
}

/// Open orbits in their three regimes: bound near the parabola, inside the band, hyperbolic.
fn write_open_orbits(writer: &mut GoldenWriter) {
    let sun = GravitationalParameter::new(GM_SUN);
    let pericentre_time = UniverseTime::new(1_000_000, 250_000_000).expect("nanos in range");
    writer.line("OpenOrbit::relative_state_at (q = 1 au, i 0.7, node 2.1, periapsis 4.2)");
    for e in [0.999_95, 1.0, 1.000_000_5, 1.2, 50.0] {
        let orbit = OpenOrbit::new(
            Metres::new(METRES_PER_AU),
            e,
            orientation(0.7, 2.1, 4.2),
            pericentre_time,
            sun,
        )
        .expect("a valid open orbit");
        for seconds in [-10_000_000_i64, 31_557_600] {
            let t = UniverseTime::new(seconds, 0).expect("nanos in range");
            let (position, velocity) = orbit.relative_state_at(t);
            let label = format!("e = {e}, t = {seconds} s");
            write_vector(writer, &format!("{label}: r"), position.metres());
            write_vector(writer, &format!("{label}: v"), velocity.metres_per_second());
        }
    }
}

/// The inverse from a state, bound and open.
fn write_inverses(writer: &mut GoldenWriter) {
    let sun = GravitationalParameter::new(GM_SUN);
    writer.line("elements_from_state (mu = GM_SUN, t = 123456789.5 s)");
    let t = UniverseTime::new(123_456_789, 500_000_000).expect("nanos in range");
    let position = SystemVector::new([1.2e11, -3.4e10, 5.6e9]);
    for (label, velocity) in [
        ("bound", [8.0e3, 2.9e4, -1.5e3]),
        ("open", [2.5e4, 4.5e4, 3.0e3]),
    ] {
        let orbit = elements_from_state(position, SystemVelocity::new(velocity), sun, t)
            .expect("a valid state");
        match orbit {
            Orbit::Bound(k) => {
                writer.f64(&format!("{label}: a"), k.semi_major_axis().value());
                writer.f64(&format!("{label}: e"), k.eccentricity().value());
                writer.f64(&format!("{label}: i"), k.inclination().value());
                writer.f64(&format!("{label}: node"), k.ascending_node().value());
                writer.f64(&format!("{label}: peri"), k.argument_of_periapsis().value());
                writer.f64(&format!("{label}: m0"), k.mean_anomaly_at_epoch().value());
                writer.f64(&format!("{label}: period"), k.period().value());
            }
            Orbit::Open(o) => {
                writer.f64(&format!("{label}: q"), o.pericentre().value());
                writer.f64(&format!("{label}: e"), o.eccentricity());
                writer.f64(
                    &format!("{label}: i"),
                    o.orientation().inclination().value(),
                );
                writer.f64(
                    &format!("{label}: node"),
                    o.orientation().ascending_node().value(),
                );
                writer.f64(
                    &format!("{label}: peri"),
                    o.orientation().argument_of_periapsis().value(),
                );
                writer.line(&format!(
                    "{label}: time of pericentre = {}",
                    o.time_of_pericentre()
                ));
            }
        }
    }
}

#[test]
fn the_same_inputs_give_the_same_bits() {
    let run = || {
        let orbit = OpenOrbit::new(
            Metres::new(2e11),
            1.000_000_2,
            orientation(1.0, 2.0, 3.0),
            UniverseTime::EPOCH,
            GravitationalParameter::new(GM_SUN),
        )
        .expect("a valid open orbit");
        let (r, v) = orbit.relative_state_at(UniverseTime::new(-7_777_777, 7).expect("in range"));
        let inverse = elements_from_state(
            r,
            v,
            GravitationalParameter::new(GM_SUN),
            UniverseTime::EPOCH,
        )
        .expect("a valid state");
        (r, v, inverse)
    };
    assert_eq!(run(), run());
}
