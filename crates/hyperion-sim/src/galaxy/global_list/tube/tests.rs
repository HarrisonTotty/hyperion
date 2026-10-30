//! The tube table's unit tests (P10.T6.a): the reduction of a synthetic stream on a circle, whose
//! centre line, frame, widths and dispersions are known. The test streams' sprays, which take the
//! full potential and seconds of integration, are `tests/global_list_tube.rs`'s.

use super::*;
use crate::Seed;
use crate::galaxy::global_list::spec::SphericalOrbit;
use crate::galaxy::global_list::{MassLoss, StreamOrigin};
use crate::galaxy::params::GalaxyParams;
use crate::rng::{ObjectKey, Stream, tags};
use crate::units::{SolarMassesPerYear, Years};

/// The circle's radius, ly.
const RADIUS: f64 = 50_000.0;
/// Each arm's extent in ψ, rad.
const ARC: f64 = 1.0;
/// The tracers' rms offsets outward and out of the plane, ly.
const SPREAD: [f64; 2] = [100.0; 2];
/// The circular speed, m/s, and the velocity dispersions along the track, outward and out of the
/// plane, m/s.
const SPEED: f64 = 2.2e5;
const DISPERSION: [f64; 3] = [3_000.0, 1_000.0, 500.0];
/// The tube's mass, M☉.
const MASS: f64 = 2e4;
/// 10⁻⁹ ly in metres.
const NANO_LY: f64 = 1e-9 * METRES_PER_LIGHT_YEAR;

/// A specification for the tables to name: only its number is read.
fn spec() -> StreamSpec {
    let orbit = SphericalOrbit::from_turning_points(|r| -1e12 / r, 40_000.0, 60_000.0)
        .expect("Kepler turning points bound an orbit");
    StreamSpec {
        number: StreamNumber::new(7).expect("below the limit"),
        origin: StreamOrigin::Orphan,
        position: GalacticDisplacement::new([RADIUS * METRES_PER_LIGHT_YEAR, 0.0, 0.0]),
        velocity: GalacticVelocity::new([0.0, SPEED, 0.0]),
        orbit,
        stripping_time: Years::new(3e9),
        mass: SolarMasses::new(MASS),
        progenitor_mass: SolarMasses::new(1e4),
        tidal_radius: LightYears::new(50.0),
        mass_loss: MassLoss::Steady(SolarMassesPerYear::new(MASS / 3e9)),
        stars: None,
    }
}

/// `per_arm` tracers from each Lagrange point of a stream on a circle of [`RADIUS`] in the
/// galactic plane, moving anticlockwise from the x axis: tracer `2k` leads at ψ from 0 to [`ARC`]
/// and `2k + 1` trails, each at the angle ψ about the centre, with Gaussian offsets of [`SPREAD`]
/// outward and up and velocities of [`SPEED`] along the circle with [`DISPERSION`]. `astray`
/// leading tracers of every hundred trail instead, as a spray's few do.
fn circle(per_arm: u32, astray: u32) -> Vec<Tracer> {
    let mut draws = Stream::open(
        Seed::new(0x0a10_6a00),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy(),
    );
    let weight = SolarMasses::new(MASS / f64::from(2 * per_arm));
    (0..2 * per_arm)
        .map(|i| {
            let k = i / 2;
            let arm = if i.is_multiple_of(2) {
                Arm::Leading
            } else {
                Arm::Trailing
            };
            let gained = ARC * (f64::from(k) + draws.uniform()) / f64::from(per_arm);
            let behind = arm == Arm::Trailing || k % 100 < astray;
            let psi = if behind { -gained } else { gained };
            let (sin, cos) = math::sin_cos(psi);
            let out = RADIUS + SPREAD[0] * draws.standard_normal();
            let up = SPREAD[1] * draws.standard_normal();
            let position = [out * cos, out * sin, up].map(|c| c * METRES_PER_LIGHT_YEAR);
            let along = SPEED + DISPERSION[0] * draws.standard_normal();
            let outward = DISPERSION[1] * draws.standard_normal();
            let vertical = DISPERSION[2] * draws.standard_normal();
            let velocity = [
                -along * sin + outward * cos,
                along * cos + outward * sin,
                vertical,
            ];
            Tracer::from_parts(
                arm,
                0,
                Years::new(1e9),
                weight,
                GalacticDisplacement::new(position),
                GalacticVelocity::new(velocity),
                Radians::new(psi),
            )
        })
        .collect()
}

fn table(tracers: &[Tracer]) -> TubeTable {
    TubeTable::from_tracers(&spec(), tracers).expect("enough tracers in each arm")
}

fn ly(position: &GalacticPosition) -> [f64; 3] {
    position.to_light_years_f64()
}

fn length(v: [f64; 3]) -> f64 {
    dot(v, v).sqrt()
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

/// The arms are split by ψ's sign, not by the Lagrange point (main's ruling of 2026-10-29): the
/// strays join the arm of their sign with their weight, a ψ of zero of either sign is the
/// leading arm's, and the arms' masses add up to the tube's.
#[test]
fn the_arms_are_split_by_the_sign_of_psi() {
    let mut tracers = circle(600, 3);
    let behind = tracers.iter().filter(|t| t.psi().value() < 0.0).count();
    assert_eq!(
        behind,
        600 + 18,
        "three of every hundred leading tracers trail"
    );
    // Two leading tracers that lead, at ψ of zero, one of each sign.
    for (i, zero) in [(6, 0.0), (8, -0.0)] {
        let t = tracers[i];
        tracers[i] = Tracer::from_parts(
            t.arm(),
            t.release_step(),
            t.released(),
            t.weight(),
            t.position(),
            t.velocity(),
            Radians::new(zero),
        );
    }
    let tube = table(&tracers);
    assert_eq!(tube.stream(), spec().number());
    assert_eq!(tube.arm(Arm::Trailing).tracers(), 618);
    assert_eq!(tube.arm(Arm::Leading).tracers(), 582);
    let weight = MASS / 1_200.0;
    let trailing = tube.arm(Arm::Trailing).mass().value();
    assert!(
        (trailing / (618.0 * weight) - 1.0).abs() < 1e-12,
        "{trailing}"
    );
    let sum = trailing + tube.arm(Arm::Leading).mass().value();
    assert_eq!(tube.mass().value(), sum);
    assert!((tube.mass().value() / MASS - 1.0).abs() < 1e-12);
    // The leading arm's root is at its ψ of zero.
    for (arm, sign) in [(Arm::Trailing, -1.0), (Arm::Leading, 1.0)] {
        let knots = tube.arm(arm).knots();
        assert_eq!(knots.len(), 256);
        assert!(
            knots.iter().all(|k| k.psi().value() * sign >= 0.0)
                && knots.iter().all(|k| k.psi().value() < 0.0) == (arm == Arm::Trailing),
            "{arm:?}"
        );
    }
}

/// The centre line follows the circle and its length is the arc's between the end knots; the
/// knots run in ψ, and Λ rises by the same mass from knot to knot within an arm, from zero at the
/// trailing tip to the tube's mass at the leading tip.
#[test]
fn the_centre_line_and_the_line_density_follow_the_tracers() {
    let tube = table(&circle(600, 0));
    let knots: Vec<&TubeKnot> = tube.knots().collect();
    assert_eq!(knots.len(), 512);
    for k in &knots {
        let [x, y, z] = ly(&k.centre());
        // The kernel's mean lies inside the arc by about R σ_ψ² ÷ 2, 7.7 ly for its window of
        // ±2σ, and its noise is about 16 ly, the offsets' 100 ly over the root of the kernel's
        // effective 39 tracers, and some 37 ly at the arms' ends, where the local-linear fit rests
        // on an effective 7.
        let radius = math::hypot(x, y);
        assert!((radius - RADIUS).abs() < 150.0, "{radius}");
        assert!(z.abs() < 150.0, "{z}");
        let angle = math::atan2(y, x);
        assert!(
            (angle - k.psi().value()).abs() < 2e-3,
            "{angle} at ψ {}",
            k.psi().value()
        );
    }
    assert!(
        knots
            .windows(2)
            .all(|w| w[1].psi().value() > w[0].psi().value())
    );
    let span = knots[511].psi().value() - knots[0].psi().value();
    let length = tube.length().value();
    assert!(
        (length / (RADIUS * span) - 1.0).abs() < 2e-3,
        "{length} over {span} rad"
    );
    // The end knots' local-linear means reach the tips.
    assert!((span / (2.0 * ARC) - 1.0).abs() < 2e-3, "{span}");
    let first = knots[0].cumulative_mass().value();
    assert_eq!(first, 0.0);
    assert_eq!(knots[511].cumulative_mass().value(), tube.mass().value());
    for arm in [Arm::Trailing, Arm::Leading] {
        let step = tube.arm(arm).mass().value() / 255.0;
        for pair in tube.arm(arm).knots().windows(2) {
            let rise = pair[1].cumulative_mass().value() - pair[0].cumulative_mass().value();
            assert!(
                (rise / step - 1.0).abs() < 1e-9,
                "{arm:?}: {rise} against {step}"
            );
        }
    }
}

/// Frames orthonormal to 10⁻¹² and right-handed at every knot, never flipping from knot to knot;
/// on a stream in one plane the binormal stays on the orbit's pole and the tangent along the
/// motion.
#[test]
fn the_frames_are_orthonormal_and_carried_without_a_flip() {
    let tube = table(&circle(600, 3));
    let knots: Vec<&TubeKnot> = tube.knots().collect();
    for k in &knots {
        let [t, n, b] = k.frame();
        for (v, what) in [(t, "tangent"), (n, "normal"), (b, "binormal")] {
            assert!((dot(v, v) - 1.0).abs() < 1e-12, "{what}: {}", dot(v, v));
        }
        for (u, v) in [(t, n), (t, b), (n, b)] {
            assert!(dot(u, v).abs() < 1e-12, "{}", dot(u, v));
        }
        let handed = cross(t, n);
        assert!(length([handed[0] - b[0], handed[1] - b[1], handed[2] - b[2]]) < 1e-12);
        // The knots' noise out of the plane tilts the fitted tangent, and the frame carried along
        // it turns a little about it: 2.8° at most here, 5.7° allowed.
        assert!(b[2] > 0.995, "the binormal leaves the pole: {b:?}");
        let (sin, cos) = math::sin_cos(k.psi().value());
        // The kernel fit's direction turns with the knots' noise, most at the arms' ends: 3.7° at
        // most here, 8° allowed.
        assert!(
            dot(t, [-sin, cos, 0.0]) > 0.99,
            "the tangent leaves the motion: {t:?}"
        );
    }
    for pair in knots.windows(2) {
        let [_, n0, _] = pair[0].frame();
        let [_, n1, _] = pair[1].frame();
        assert!(dot(n0, n1) > 0.999, "the normal turns by {}", dot(n0, n1));
    }
}

/// The widths and dispersions are the tracers' offsets', measured about the centre line and the
/// mean velocity at each tracer's own rank: the normal is inward and the binormal up on this
/// circle, so the widths are [`SPREAD`] and the dispersions [`DISPERSION`], each knot's to the
/// kernel's noise and their medians to 8 %. A knot's rms rests on the kernel's effective 39
/// tracers, 22 at an arm's end, where its one-sided kernel reaches only one way: about 11 % of
/// noise, and 15 % at the ends.
#[test]
fn the_widths_and_dispersions_are_the_tracers_rms_offsets() {
    let tube = table(&circle(600, 0));
    let knots: Vec<&TubeKnot> = tube.knots().collect();
    for (axis, spread) in SPREAD.into_iter().enumerate() {
        let widths: Vec<f64> = knots.iter().map(|k| k.widths()[axis].value()).collect();
        assert!(
            widths.iter().all(|w| (w / spread - 1.0).abs() < 0.5),
            "{widths:?}"
        );
        let middle = median(widths);
        assert!(
            (middle / spread - 1.0).abs() < 0.08,
            "width {axis}: {middle}"
        );
    }
    for (axis, dispersion) in DISPERSION.into_iter().enumerate() {
        let sigmas: Vec<f64> = knots
            .iter()
            .map(|k| k.dispersions()[axis].value())
            .collect();
        assert!(
            sigmas.iter().all(|s| (s / dispersion - 1.0).abs() < 0.5),
            "{sigmas:?}"
        );
        let middle = median(sigmas);
        assert!(
            (middle / dispersion - 1.0).abs() < 0.08,
            "dispersion {axis}: {middle}"
        );
    }
}

/// `locate`, `mean_velocity` and `psi` are linear between knots in the along coordinate, the
/// share of the tube's mass behind the point: at a knot's share they give the knot's quantities,
/// the across coordinates step by a width along the normal and the binormal, and `u` is clamped.
#[test]
fn locate_mean_velocity_and_psi_invert_the_line_density() {
    let tube = table(&circle(600, 0));
    let total = tube.mass().value();
    let knots: Vec<&TubeKnot> = tube.knots().collect();
    // The two arms' roots share one u, the trailing arm's share of the mass, and it names one of
    // them; every other knot's u names that knot.
    let junction = tube.locate(knots[255].cumulative_mass().value() / total, 0.0, 0.0);
    assert!(
        junction == knots[255].centre() || junction == knots[256].centre(),
        "{junction:?} is neither root"
    );
    for (j, k) in knots
        .iter()
        .enumerate()
        .filter(|(j, _)| !matches!(j, 255 | 256))
    {
        let u = k.cumulative_mass().value() / total;
        let at = tube.locate(u, 0.0, 0.0);
        // u's rounding moves the point along by about 10⁻¹³ of a segment, 10⁻¹¹ ly.
        assert!(at.distance_to(&k.centre()).value() < NANO_LY, "knot {j}");
        assert!(
            (tube.psi(u).value() - k.psi().value()).abs() < 1e-12,
            "knot {j}"
        );
        let v = tube.mean_velocity(u).metres_per_second();
        let w = k.velocity().metres_per_second();
        assert!(
            length([v[0] - w[0], v[1] - w[1], v[2] - w[2]]) < 1e-6,
            "knot {j}"
        );
        let [_, normal, binormal] = k.frame();
        for (a, b, axis, width) in [
            (1.0, 0.0, normal, k.widths()[0]),
            (0.0, -2.0, binormal, k.widths()[1]),
        ] {
            let step = at.displacement_to(&tube.locate(u, a, b)).metres();
            let expected = (a + b) * width.value() * METRES_PER_LIGHT_YEAR;
            let off: [f64; 3] = core::array::from_fn(|i| step[i] - expected * axis[i]);
            // A displacement of 200 ly holds about 256 m in f64 metres, the sum of its terms more.
            assert!(length(off) < 1e4, "knot {j}: {off:?} m");
        }
    }
    assert_eq!(tube.locate(-0.5, 0.0, 0.0), knots[0].centre());
    assert_eq!(tube.locate(1.5, 0.0, 0.0), knots[511].centre());
    // Halfway between two knots of an arm, halfway between their quantities.
    let (p, q) = (knots[300], knots[301]);
    let u = f64::midpoint(p.cumulative_mass().value(), q.cumulative_mass().value()) / total;
    let psi = f64::midpoint(p.psi().value(), q.psi().value());
    assert!((tube.psi(u).value() - psi).abs() < 1e-12);
    let middle = p
        .centre()
        .displacement_to(&tube.locate(u, 0.0, 0.0))
        .metres();
    let half = p
        .centre()
        .displacement_to(&q.centre())
        .metres()
        .map(|c| 0.5 * c);
    assert!(
        length([
            middle[0] - half[0],
            middle[1] - half[1],
            middle[2] - half[2]
        ]) < NANO_LY
    );
    // ψ rises with u everywhere.
    let psis: Vec<f64> = (0..=10_000)
        .map(|i| tube.psi(f64::from(i) / 1e4).value())
        .collect();
    assert!(psis.windows(2).all(|w| w[1] >= w[0]));
    // A uniform u is a draw by mass: the share of u behind ψ = 0 is the trailing arm's.
    let behind = psis.iter().filter(|&&psi| psi < 0.0).count();
    assert!((f64::from(u32::try_from(behind).unwrap()) / 1e4 - 0.5).abs() < 2e-3);
}

/// The table is a pure function of the tracers: the same list twice gives the same bits, and,
/// with no two tracers of equal ψ, so does the list in any order.
#[test]
fn the_table_is_a_pure_function_of_its_tracers() {
    let tracers = circle(300, 2);
    let tube = table(&tracers);
    assert_eq!(table(&tracers), tube);
    let reversed: Vec<Tracer> = tracers.iter().rev().copied().collect();
    assert_eq!(table(&reversed), tube);
    let other = table(&circle(300, 3));
    assert_ne!(other, tube);
}

/// An arm needs a kernel's worth of tracers, [`KERNEL_TRACERS`].
#[test]
fn an_arm_of_fewer_tracers_than_a_kernel_is_an_error() {
    let mut tracers = circle(600, 0);
    // Keep 47 trailing tracers.
    let mut kept = 0;
    tracers.retain(|t| {
        let keep = t.psi().value() >= 0.0 || kept < 47;
        if t.psi().value() < 0.0 && keep {
            kept += 1;
        }
        keep
    });
    assert_eq!(
        TubeTable::from_tracers(&spec(), &tracers),
        Err(BuildTubeTableError::TooFewTracers {
            arm: Arm::Trailing,
            tracers: 47
        })
    );
    assert_eq!(
        BuildTubeTableError::TooFewTracers {
            arm: Arm::Trailing,
            tracers: 47
        }
        .to_string(),
        "the trailing arm holds 47 tracers, fewer than a knot's kernel of 48"
    );
}

/// `build` sprays the stream, so a galaxy without the full potential is the spray's error, which
/// the table's error carries as its source.
#[test]
fn building_without_the_full_potential_is_the_spray_s_error() {
    let galaxy = Galaxy::from_params(Seed::new(0x0a10_6a01), GalaxyParams::milky_way_like())
        .expect("the fixture's parameters are valid");
    let error = TubeTable::build(&galaxy, &spec()).expect_err("no (R, z) grid");
    assert_eq!(
        error,
        BuildTubeTableError::Spray(BuildLeapfrogError::NoVerticalGrid)
    );
    assert_eq!(error.to_string(), "the stream's spray could not be made");
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("the potential tables have no (R, z) grid")
    );
}

/// Tracers of equal ψ keep their order in the list, the sort being stable: a spray's last
/// release can fall on the epoch's boundary, and its two tracers then both have ψ of +0.0. With
/// such a tie the table is unchanged when the other tracers are permuted, and changes when the
/// tied pair is swapped.
#[test]
fn tracers_of_equal_psi_keep_their_order() {
    let mut tracers = circle(300, 0);
    for i in [10, 12] {
        let t = tracers[i];
        tracers[i] = Tracer::from_parts(
            t.arm(),
            t.release_step(),
            t.released(),
            t.weight(),
            t.position(),
            t.velocity(),
            Radians::new(0.0),
        );
    }
    let tube = table(&tracers);
    let mut permuted = tracers.clone();
    permuted.swap(0, 599);
    permuted.swap(3, 400);
    assert_eq!(
        table(&permuted),
        tube,
        "permuting untied tracers moves nothing"
    );
    let mut swapped = tracers.clone();
    swapped.swap(10, 12);
    assert_ne!(table(&swapped), tube, "the tie's order is the list's");
}

/// Knots sit at equal steps of cumulative weight, not of rank, when the weights differ, as a
/// dwarf's pulses' or a merged spray's might: each knot's Λ is the weight of the tracers behind
/// its ψ to within one tracer's (the last knot's is the whole arm, its tip's tracer included), and
/// Λ still sums to the tracers' weights.
#[test]
fn the_line_density_follows_unequal_weights() {
    let tracers: Vec<Tracer> = circle(600, 0)
        .into_iter()
        .enumerate()
        .map(|(i, t)| {
            // Weights rising threefold along each arm's index.
            let scale = 1.0 + 2.0 * f64::from(u32::try_from(i / 2).unwrap()) / 600.0;
            Tracer::from_parts(
                t.arm(),
                t.release_step(),
                t.released(),
                SolarMasses::new(t.weight().value() * scale),
                t.position(),
                t.velocity(),
                t.psi(),
            )
        })
        .collect();
    let heaviest = tracers
        .iter()
        .map(|t| t.weight().value())
        .fold(0.0, f64::max);
    let total = tracers.iter().fold(0.0, |sum, t| sum + t.weight().value());
    let tube = table(&tracers);
    assert!((tube.mass().value() / total - 1.0).abs() < 1e-12);
    for (j, k) in tube.knots().enumerate() {
        let psi = k.psi().value();
        let behind = tracers
            .iter()
            .filter(|t| t.psi().value() < psi)
            .fold(0.0, |sum, t| sum + t.weight().value());
        let lambda = k.cumulative_mass().value();
        assert!(
            (lambda - behind).abs() <= heaviest * (1.0 + 1e-9),
            "knot {j}: Λ {lambda} against {behind} behind its ψ"
        );
    }
}
