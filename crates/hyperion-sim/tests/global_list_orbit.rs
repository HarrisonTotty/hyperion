//! The global list's orbit integrator (plan 10, P10.T2): its force against the mass model, the
//! leapfrog's conservation laws and reversibility, and a golden file of an orbit's bits.
//!
//! The (R, z) grid costs seconds to build (tens on a loaded machine), so every test here shares one
//! set of tables, the Milky Way fixture's, and only the golden file, the determinism check and the
//! step checks run outside `just test-slow`. Steps follow ruling 146 of 2026-09-22: 1 ⁄ 64 of the
//! pericentre crossing time, at most 2 Myr ([`FixedStep`]).

use std::sync::OnceLock;

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::coords::{GalacticDisplacement, GalacticVelocity};
use hyperion_sim::galaxy::global_list::orbit::{
    BuildLeapfrogError, FixedStep, Leapfrog, OrbitState,
};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
use hyperion_sim::math;
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
use hyperion_sim::units::{Gigayears, LightYears, Megayears, Metres, MetresPerSecond, Seconds};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// A gradient in (km/s)² per light-year in metres per second squared.
const FORCE_IN_SI: f64 = 1e6 / METRES_PER_LIGHT_YEAR;

struct Fixture {
    model: MassModel,
    tables: PotentialTables,
}

fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let model = MassModel::new(&GalaxyParams::milky_way_like());
        let tables = PotentialTables::full(&model);
        Fixture { model, tables }
    })
}

fn leapfrog(step: Seconds) -> Leapfrog<'static> {
    Leapfrog::new(&fixture().tables, step).expect("the full tables and a finite step")
}

/// Light-years to metres.
fn m(ly: f64) -> f64 {
    ly * METRES_PER_LIGHT_YEAR
}

/// A pinned start: position in ly about the centre and velocity in km/s, how long its probe
/// integrates to pass several turning points, and its spec as a stream's would give it.
#[derive(Clone, Copy)]
struct Start {
    name: &'static str,
    position_ly: [f64; 3],
    velocity_km_s: [f64; 3],
    /// The probe's span, Gyr ([`probe`]).
    probe_gyr: f64,
    /// The pericentre, ly, and the speed there, km/s: the spec the step is set from, as measured
    /// by the probe on the Milky Way fixture at v15 (checked to 1 % in the energy test).
    pericentre_ly: f64,
    pericentre_speed_km_s: f64,
    /// The largest relative energy drift over ten radial periods at the rule's step, pinned at
    /// about 1.5 times the figure measured at v15 (in the comment); every pin is under
    /// [`ENERGY_DRIFT`].
    drift: f64,
}

impl Start {
    /// The rule's fixed step over `span` from the pinned spec.
    fn fixed_step(&self, span: Seconds) -> FixedStep {
        FixedStep::new(
            Metres::new(m(self.pericentre_ly)),
            MetresPerSecond::new(self.pericentre_speed_km_s * 1e3),
            span,
        )
        .expect("a positive crossing time and a finite span")
    }
}

/// The pinned starts: an inner inclined prograde orbit with Pal 5's pericentre (23,300–38,400 ly,
/// e about 0.24, where Pal 5's own orbit reaches 16–19 kpc; Vasiliev and Baumgardt 2021), a
/// retrograde one near 20 kpc like GD-1's (e about 0.08), an eccentric one with radii near
/// Sagittarius's (41,000–203,000 ly, e about 0.66), inclined at 62° where Sagittarius's plane is at
/// about 77° (Law and Majewski 2010), a more eccentric one (e about 0.8, 30,000–264,000 ly) whose
/// apocentre lies just beyond the tables' grid at 2¹⁸ ly, and a wide one (e about 0.9, 53,000 ly to
/// 10⁶ ly), started at apocentre at the orphan draws' limit, inclined at 30° (ruling 146.2 of
/// 2026-09-22). The golden file pins the inner, retrograde and outer ones.
const STARTS: [Start; 5] = [
    Start {
        name: "inner",
        position_ly: [25_000.0, 0.0, 20_000.0],
        velocity_km_s: [30.0, 180.0, 60.0],
        probe_gyr: 4.0,
        pericentre_ly: 23_340.0,
        pericentre_speed_km_s: 249.1,
        drift: 4.5e-5, // 3.0e-5
    },
    Start {
        name: "retrograde",
        position_ly: [-50_000.0, 30_000.0, 25_000.0],
        velocity_km_s: [60.0, 150.0, -80.0],
        probe_gyr: 4.0,
        pericentre_ly: 54_780.0,
        pericentre_speed_km_s: 207.5,
        drift: 1e-5, // 5.8e-6
    },
    Start {
        name: "sagittarius",
        position_ly: [98_000.0, 0.0, 170_000.0],
        velocity_km_s: [0.0, 68.4, 51.3],
        probe_gyr: 4.0,
        pericentre_ly: 41_210.0,
        pericentre_speed_km_s: 344.4,
        drift: 4.5e-5, // 3.0e-5
    },
    Start {
        name: "outer",
        position_ly: [15_000.0, 5_000.0, 150_000.0],
        velocity_km_s: [-60.0, 20.0, 190.0],
        probe_gyr: 4.0,
        pericentre_ly: 30_070.0,
        pericentre_speed_km_s: 394.7,
        drift: 9e-5, // 5.3e-5
    },
    Start {
        name: "wide",
        position_ly: [866_025.4, 0.0, 500_000.0],
        velocity_km_s: [0.0, 23.0, 0.0],
        probe_gyr: 40.0,
        pericentre_ly: 53_240.0,
        pericentre_speed_km_s: 430.8,
        drift: 4e-5, // 2.3e-5
    },
];

/// The bound on the energy drift over ten radial periods at the step rule, relative (ruling 146.2
/// of 2026-09-22): the plan's 10⁻⁴ read as "the pericentre passage is resolved", with room for e
/// up to 0.9. The pinned orbits hold 6 × 10⁻⁵ at worst ([`Start::drift`]).
const ENERGY_DRIFT: f64 = 2e-4;

fn start(leapfrog: &Leapfrog<'_>, position_ly: [f64; 3], velocity_km_s: [f64; 3]) -> OrbitState {
    leapfrog.state(
        GalacticDisplacement::new(position_ly.map(m)),
        GalacticVelocity::new(velocity_km_s.map(|v| v * 1e3)),
    )
}

/// What a probe with a short step measures of a start over [`Start::probe_gyr`]: its radial
/// period, its pericentre and the speed at the step nearest the pericentre.
struct Probe {
    radial_period: Seconds,
    pericentre: Metres,
    pericentre_speed: MetresPerSecond,
}

/// Probes `s` with a step of 0.25 Myr (several turning points: 16,000 steps for the orbits near
/// the disc, 160,000 for the wide one).
fn probe(s: &Start) -> Probe {
    let probe = leapfrog(Seconds::from(Megayears::new(0.25)));
    let span = Seconds::from(Gigayears::new(s.probe_gyr));
    let steps = (span.value() / probe.time_step().value()).round();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "16,000 or 160,000 steps"
    )]
    let steps = steps as u32;
    let mut state = start(&probe, s.position_ly, s.velocity_km_s);
    let summary = probe.integrate(&mut state, steps);
    assert!(summary.turning_points() >= 4, "{}: {summary:?}", s.name);
    let mut state = start(&probe, s.position_ly, s.velocity_km_s);
    let (mut nearest, mut speed) = (f64::INFINITY, 0.0);
    for _ in 0..steps {
        probe.step(&mut state);
        let r = state.radius().value();
        if r < nearest {
            nearest = r;
            speed = length(state.velocity().metres_per_second());
        }
    }
    Probe {
        radial_period: summary.radial_period().expect("several turning points"),
        pericentre: summary.pericentre().expect("a pericentre"),
        pericentre_speed: MetresPerSecond::new(speed),
    }
}

/// The largest relative energy drift of `s` over `steps` steps of `leapfrog`, and the largest in
/// each run of `per_window` steps.
fn energy_drift(
    leapfrog: &Leapfrog<'_>,
    s: &Start,
    steps: u32,
    per_window: u32,
) -> (f64, Vec<f64>) {
    let mut state = start(leapfrog, s.position_ly, s.velocity_km_s);
    let energy = leapfrog.energy_j_kg(&state);
    let mut windows = Vec::new();
    let mut worst = 0.0_f64;
    for i in 0..steps {
        leapfrog.step(&mut state);
        let drift = (leapfrog.energy_j_kg(&state) / energy - 1.0).abs();
        worst = worst.max(drift);
        let window = usize::try_from(i / per_window).expect("a u32 fits a usize");
        if window == windows.len() {
            windows.push(0.0_f64);
        }
        windows[window] = windows[window].max(drift);
    }
    (worst, windows)
}

/// The model's acceleration `−∇Φ` at a point, m/s² along x, y and z.
fn model_acceleration(position_ly: [f64; 3]) -> [f64; 3] {
    let [x, y, z] = position_ly;
    let r_cyl = math::hypot(x, y);
    let force = fixture()
        .model
        .force(LightYears::new(r_cyl), LightYears::new(z));
    let per_ly = -force.radial * FORCE_IN_SI / r_cyl;
    [per_ly * x, per_ly * y, -force.vertical * FORCE_IN_SI]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// A point at spherical radius `r` (ly), galactic latitude `latitude` (degrees) and longitude
/// `longitude` (degrees).
fn point(r: f64, latitude: f64, longitude: f64) -> [f64; 3] {
    let (b, l) = (latitude.to_radians(), longitude.to_radians());
    let (sin_b, cos_b) = math::sin_cos(b);
    let (sin_l, cos_l) = math::sin_cos(l);
    [r * cos_b * cos_l, r * cos_b * sin_l, r * sin_b]
}

/// 1–300 kpc in light-years, `n` radii spaced evenly in the logarithm.
fn radii(n: u32) -> impl Iterator<Item = f64> {
    let (inner, outer) = (3_261.56, 978_469.0);
    (0..n)
        .map(move |i| inner * math::exp(f64::from(i) / f64::from(n - 1) * math::ln(outer / inner)))
}

// P10.T2.a: forces from the tables.

/// The force the leapfrog steps with is the mass model's direct sum to 10⁻³ over 1–300 kpc, in
/// the disc's plane, above and below it, and beyond the tables' grid at 2¹⁸ ly (80 kpc), where it
/// is the NFW halo's closed form plus the Gaussian components as a point mass. This stands for the
/// plan's Plummer and NFW profiles "loaded into a table": the tables are built from a mass model
/// only, and the model is the reference their force is checked against.
#[test]
#[ignore = "slow: builds the (R, z) grid and 216 direct mass-model forces"]
fn the_force_is_the_mass_models_over_1_to_300_kpc() {
    let leapfrog = leapfrog(Seconds::from(Megayears::new(1.0)));
    let mut worst: (f64, [f64; 3]) = (0.0, [0.0; 3]);
    for r in radii(24) {
        for latitude in [-80.0, -30.0, -5.0, 0.0, 2.0, 10.0, 45.0, 70.0, 85.0] {
            let p = point(r, latitude, 37.0);
            let fast = leapfrog.acceleration_m_s2(&GalacticDisplacement::new(p.map(m)));
            let exact = model_acceleration(p);
            let error = length([0, 1, 2].map(|k| fast[k] - exact[k])) / length(exact);
            if error > worst.0 {
                worst = (error, p);
            }
        }
    }
    assert!(worst.0 < 1e-3, "{:e} at {:?} ly", worst.0, worst.1);
}

/// The circular speed from the force, `√(R |a_R|)` in the plane, is the tables' `v_circ` to 10⁻³
/// over 1–300 kpc.
#[test]
#[ignore = "slow: builds the (R, z) grid"]
fn the_circular_speed_from_the_force_is_the_tables() {
    let leapfrog = leapfrog(Seconds::from(Megayears::new(1.0)));
    for r in radii(40) {
        let a = leapfrog.acceleration_m_s2(&GalacticDisplacement::new([m(r), 0.0, 0.0]));
        let v = (m(r) * -a[0]).sqrt() / 1e3;
        let expected = fixture().tables.v_circ(LightYears::new(r)).value();
        assert!(
            (v / expected - 1.0).abs() < 1e-3,
            "{v} against {expected} km/s at {r} ly"
        );
        assert!(a[1].abs() < f64::EPSILON && a[2].abs() < f64::EPSILON);
    }
}

/// The vertical force is the mass model's `K_z` to 10⁻³ at twenty points off the plane, at
/// latitudes of 10–70° from 1 to 300 kpc. (Within about 2° of the plane the tables' vertical
/// force departs from the model's by up to 2 × 10⁻³ of itself, where it is small against the
/// radial force; the force as a whole stays within 10⁻³ there, as the test above shows.)
#[test]
#[ignore = "slow: builds the (R, z) grid and 20 direct mass-model forces"]
fn the_vertical_force_is_the_mass_models_off_the_plane() {
    let leapfrog = leapfrog(Seconds::from(Megayears::new(1.0)));
    let mut points = 0;
    for r in radii(5) {
        for latitude in [10.0, 30.0, -50.0, 70.0] {
            let p = point(r, latitude, 120.0);
            let a_z = leapfrog.acceleration_m_s2(&GalacticDisplacement::new(p.map(m)))[2];
            let k_z = fixture().model.vertical_force(
                LightYears::new(math::hypot(p[0], p[1])),
                LightYears::new(p[2]),
            );
            let expected = -k_z * FORCE_IN_SI;
            assert!(
                (a_z / expected - 1.0).abs() < 1e-3,
                "{a_z} against {expected} m/s² at {p:?} ly"
            );
            points += 1;
        }
    }
    assert_eq!(points, 20);
}

/// The field is the gradient of the leapfrog's potential everywhere, the blend from the tables to
/// the far field between 2¹⁷·⁵ and 2¹⁸ ly and the far field beyond included: central differences of
/// the potential match the acceleration to 10⁻⁶ of its size. A field that is not a gradient would
/// not hold the energy (module documentation, "The far field"). Within 1 ⁄ 16 ly of the plane
/// the tables' force is their in-plane `v_c²` table rather than the derivative of their in-plane
/// potential, which differ by about 2 × 10⁻⁵ at 150,000 ly, so the points keep off the plane.
#[test]
#[ignore = "slow: builds the (R, z) grid"]
fn the_force_is_the_gradient_of_the_potential_across_the_far_field() {
    let leapfrog = leapfrog(Seconds::from(Megayears::new(1.0)));
    for r in [
        150_000.0, 190_000.0, 225_000.0, 255_000.0, 262_144.0, 270_000.0, 400_000.0, 900_000.0,
    ] {
        for latitude in [-60.0, 0.5, 3.0, 35.0, 88.0] {
            let p = point(r, latitude, 250.0).map(m);
            let a = leapfrog.acceleration_m_s2(&GalacticDisplacement::new(p));
            let d = 1e-5 * length(p);
            let slope: [f64; 3] = [0, 1, 2].map(|k| {
                let (mut hi, mut lo) = (p, p);
                hi[k] += d;
                lo[k] -= d;
                (leapfrog.potential_j_kg(&GalacticDisplacement::new(hi))
                    - leapfrog.potential_j_kg(&GalacticDisplacement::new(lo)))
                    / (2.0 * d)
            });
            let error = length([0, 1, 2].map(|k| a[k] + slope[k])) / length(a);
            assert!(error < 1e-6, "{error:e} at {r} ly, latitude {latitude}°");
        }
    }
}

#[test]
fn a_leapfrog_refuses_a_step_it_cannot_take() {
    let tables = &fixture().tables;
    for bad in [0.0, -0.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            Leapfrog::new(tables, Seconds::new(bad)),
            Err(BuildLeapfrogError::InvalidStep(_))
        ));
    }
    let backwards = Leapfrog::new(tables, Seconds::new(-1e13)).expect("a negative step");
    for bad in [0.0, -0.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            backwards.with_step(Seconds::new(bad)),
            Err(BuildLeapfrogError::InvalidStep(_))
        ));
    }
    let forwards = backwards
        .with_step(Seconds::new(1e13))
        .expect("a positive step");
    assert!((forwards.time_step().value() - 1e13).abs() < f64::EPSILON);
}

// P10.T2.b: the fixed-step leapfrog.

/// Over ten radial periods at the step rule (1 ⁄ 64 of the pericentre crossing time `r_p ÷ v_p`, at
/// most 2 Myr) each pinned orbit holds its energy to its pin and to [`ENERGY_DRIFT`], and its
/// angular momentum about z to 10⁻¹². Each start's pinned spec is its probe's to 1 %.
#[test]
#[ignore = "slow: builds the (R, z) grid and integrates five orbits for 83 Gyr"]
fn energy_and_angular_momentum_are_conserved_over_ten_radial_periods() {
    for s in STARTS {
        let name = s.name;
        let measured = probe(&s);
        let pericentre = LightYears::from(measured.pericentre).value();
        let speed = measured.pericentre_speed.value() / 1e3;
        assert!(
            (pericentre / s.pericentre_ly - 1.0).abs() < 0.01
                && (speed / s.pericentre_speed_km_s - 1.0).abs() < 0.01,
            "{name}: the probe finds a pericentre of {pericentre:.0} ly at {speed:.2} km/s"
        );
        assert!(s.drift <= ENERGY_DRIFT, "{name}");
        let period = measured.radial_period;
        let fixed = s.fixed_step(period * 10.0);
        let leapfrog = leapfrog(fixed.step());
        let mut state = start(&leapfrog, s.position_ly, s.velocity_km_s);
        let (energy, l_z) = (
            leapfrog.energy_j_kg(&state),
            state.angular_momentum_z_m2_s(),
        );
        let (mut drift, mut torque) = (0.0_f64, 0.0_f64);
        for _ in 0..fixed.count() {
            leapfrog.step(&mut state);
            drift = drift.max((leapfrog.energy_j_kg(&state) / energy - 1.0).abs());
            torque = torque.max((state.angular_momentum_z_m2_s() / l_z - 1.0).abs());
        }
        // About 340 steps a radial period for the near-circular retrograde orbit, whose
        // crossing time is about its period ÷ 2π; the eccentric ones take thousands.
        assert!(fixed.count() >= 3_000, "{name}: {}", fixed.count());
        eprintln!(
            "{name}: {} steps of {:.3} Myr, radial period {:.1} Myr, energy {drift:.2e}, L_z \
             {torque:.2e}",
            fixed.count(),
            Megayears::from(fixed.step()).value(),
            Megayears::from(period).value()
        );
        assert!(drift < s.drift, "{name}: energy drifted by {drift:e}");
        assert!(torque < 1e-12, "{name}: L_z drifted by {torque:e}");
    }
}

/// Halving the step cuts the energy drift over ten radial periods by about four, the leapfrog's
/// h² (ruling 146.2 of 2026-09-22). The worst drift of an orbit is read at the steps, so a single
/// orbit's ratio scatters (3.2–4.6 over the five, measured at v15); the geometric mean over the
/// five is held to 3.5–4.5, and each ratio to 3–5.
#[test]
#[ignore = "slow: builds the (R, z) grid and integrates five orbits for 250 Gyr"]
fn the_energy_drift_falls_as_the_step_squared() {
    let mut log_sum = 0.0;
    for s in STARTS {
        let period = probe(&s).radial_period;
        let fixed = s.fixed_step(period * 10.0);
        let coarse = leapfrog(fixed.step());
        let fine = coarse
            .with_step(fixed.step() * 0.5)
            .expect("half a valid step");
        let (at_h, _) = energy_drift(&coarse, &s, fixed.count(), fixed.count());
        let (at_half, _) = energy_drift(&fine, &s, 2 * fixed.count(), 2 * fixed.count());
        let ratio = at_h / at_half;
        eprintln!(
            "{}: {at_h:.3e} at h, {at_half:.3e} at h ÷ 2, ratio {ratio:.2}",
            s.name
        );
        assert!((3.0..5.0).contains(&ratio), "{}: ratio {ratio}", s.name);
        log_sum += math::ln(ratio);
    }
    #[expect(clippy::cast_precision_loss, reason = "five starts")]
    let mean = math::exp(log_sum / STARTS.len() as f64);
    eprintln!("geometric mean ratio {mean:.2}");
    assert!((3.5..4.5).contains(&mean), "{mean}");
}

/// Over fifty radial periods the energy error does not grow: the worst drift in the last five
/// periods is at most twice the worst in the first five (ruling 146.2 of 2026-09-22). The
/// bicubic Hermite field is continuous in its gradient only, its Hessian jumping at every cell edge
/// and at both ends of the far field's blend, which voids the formal bound of a symplectic
/// integrator's shadow Hamiltonian, so errors from cell crossings could add up; ten periods cannot
/// show that. At v15 the last five read 0.7–1.2 times the first five.
#[test]
#[ignore = "slow: builds the (R, z) grid and integrates five orbits for 420 Gyr"]
fn the_energy_error_does_not_grow_over_fifty_radial_periods() {
    for s in STARTS {
        let period = probe(&s).radial_period;
        let fixed = s.fixed_step(period * 50.0);
        let leapfrog = leapfrog(fixed.step());
        let per_period = fixed.count() / 50;
        let (_, windows) = energy_drift(&leapfrog, &s, 50 * per_period, per_period);
        assert_eq!(windows.len(), 50);
        let worst = |w: &[f64]| w.iter().copied().fold(0.0, f64::max);
        let (first, last) = (worst(&windows[..5]), worst(&windows[45..]));
        eprintln!(
            "{}: first five periods {first:.3e}, last five {last:.3e}, ratio {:.2}",
            s.name,
            last / first
        );
        assert!(last <= 2.0 * first, "{}: {first:e}, then {last:e}", s.name);
    }
}

/// Two starts whose energies differ by 2 × 10⁻³ of |E|, about a Pal 5-like stream's tracer offset
/// `ε ≈ r_t ∂Φ ÷ ∂r` (research `r-int10`), keep that difference to 1 % over ten radial periods at
/// the progenitor's step: the energy offsets that set a stream's track are not distorted by the
/// leapfrog's shared error (ruling 146.2 of 2026-09-22, the optional check). The difference is of
/// the energies averaged over each radial period: the two orbits drift apart in phase, so their
/// instantaneous energy errors, each up to [`Start::drift`], decorrelate and read 1.2–4.2 % of
/// ε at worst for the eccentric orbits at v15, while the averages, which follow each orbit's
/// shadow Hamiltonian, hold 0.04–0.27 %, except the outer orbit's 1.04 %, held to 1.5 %.
#[test]
#[ignore = "slow: builds the (R, z) grid and integrates ten orbits for 83 Gyr"]
fn neighbouring_orbits_keep_their_energy_difference() {
    for s in STARTS {
        let period = probe(&s).radial_period;
        let fixed = s.fixed_step(period * 10.0);
        let leapfrog = leapfrog(fixed.step());
        let mut a = start(&leapfrog, s.position_ly, s.velocity_km_s);
        let energy = leapfrog.energy_j_kg(&a);
        let v = a.velocity().metres_per_second();
        let offset = 2e-3 * energy.abs();
        let scale = (dot(v, v) + 2.0 * offset).sqrt() / dot(v, v).sqrt();
        let mut b = leapfrog.state(a.position(), GalacticVelocity::new(v.map(|u| u * scale)));
        let difference = leapfrog.energy_j_kg(&b) - energy;
        assert!((difference / offset - 1.0).abs() < 1e-6);
        let per_period = fixed.count() / 10;
        let mut worst = 0.0_f64;
        for _ in 0..10 {
            let mut sum = 0.0;
            for _ in 0..per_period {
                leapfrog.step(&mut a);
                leapfrog.step(&mut b);
                sum += leapfrog.energy_j_kg(&b) - leapfrog.energy_j_kg(&a);
            }
            let mean = sum / f64::from(per_period);
            worst = worst.max((mean / difference - 1.0).abs());
        }
        eprintln!("{}: the energy difference kept to {worst:.2e}", s.name);
        // The outer orbit's difference grows steadily to 1.04 % by the tenth period, with its
        // apocentre at the far field's blend and the grid's edge; at a step 0.3 % shorter it read
        // 0.19 %. Pinned, not ruled (plan 10, Risks).
        let bound = if s.name == "outer" { 0.015 } else { 0.01 };
        assert!(worst < bound, "{}: {worst:e}", s.name);
    }
}

/// Integrating forwards, reversing the velocity and integrating as long again returns each start
/// to 10⁻⁹ of its position and velocity.
#[test]
#[ignore = "slow: builds the (R, z) grid"]
fn time_reversal_returns_the_start() {
    let leapfrog = leapfrog(Seconds::from(Megayears::new(1.0)));
    for Start {
        name,
        position_ly,
        velocity_km_s,
        ..
    } in STARTS
    {
        let begin = start(&leapfrog, position_ly, velocity_km_s);
        let mut state = begin;
        leapfrog.integrate(&mut state, 5_000);
        let mut back = state.reversed();
        leapfrog.integrate(&mut back, 5_000);
        let back = back.reversed();
        let p = begin.position().metres();
        let v = begin.velocity().metres_per_second();
        let dp = back.position().metres();
        let dv = back.velocity().metres_per_second();
        let position_error = length([0, 1, 2].map(|k| dp[k] - p[k])) / length(p);
        let velocity_error = length([0, 1, 2].map(|k| dv[k] - v[k])) / length(v);
        assert!(position_error < 1e-9, "{name}: {position_error:e}");
        assert!(velocity_error < 1e-9, "{name}: {velocity_error:e}");
    }
}

/// An orbit started at the circular speed in the plane stays in the plane and at its radius over
/// ten periods at 256 steps a period: the leapfrog's own error puts it on a slightly different
/// circle with a small epicycle, of order `(Ω h)² ÷ 4 = (2π ÷ 256)² ÷ 4 ≈ 1.5 × 10⁻⁴` of the radius,
/// which must not grow.
#[test]
#[ignore = "slow: builds the (R, z) grid"]
fn a_circular_orbit_stays_circular() {
    for r in [8_000.0, 26_000.0, 100_000.0] {
        let v = fixture().tables.v_circ(LightYears::new(r)).value() * 1e3;
        let period = 2.0 * core::f64::consts::PI * m(r) / v;
        let leapfrog = leapfrog(Seconds::new(period / 256.0));
        let mut state = leapfrog.state(
            GalacticDisplacement::new([m(r), 0.0, 0.0]),
            GalacticVelocity::new([0.0, v, 0.0]),
        );
        let (mut first, mut worst) = (0.0_f64, 0.0_f64);
        for i in 0..2_560 {
            leapfrog.step(&mut state);
            worst = worst.max((state.radius().value() / m(r) - 1.0).abs());
            if i < 256 {
                first = worst;
            }
            assert!(state.position().metres()[2].abs() < f64::EPSILON);
        }
        eprintln!(
            "{r} ly: the radius strayed by {first:.2e} in the first period, {worst:.2e} in ten"
        );
        assert!(worst < 3e-4, "{r} ly: the radius strayed by {worst:e}");
        assert!(worst <= 1.05 * first, "{r} ly: {first:e}, then {worst:e}");
        let omega = leapfrog.integrate(&mut state, 256).mean_angular_frequency();
        let expected = fixture().tables.omega(LightYears::new(r)).value();
        assert!(
            (omega.value() / expected - 1.0).abs() < 1e-3,
            "{r} ly: Ω̄ {} against {expected}",
            omega.value()
        );
    }
}

/// The summary of each pinned orbit: turning points that alternate within its bounding radii, a
/// pericentre below its apocentre, and a mean angular frequency between the circular frequencies
/// at the two.
#[test]
#[ignore = "slow: builds the (R, z) grid and integrates five orbits for 33 Gyr"]
fn the_summary_reads_the_turning_points() {
    for s in STARTS {
        let name = s.name;
        let period = probe(&s).radial_period;
        let fixed = s.fixed_step(period * 4.0);
        let leapfrog = leapfrog(fixed.step());
        let mut state = start(&leapfrog, s.position_ly, s.velocity_km_s);
        let summary = leapfrog.integrate(&mut state, fixed.count());
        let (peri, apo) = (summary.pericentre().unwrap(), summary.apocentre().unwrap());
        let [inner, outer] = summary.bounding_radii();
        assert!(inner.value() <= peri.value() && peri.value() < apo.value());
        assert!(apo.value() <= outer.value());
        assert!(
            (7..=9).contains(&summary.turning_points()),
            "{name}: {summary:?}"
        );
        // In a flattened potential r(t) is quasi-periodic, not periodic, so the mean interval
        // between turning points depends a little on which are counted: over four periods against
        // the probe's four gigayears, the three orbits differ by up to 1.1 × 10⁻³.
        let measured = summary.radial_period().unwrap().value();
        assert!(
            (measured / period.value() - 1.0).abs() < 5e-3,
            "{name}: {measured} against {} s",
            period.value()
        );
        let omega = summary.mean_angular_frequency().value();
        let at = |r: Metres| fixture().tables.omega(LightYears::from(r)).value();
        assert!(at(apo) < omega && omega < at(peri), "{name}: Ω̄ = {omega}");
        let [peri_ly, apo_ly] = [peri, apo].map(|r| LightYears::from(r).value());
        eprintln!(
            "{name}: pericentre {peri_ly:.0} ly, apocentre {apo_ly:.0} ly, radial period {:.1} Myr",
            Megayears::from(period).value()
        );
    }
}

// P10.T2.c: determinism across platforms.

/// The exact bits of each of three pinned orbits after 4 Gyr at the step rule's step from its
/// spec (3,235–11,209 steps), the step itself, and the orbit's summary. The bits depend on the
/// potential tables and on the step rule, so a change to the galaxy's mass model or to the rule
/// moves them.
#[test]
fn golden_orbits() {
    let mut writer = GoldenWriter::new();
    writer.header(GENERATOR_VERSION.get());
    for s in STARTS
        .into_iter()
        .filter(|s| ["inner", "retrograde", "outer"].contains(&s.name))
    {
        let name = s.name;
        let fixed = s.fixed_step(Seconds::from(Gigayears::new(4.0)));
        let leapfrog = leapfrog(fixed.step());
        writer.f64(&format!("{name}.step"), fixed.step().value());
        writer.line(&format!("{name}.steps {}", fixed.count()));
        let mut state = start(&leapfrog, s.position_ly, s.velocity_km_s);
        let summary = leapfrog.integrate(&mut state, fixed.count());
        let p = state.position().metres();
        let v = state.velocity().metres_per_second();
        for (axis, (x, u)) in ["x", "y", "z"].iter().zip(p.iter().zip(v)) {
            writer.f64(&format!("{name}.position.{axis}"), *x);
            writer.f64(&format!("{name}.velocity.{axis}"), u);
        }
        writer.f64(
            &format!("{name}.pericentre"),
            summary.pericentre().unwrap().value(),
        );
        writer.f64(
            &format!("{name}.apocentre"),
            summary.apocentre().unwrap().value(),
        );
        writer.f64(
            &format!("{name}.radial_period"),
            summary.radial_period().unwrap().value(),
        );
        writer.f64(
            &format!("{name}.mean_angular_frequency"),
            summary.mean_angular_frequency().value(),
        );
        writer.line(&format!(
            "{name}.turning_points {}",
            summary.turning_points()
        ));
    }
    golden!("galaxy/global_list/orbit", writer.as_str());
}

/// The same start integrated twice, and as one run against steps one at a time, gives the same
/// bits.
#[test]
fn the_integration_is_deterministic() {
    let leapfrog = leapfrog(Seconds::from(Megayears::new(0.5)));
    let s = STARTS[1];
    let mut once = start(&leapfrog, s.position_ly, s.velocity_km_s);
    let mut again = once;
    let mut stepped = once;
    let first = leapfrog.integrate(&mut once, 2_000);
    let second = leapfrog.integrate(&mut again, 2_000);
    for _ in 0..2_000 {
        leapfrog.step(&mut stepped);
    }
    assert_eq!(first, second);
    assert_eq!(once, again);
    assert_eq!(once, stepped);
}
