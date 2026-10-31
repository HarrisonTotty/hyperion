//! The tube table's unit tests (P10.T6.a and T6.b): the reduction of a synthetic stream on a
//! circle, whose centre line, frame, widths and dispersions are known, and its gap noise. The test
//! streams' sprays, which take the full potential and seconds of integration, are
//! `tests/global_list_tube.rs`'s.

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
/// The size of the tracers' energy offsets at release, J/kg: a stream's tracers' are of order
/// the circular speed times a few km/s.
const ENERGY_OFFSET: f64 = 1e9;
/// The seed of the tables' gap noise.
const SEED: Seed = Seed::new(0x0a10_6b00);
/// The mean system mass the tables' along cells are cut for, M☉: a round figure near the Milky
/// Way fixture's halo's 0.61 (`Galaxy::mean_system_mass`), which puts 512 cells in [`MASS`].
const MEAN_MASS: f64 = 0.55;

/// A specification for the tables to name: only its number is read.
fn spec() -> StreamSpec {
    numbered(7)
}

/// [`spec`] with stream number `number`.
fn numbered(number: u16) -> StreamSpec {
    let orbit = SphericalOrbit::from_turning_points(|r| -1e12 / r, 40_000.0, 60_000.0)
        .expect("Kepler turning points bound an orbit");
    StreamSpec {
        number: StreamNumber::new(number).expect("below the limit"),
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
/// outward and up and velocities of [`SPEED`] along the circle with [`DISPERSION`], and an energy
/// offset of [`ENERGY_OFFSET`] of its arm's sign. `astray` leading tracers of every hundred trail
/// instead, with the trailing arm's energy, as a spray's few do.
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
            let (psi, energy) = if behind {
                (-gained, ENERGY_OFFSET)
            } else {
                (gained, -ENERGY_OFFSET)
            };
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
                energy,
            )
        })
        .collect()
}

/// `t` with the energy offset `energy`, J/kg, in place of its own.
fn with_energy(t: &Tracer, energy: f64) -> Tracer {
    Tracer::from_parts(
        t.arm(),
        t.release_step(),
        t.released(),
        t.weight(),
        t.position(),
        t.velocity(),
        t.psi(),
        energy,
    )
}

fn table(tracers: &[Tracer]) -> TubeTable {
    built(SEED, MEAN_MASS, &spec(), tracers)
}

/// The table of `tracers` with the gap noise of `seed` and along cells for `mean_mass`.
fn built(seed: Seed, mean_mass: f64, spec: &StreamSpec, tracers: &[Tracer]) -> TubeTable {
    TubeTable::from_tracers(seed, SolarMasses::new(mean_mass), spec, tracers)
        .expect("enough tracers in each arm")
}

/// Every along cell's gap noise, in cell order.
fn noises(tube: &TubeTable) -> Vec<f64> {
    (0..tube.along_cells())
        .map(|cell| tube.noise(cell))
        .collect()
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

/// The arms are split by the energy's sign at release, not by the Lagrange point nor by ψ's sign
/// (main's ruling of 2026-10-30): the strays join the arm of their energy with their weight, those
/// behind the progenitor with the leading arm's energy at that arm's low end of ψ, an energy of
/// zero of either sign is the trailing arm's, and the arms' masses add up to the tube's.
#[test]
fn the_arms_are_split_by_the_energy_s_sign() {
    let mut tracers = circle(600, 3);
    let behind = tracers.iter().filter(|t| t.psi().value() < 0.0).count();
    assert_eq!(
        behind,
        600 + 18,
        "three of every hundred leading tracers trail"
    );
    // Four of the strays behind keep the leading arm's energy, and two leading tracers ahead have
    // an energy offset of zero, one of each sign.
    for i in [0, 2, 200, 202] {
        let psi = tracers[i].psi().value();
        assert!(psi < 0.0, "tracer {i} at ψ {psi}");
        assert_eq!(tracers[i].arm(), Arm::Leading, "tracer {i}");
        tracers[i] = with_energy(&tracers[i], -ENERGY_OFFSET);
    }
    for (i, zero) in [(6, 0.0), (8, -0.0)] {
        let psi = tracers[i].psi().value();
        assert!(psi > 0.0, "tracer {i} at ψ {psi}");
        tracers[i] = with_energy(&tracers[i], zero);
        assert_eq!(tracers[i].energy_arm(), Arm::Trailing, "{zero:?}");
    }
    let tube = table(&tracers);
    assert_eq!(tube.stream(), spec().number());
    assert_eq!(tube.arm(Arm::Trailing).tracers(), 616);
    assert_eq!(tube.arm(Arm::Leading).tracers(), 584);
    let weight = MASS / 1_200.0;
    let trailing = tube.arm(Arm::Trailing).mass().value();
    assert!(
        (trailing / (616.0 * weight) - 1.0).abs() < 1e-12,
        "{trailing}"
    );
    let sum = trailing + tube.arm(Arm::Leading).mass().value();
    assert_eq!(tube.mass().value(), sum);
    let mass = tube.mass().value();
    assert!((mass / MASS - 1.0).abs() < 1e-12, "{mass}");
    // Each arm's knots run in ψ: the leading arm's first lies behind the progenitor, at the strays
    // that kept its energy, and the trailing arm's last ahead of it, at the zeros.
    for arm in [Arm::Trailing, Arm::Leading] {
        let knots = tube.arm(arm).knots();
        assert_eq!(knots.len(), 256);
        assert!(
            knots
                .windows(2)
                .all(|w| w[0].psi().value() <= w[1].psi().value()),
            "{arm:?}"
        );
    }
    let first = tube.arm(Arm::Leading).knots()[0].psi().value();
    assert!(first < 0.0, "the leading arm starts at ψ {first}");
    let last = tube.arm(Arm::Trailing).knots()[255].psi().value();
    assert!(last > 0.0, "the trailing arm ends at ψ {last}");
}

/// [`circle`]'s 300 tracers an arm made a cloud: the ten furthest behind take the leading arm's
/// energy and the ten furthest ahead the trailing arm's, so that each arm's ψ crosses zero and
/// reaches past the other arm's tip.
fn cloud() -> Vec<Tracer> {
    let mut tracers = circle(300, 0);
    let mut order: Vec<usize> = (0..tracers.len()).collect();
    order.sort_by(|&a, &b| {
        tracers[a]
            .psi()
            .value()
            .total_cmp(&tracers[b].psi().value())
    });
    for (&behind, &ahead) in order.iter().zip(order.iter().rev()).take(10) {
        tracers[behind] = with_energy(&tracers[behind], -ENERGY_OFFSET);
        tracers[ahead] = with_energy(&tracers[ahead], ENERGY_OFFSET);
    }
    tracers
}

/// The gap noise's span of ψ runs from the least of both arms' knots' ψ to the greatest: the
/// trailing arm's tip and the leading arm's where every tracer's ψ has its arm's sign, and where
/// an arm's ψ crosses zero and reaches past the other arm's tip, as in a cloud, that arm's end.
/// Every cell's middle lies in the span, and the noise still averages one.
#[test]
fn the_gap_noise_s_span_covers_both_arms() {
    let ends = |tube: &TubeTable, arm| {
        let knots = tube.arm(arm).knots();
        [knots[0].psi().value(), knots[knots.len() - 1].psi().value()]
    };
    let tube = table(&circle(300, 0));
    assert_eq!(
        tube.psi_span().map(Radians::value),
        [ends(&tube, Arm::Trailing)[0], ends(&tube, Arm::Leading)[1]]
    );
    let cloud = table(&cloud());
    let [low, high] = cloud.psi_span().map(Radians::value);
    let mut psis: Vec<f64> = cloud.knots().map(|k| k.psi().value()).collect();
    psis.sort_by(f64::total_cmp);
    assert_eq!([low, high], [psis[0], psis[psis.len() - 1]]);
    assert_eq!(
        [low, high],
        [
            ends(&cloud, Arm::Leading)[0],
            ends(&cloud, Arm::Trailing)[1]
        ]
    );
    let [inner_low, inner_high] = [
        ends(&cloud, Arm::Trailing)[0],
        ends(&cloud, Arm::Leading)[1],
    ];
    assert!(
        low < inner_low,
        "{low} against the trailing tip's {inner_low}"
    );
    assert!(
        inner_high < high,
        "{high} against the leading tip's {inner_high}"
    );
    let cells = cloud.along_cells();
    for cell in 0..cells {
        let psi = cloud
            .psi((f64::from(cell) + 0.5) / f64::from(cells))
            .value();
        assert!(low <= psi && psi <= high, "cell {cell} at ψ {psi}");
    }
    let sum = noises(&cloud).iter().sum::<f64>();
    assert!((sum / f64::from(cells) - 1.0).abs() < 1e-12, "{sum}");
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
    // Between the arms' roots each knot's tangent is its own one-sided fit's, so the normal turns
    // most there: by 3.0° (cos 0.99865) since the fit moved into the orbital plane, against 2.6°
    // allowed elsewhere. Not a flip, which would turn it by some 180°.
    let junction = usize::try_from(KNOTS_PER_ARM).expect("small") - 1;
    for (j, pair) in knots.windows(2).enumerate() {
        let [_, n0, _] = pair[0].frame();
        let [_, n1, _] = pair[1].frame();
        let least = if j == junction { 0.9985 } else { 0.999 };
        assert!(
            dot(n0, n1) > least,
            "knots {j}–{}: the normal turns by {}",
            j + 1,
            dot(n0, n1)
        );
    }
}

/// Tracers on an exact circle of [`RADIUS`] in the x–y plane, moving along it at [`SPEED`]: an
/// arm of `per_arm` each side of the progenitor, spread evenly over `arc` (rad) of it, with
/// [`MASS`] shared out.
fn arc(per_arm: u32, arc: f64) -> Vec<Tracer> {
    let weight = SolarMasses::new(MASS / f64::from(2 * per_arm));
    (0..2 * per_arm)
        .map(|i| {
            let k = i / 2;
            let (arm, side, energy) = if i.is_multiple_of(2) {
                (Arm::Leading, 1.0, -ENERGY_OFFSET)
            } else {
                (Arm::Trailing, -1.0, ENERGY_OFFSET)
            };
            let psi = side * arc * (f64::from(k) + 0.5) / f64::from(per_arm);
            let (sin, cos) = math::sin_cos(psi);
            Tracer::from_parts(
                arm,
                0,
                Years::new(1e9),
                weight,
                GalacticDisplacement::new(
                    [RADIUS * cos, RADIUS * sin, 0.0].map(|c| c * METRES_PER_LIGHT_YEAR),
                ),
                GalacticVelocity::new([-SPEED * sin, SPEED * cos, 0.0]),
                Radians::new(psi),
                energy,
            )
        })
        .collect()
}

/// `tracers` reduced, with the knots that kept the Cartesian fit while they were.
fn counted(tracers: &[Tracer]) -> (TubeTable, u64) {
    let before = cartesian_knots();
    let tube = table(tracers);
    (tube, cartesian_knots() - before)
}

/// P10.T6.a's tips: on a sparse arc, each arm's [`KERNEL_TRACERS`] over 1.2 rad (69°) of a
/// circle, so that every knot's kernel spans its whole arm, every knot's centre lies on the
/// circle and its mean velocity along it at the tracers' speed, the fit in the orbital plane being
/// exact there; in x, y and z the centre line was a chord, its middle knot some 9,000 ly inside.
#[test]
fn on_a_sparse_arc_every_knot_lies_on_the_arc() {
    let (tube, fallbacks) = counted(&arc(KERNEL_TRACERS, 1.2));
    assert_eq!(fallbacks, 0);
    for (j, knot) in tube.knots().enumerate() {
        let [x, y, z] = ly(&knot.centre());
        let radius = (x * x + y * y).sqrt();
        assert!(
            (radius / RADIUS - 1.0).abs() < 1e-9,
            "knot {j}: {radius} ly"
        );
        assert!(z.abs() < 1e-6, "knot {j}: {z} ly out of the plane");
        let velocity = knot.velocity().metres_per_second();
        assert!(
            (length(velocity) / SPEED - 1.0).abs() < 1e-9,
            "knot {j}: {velocity:?}"
        );
        assert!(
            dot(velocity, [x, y, 0.0]).abs() / (radius * SPEED) < 1e-9,
            "knot {j}: {velocity:?}"
        );
    }
}

/// P10.T6.a's fallback: where a kernel spans so much of the orbit that a tracer lies more than
/// three-eighths of a turn from the plane's first axis, near `atan2`'s cut, the knot keeps the
/// Cartesian fit; on arms of 300° every knot of both does.
#[test]
fn a_kernel_reaching_atan2_s_cut_keeps_the_cartesian_fit() {
    let (tube, fallbacks) = counted(&arc(KERNEL_TRACERS, 300_f64.to_radians()));
    assert_eq!(fallbacks, 2 * u64::from(KNOTS_PER_ARM));
    assert!(
        tube.knots()
            .all(|k| ly(&k.centre()).iter().all(|c| c.is_finite()))
    );
}

/// P10.T6.a's fallback: a kernel tracer nearer the pole's line than a tenth of the kernel's mean
/// radius leaves the angle ill-posed, so every knot of the arm it lies in keeps the Cartesian
/// fit, and none of the other arm's.
#[test]
fn a_tracer_near_the_pole_s_line_keeps_the_cartesian_fit() {
    let mut tracers = arc(KERNEL_TRACERS, 1.2);
    let t = &tracers[0];
    tracers[0] = Tracer::from_parts(
        t.arm(),
        t.release_step(),
        t.released(),
        t.weight(),
        GalacticDisplacement::new(t.position().metres().map(|c| c / 20.0)),
        t.velocity(),
        t.psi(),
        t.energy_offset_j_kg(),
    );
    let (_, fallbacks) = counted(&tracers);
    assert_eq!(fallbacks, u64::from(KNOTS_PER_ARM));
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
        let trailing = t.energy_arm() == Arm::Trailing;
        let keep = !trailing || kept < 47;
        if trailing && keep {
            kept += 1;
        }
        keep
    });
    assert_eq!(
        TubeTable::from_tracers(SEED, SolarMasses::new(MEAN_MASS), &spec(), &tracers),
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

/// Tracers of one arm and equal ψ keep their order in the list, the sort being stable: a spray's
/// last release can fall on the epoch's boundary, and its two tracers then both have ψ of +0.0,
/// in one arm if their offsets gave them energies of one sign. With such a tie the table is
/// unchanged when the other tracers are permuted, and changes when the tied pair is swapped.
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
            t.energy_offset_j_kg(),
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
                t.energy_offset_j_kg(),
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

/// The gap noise keeps the tube's expected total (P10.T6.b): its factors over the along cells,
/// summed in cell order, are the cell count to 10⁻¹², at the fewest cells, the most and between.
/// Each is positive and finite, and they vary as a log-normal of [`GAP_NOISE_AMPLITUDE`] does: the
/// rms of their logarithms over 512 cells, 64 node spacings, is 0.302 here.
#[test]
fn the_gap_noise_keeps_the_expected_total() {
    let tracers = circle(600, 3);
    for (mean_mass, cells) in [(1e3, 16), (MEAN_MASS, 512), (1e-3, 16_384)] {
        let tube = built(SEED, mean_mass, &spec(), &tracers);
        assert_eq!(tube.along_cells(), cells);
        let noise = noises(&tube);
        assert!(noise.iter().all(|f| *f > 0.0 && f.is_finite()));
        let sum = noise.iter().fold(0.0, |sum, f| sum + f);
        let n = f64::from(cells);
        assert!((sum / n - 1.0).abs() < 1e-12, "{cells} cells: {sum}");
        if cells == 512 {
            let logs: Vec<f64> = noise.iter().map(|f| math::ln(*f)).collect();
            let mean = logs.iter().sum::<f64>() / n;
            let rms = (logs.iter().map(|l| (l - mean) * (l - mean)).sum::<f64>() / n).sqrt();
            assert!(
                (0.5 * GAP_NOISE_AMPLITUDE..1.5 * GAP_NOISE_AMPLITUDE).contains(&rms),
                "{rms}"
            );
        }
    }
}

/// The gap noise is a pure function of the seed and the stream (P10.T6.b): the same seed and
/// stream number give the same bits from the tracers in any order and whatever cells, of whichever
/// table, were read before; another seed or another number gives other noise; and each cell's
/// factor is the module documentation's, bit for bit, from node `k`'s normal at word `2k` of
/// [`tags::STREAM_TUBE_GAPS`] keyed by the stream's number.
#[test]
fn the_gap_noise_is_a_pure_function_of_the_seed_and_the_stream() {
    let tracers = circle(300, 2);
    let tube = table(&tracers);
    let noise = noises(&tube);
    let reversed: Vec<Tracer> = tracers.iter().rev().copied().collect();
    assert_eq!(noises(&table(&reversed)), noise);
    let others = [
        built(Seed::new(0x0a10_6b01), MEAN_MASS, &spec(), &tracers),
        built(SEED, MEAN_MASS, &numbered(8), &tracers),
    ];
    let tubes = [&tube, &others[0], &others[1]];
    let keys: Vec<(usize, u32)> = (0..3)
        .flat_map(|t| {
            (0..tube.along_cells())
                .step_by(7)
                .map(move |cell| (t, cell))
        })
        .collect();
    hyperion_testkit::order::assert_order_independent(&keys, |&(t, cell)| tubes[t].noise(cell));
    for other in &others {
        assert_eq!(other.along_cells(), tube.along_cells());
        let moved = noises(other)
            .iter()
            .zip(&noise)
            .filter(|(a, b)| (*a / *b - 1.0).abs() > 1e-3)
            .count();
        assert!(moved > 400, "{moved} of 512 cells moved");
    }
    // By hand: the lattice from the trailing tip's ψ to the leading tip's, eight mean cells a node.
    let cells = tube.along_cells();
    let knots: Vec<&TubeKnot> = tube.knots().collect();
    let (low, high) = (knots[0].psi().value(), knots[511].psi().value());
    let spacing = 8.0 * (high - low) / f64::from(cells);
    let mut nodes = Stream::open(SEED, tags::STREAM_TUBE_GAPS, ObjectKey::galaxy_item(7));
    let normals: Vec<f64> = (0..=cells / 8)
        .map(|k| {
            nodes.seek(2 * u64::from(k));
            nodes.standard_normal()
        })
        .collect();
    let factors: Vec<f64> = (0..cells)
        .map(|cell| {
            let psi = tube.psi((f64::from(cell) + 0.5) / f64::from(cells)).value();
            let t = ((psi - low) / spacing).clamp(0.0, f64::from(cells / 8));
            let below = t.min(f64::from(cells / 8 - 1)).floor();
            let s = fade(t - below);
            let (w0, w1) = (1.0 - s, s);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "below is a whole number in [0, cells ÷ 8 − 1]"
            )]
            let k = below as usize;
            let g = (w0 * normals[k] + w1 * normals[k + 1]) / (w0 * w0 + w1 * w1).sqrt();
            math::exp(GAP_NOISE_AMPLITUDE * g - 0.5 * GAP_NOISE_AMPLITUDE * GAP_NOISE_AMPLITUDE)
        })
        .collect();
    let mean = factors.iter().fold(0.0, |sum, f| sum + f) / f64::from(cells);
    for (by_hand, f) in factors.iter().zip(&noise) {
        hyperion_testkit::float::assert_same_bits(by_hand / mean, *f);
    }
}

/// A tube's along cells are the power of two nearest its mass over 64 mean systems, nearest in the
/// logarithm (Design note 7), held to 16 (this task's lean) and 16,384 (the note's) (P10.T6.b).
#[test]
fn the_along_cells_follow_the_rule_and_its_clamp() {
    let cells = |ideal: f64| along_cells(SolarMasses::new(64.0 * ideal), SolarMasses::new(1.0));
    for (ideal, expected) in [
        (0.0, 16),
        (1.0, 16),
        (22.0, 16),
        (23.0, 32),
        (200.0, 256),
        (362.0, 256),
        (363.0, 512),
        (11_585.0, 8_192),
        (11_586.0, 16_384),
        (1e9, 16_384),
    ] {
        assert_eq!(cells(ideal), expected, "{ideal}");
    }
    // At each geometric middle the count rounds up.
    for log2 in 4..14 {
        let middle = f64::from(1_u32 << log2) * core::f64::consts::SQRT_2;
        assert_eq!(cells(middle), 2 << log2);
        assert_eq!(cells(middle * (1.0 - 1e-12)), 1 << log2);
    }
    // The synthetic stream: 2 × 10⁴ M☉ over 0.55 × 64 is 568, so 512.
    assert_eq!(table(&circle(600, 0)).along_cells(), 512);
}

/// A mean system mass that is not positive cannot cut a tube.
#[test]
#[should_panic(expected = "a galaxy's mean system mass is positive and finite: 0")]
fn a_mean_system_mass_of_zero_is_a_broken_invariant() {
    let _ = along_cells(SolarMasses::new(1e4), SolarMasses::new(0.0));
}

/// P10.T8.c: the frame and the dispersions at a knot's along coordinate are the knot's, and
/// between knots the dispersions are linear and the frame's vectors unit vectors.
#[test]
fn the_frame_and_the_dispersions_at_an_along_coordinate_are_the_knots() {
    let tube = table(&circle(300, 0));
    let total = tube.mass().value();
    for k in tube.arm(Arm::Leading).knots().iter().skip(1).step_by(37) {
        let u = k.cumulative_mass().value() / total;
        let frame = tube.frame_at(u);
        for (got, want) in frame.iter().zip(k.frame()) {
            for i in 0..3 {
                assert!((got[i] - want[i]).abs() < 1e-12);
            }
        }
        for (got, want) in tube.dispersions_at(u).iter().zip(k.dispersions()) {
            assert!((got.value() / want.value() - 1.0).abs() < 1e-12);
        }
    }
    for u in [0.13, 0.5, 0.77] {
        for v in tube.frame_at(u) {
            assert!((length(v) - 1.0).abs() < 1e-12);
        }
    }
}

/// A knot dropped its stored binormal (P10.T6.c): it is the tangent × the normal, recomputed to
/// the bits the frames were built with, so that a knot is 168 bytes on every target.
#[test]
fn a_knot_keeps_two_frame_vectors_in_168_bytes() {
    assert_eq!(size_of::<TubeKnot>(), 168);
    let tracers = circle(300, 2);
    let tube = table(&tracers);
    // The frames as `from_tracers` builds them, from the arms by the energy's sign.
    let [trailing, leading] = [Arm::Trailing, Arm::Leading].map(|arm| {
        let mut on: Vec<&Tracer> = tracers.iter().filter(|t| t.energy_arm() == arm).collect();
        on.sort_by(|a, b| a.psi().value().total_cmp(&b.psi().value()));
        on
    });
    let built = frames(&[Reduced::new(&trailing), Reduced::new(&leading)]);
    let read: Vec<[[f64; 3]; 3]> = tube.knots().map(TubeKnot::frame).collect();
    assert_eq!(read, built.concat());
}

/// Spheres about the synthetic tube and beyond it, of radii from 1 ly to beyond the tube's span.
fn spheres(tube: &TubeTable, n: u32) -> Vec<(GalacticPosition, f64)> {
    let mut draws = Stream::open(
        Seed::new(0x0a10_6c01),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy(),
    );
    (0..n)
        .map(|_| {
            let (u, a, b) = (
                draws.uniform(),
                12.0 * draws.uniform() - 6.0,
                12.0 * draws.uniform() - 6.0,
            );
            let radius = math::powf(10.0, 5.5 * draws.uniform());
            (tube.locate(u, a, b), radius)
        })
        .collect()
}

/// The scan of the segments' padded boxes finds every segment whose reach meets a sphere, as a
/// test of each segment's distance alone does, in the order of the along coordinate, for spheres
/// from 1 ly to 3 × 10⁵ ly, the largest spanning the tube.
#[test]
fn the_scan_finds_every_segment_a_sphere_reaches() {
    let tube = table(&circle(600, 0));
    let mut met = 0;
    for (centre, radius) in spheres(&tube, 400) {
        let near: Vec<f64> = tube
            .segments_near(&centre, radius)
            .map(|near| near.knots[0].cumulative_mass.value())
            .collect();
        assert!(near.windows(2).all(|w| w[0] < w[1]), "ascending and unique");
        let reached: Vec<f64> = tube
            .arms
            .iter()
            .flat_map(|arm| arm.knots.windows(2).zip(&arm.reaches_ly))
            .filter(|(pair, reach)| {
                let (q, d) = (
                    pair[0].centre.displacement_to(&centre).metres(),
                    pair[0].centre.displacement_to(&pair[1].centre).metres(),
                );
                distance_to_segment(q, d) <= (radius + **reach + MARGIN_LY) * METRES_PER_LIGHT_YEAR
            })
            .map(|(pair, _)| pair[0].cumulative_mass.value())
            .collect();
        assert_eq!(near, reached, "{radius} ly");
        met += usize::from(!near.is_empty());
    }
    assert!(met > 300, "{met} of 400 spheres meet the tube");
    assert_eq!(
        tube.arms
            .iter()
            .map(|arm| arm.reaches_ly.len())
            .sum::<usize>(),
        510
    );
}

/// Every point within the cut lies within its segment's reach of the segment's centre line, the
/// premise of the scan of the segments, at the cut's corners too.
#[test]
fn every_point_within_the_cut_lies_within_its_segment_s_reach() {
    let tube = table(&circle(300, 0));
    let mut draws = Stream::open(
        Seed::new(0x0a10_6c02),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy(),
    );
    for i in 0..20_000_u32 {
        let u = draws.uniform();
        let (a, b) = if i % 4 == 0 {
            (CUT_WIDTHS, -CUT_WIDTHS)
        } else {
            (8.0 * draws.uniform() - 4.0, 8.0 * draws.uniform() - 4.0)
        };
        let point = tube.locate(u, a, b);
        let (from, to, _) = tube.segment(u);
        let reach = segment_reach_ly(from, to) * METRES_PER_LIGHT_YEAR;
        let off = distance_to_segment(
            from.centre.displacement_to(&point).metres(),
            from.centre.displacement_to(&to.centre).metres(),
        );
        assert!(off <= reach, "{u}, {a}, {b}: {off} against {reach}");
    }
}

/// The tube cell of the point at `(u, a, b)` in a tube of `cells` along cells.
fn cell_of(u: f64, [a, b]: [f64; 2], cells: u32) -> TubeCell {
    let index = |c: f64| cell_u8(across_span(c, c, 1.0).expect("within the cut")[0]);
    TubeCell {
        along: along_index(u * f64::from(cells), cells),
        across: [index(a), index(b)],
    }
}

/// `cells_touching` finds every cell holding a point of the tube within the sphere: for spheres
/// on and off the tube of radii from 1 ly to 30,000 ly, points drawn within the cut at along
/// coordinates about the sphere's, each inside the sphere in its cell. Its cells are ascending and
/// unique, and a sphere far from the tube finds none.
#[test]
fn cells_touching_finds_every_cell_holding_a_point_in_the_sphere() {
    let tube = table(&circle(600, 2));
    let cells = tube.along_cells();
    let mut draws = Stream::open(
        Seed::new(0x0a10_6c03),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy(),
    );
    let (mut inside, mut found) = (0_u32, 0_usize);
    for _ in 0..60 {
        let centre_u = draws.uniform();
        let across = [12.0 * draws.uniform() - 6.0, 12.0 * draws.uniform() - 6.0];
        let radius = math::powf(10.0, 4.5 * draws.uniform());
        let centre = tube.locate(centre_u, across[0], across[1]);
        let touched: Vec<TubeCell> = tube
            .cells_touching(&centre, LightYears::new(radius))
            .collect();
        assert!(
            touched.windows(2).all(|w| w[0] < w[1]),
            "ascending and unique"
        );
        found += touched.len();
        // The tube is 100,000 ly long, so a sphere's along coordinates lie within its radius plus
        // the cut's 400 ly over 10⁵ of the centre's.
        let reach = ((radius + 1_000.0) / 1e5).min(1.0);
        for _ in 0..4_000 {
            let u = (centre_u + reach * (2.0 * draws.uniform() - 1.0)).clamp(0.0, 1.0 - 1e-12);
            let ab = [8.0 * draws.uniform() - 4.0, 8.0 * draws.uniform() - 4.0];
            let point = tube.locate(u, ab[0], ab[1]);
            if point.distance_to(&centre).value() <= radius * METRES_PER_LIGHT_YEAR {
                inside += 1;
                let cell = cell_of(u, ab, cells);
                assert!(
                    touched.binary_search(&cell).is_ok(),
                    "{cell:?} of ({u}, {ab:?}) within {radius} ly of ({centre_u}, {across:?})"
                );
            }
        }
    }
    assert!(inside > 10_000, "{inside} points drawn inside the spheres");
    assert!(found > 0);
    let far = GalacticPosition::from_light_years([0.0, 0.0, 30_000.0]).expect("in range");
    assert_eq!(
        tube.cells_touching(&far, LightYears::new(1_000.0)).count(),
        0
    );
}

/// A sphere on the axis finds a cross-section's middle, not the whole of it: of radius 10 ly,
/// a tenth of a width, about the tube's middle, a few along cells and some 4 × 4 across cells
/// each.
#[test]
fn a_small_sphere_on_the_axis_touches_a_few_cells() {
    let tube = table(&circle(600, 0));
    let centre = tube.locate(0.25, 0.0, 0.0);
    let touched: Vec<TubeCell> = tube
        .cells_touching(&centre, LightYears::new(10.0))
        .collect();
    let along: Vec<u32> = touched.iter().map(|c| c.along()).collect();
    assert!(
        along.first() >= Some(&127) && along.last() <= Some(&129),
        "{along:?}"
    );
    assert!(touched.len() <= 80, "{} cells", touched.len());
    for cell in &touched {
        for j in cell.across() {
            assert!((28..36).contains(&j), "{cell:?}");
        }
    }
}

/// A cell's across coordinates are an eighth of a width each, from −4 to +4 widths.
#[test]
fn a_tube_cell_spans_an_eighth_of_a_width_across() {
    let cell = TubeCell {
        along: 3,
        across: [0, 63],
    };
    assert_eq!(cell.across_widths(), [[-4.0, -3.875], [3.875, 4.0]]);
    assert_eq!(
        TubeCell {
            along: 0,
            across: [32, 31]
        }
        .across_widths(),
        [[0.0, 0.125], [-0.125, 0.0]]
    );
    let id = crate::id::StreamMemberId::new(5, crate::id::Layer::C, 16_383, [63, 7], 12)
        .expect("fields in range");
    assert_eq!(
        TubeCell::from(id),
        TubeCell {
            along: 16_383,
            across: [63, 7]
        }
    );
    assert_eq!(across_span(-1.0, 1.0, 0.0), Some([0, 63]));
    assert_eq!(across_span(1.0, 2.0, 0.0), None);
    assert_eq!(across_span(-100.0, -50.0, 10.0), None);
    assert_eq!(across_span(-0.05, 0.05, 1.0), Some([31, 32]));
}

/// `byte_size` counts the table, its knots and its segments' reaches: 512 knots of 168 bytes and
/// 510 reaches of 8, 90,096 bytes beyond the table's own.
#[test]
fn byte_size_counts_the_knots_and_the_reaches() {
    let tube = table(&circle(600, 0));
    assert_eq!(
        tube.byte_size(),
        size_of::<TubeTable>() + 512 * 168 + 510 * 8
    );
}

/// A sphere's radius is finite and not negative.
#[test]
#[should_panic(expected = "a sphere's radius is finite and not negative: -1")]
fn a_negative_radius_is_a_broken_invariant() {
    let tube = table(&circle(300, 0));
    let _ = tube
        .cells_touching(&tube.locate(0.5, 0.0, 0.0), LightYears::new(-1.0))
        .count();
}

/// The three test streams' tubes on the Milky Way fixture, at 64 tracers an arm: the golden file
/// and order independence (P10.T6.c). Here rather than in an integration test, so that the sim's
/// fast suite compares the golden on `wasm32-wasip1` too, where `global_list::testing`'s feature
/// is off.
mod milky_way {
    use std::num::NonZeroU32;
    use std::sync::OnceLock;

    use hyperion_testkit::golden;
    use hyperion_testkit::golden::{GoldenWriter, f64_digest};
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::galaxy::global_list::spray::{Spray, SprayForce, SprayParams};
    use crate::galaxy::global_list::testing::{gd1_like, pal5_like, sagittarius_like};
    use crate::galaxy::potential::{MassModel, PotentialTables};

    const SEED: Seed = Seed::new(0x0a10_6c00_0000_0000);

    /// Tracers an arm: 64, past a kernel's 48 with room for the strays (P10.T6.a, as built).
    const TRACERS_PER_ARM: u32 = 64;

    /// The streams' names, specifications and tracers, and the fixture halo's mean system mass;
    /// and [`EPOCH_PER_ARM`] tracers an arm of `pal5_like` stripped over 20 Myr, a few hundred
    /// steps, so that its last release falls on the epoch's boundary (P10.T6.a).
    struct Sprayed {
        mean_mass: SolarMasses,
        streams: Vec<(&'static str, StreamSpec, Vec<Tracer>)>,
        epoch: (StreamSpec, Vec<Tracer>),
    }

    /// The epoch's stream's tracers an arm: more than twice its steps, so that its last stratum
    /// lies within half a step of the epoch.
    const EPOCH_PER_ARM: u32 = 1_000;

    fn sprayed() -> &'static Sprayed {
        static SPRAYED: OnceLock<Sprayed> = OnceLock::new();
        SPRAYED.get_or_init(|| {
            let params = GalaxyParams::milky_way_like();
            let tables = PotentialTables::full(&MassModel::new(&params));
            let force = SprayForce::new(&tables).expect("the full tables");
            let spray = SprayParams::new(NonZeroU32::new(TRACERS_PER_ARM).expect("not zero"));
            let streams = [
                ("pal5", pal5_like(&tables)),
                ("gd1", gd1_like(&tables)),
                ("sagittarius", sagittarius_like(&tables)),
            ]
            .into_iter()
            .map(|(name, spec)| {
                let tracers = Spray::new(SEED, &force, &tables, &spec, &spray)
                    .expect("a finite stripping time")
                    .tracers();
                (name, spec, tracers)
            })
            .collect();
            let epoch = StreamSpec {
                stripping_time: Years::new(2e7),
                ..pal5_like(&tables)
            };
            let per_arm = SprayParams::new(NonZeroU32::new(EPOCH_PER_ARM).expect("not zero"));
            let sprayed = Spray::new(SEED, &force, &tables, &epoch, &per_arm)
                .expect("a finite stripping time");
            let steps = sprayed.schedule().step().count();
            assert!(2 * steps < EPOCH_PER_ARM, "{steps} steps");
            let last = sprayed.schedule().releases()[index(EPOCH_PER_ARM - 1)];
            assert_eq!(
                last.step(),
                steps,
                "the last release is on the epoch's boundary"
            );
            Sprayed {
                mean_mass: params.mean_system_mass(crate::galaxy::Population::Halo),
                streams,
                epoch: (epoch, sprayed.tracers()),
            }
        })
    }

    fn tube(stream: usize) -> TubeTable {
        let Sprayed {
            mean_mass, streams, ..
        } = sprayed();
        let (_, spec, tracers) = &streams[stream];
        TubeTable::from_tracers(SEED, *mean_mass, spec, tracers).expect("enough tracers an arm")
    }

    /// The sphere the golden and the order check ask about: half a width about a point of the
    /// tube's middle, off its axis.
    fn touched(tube: &TubeTable) -> Vec<TubeCell> {
        let centre = tube.locate(0.5, 1.5, -0.5);
        let width = tube.knots().nth(256).expect("512 knots").widths()[0];
        tube.cells_touching(&centre, width * 0.5).collect()
    }

    /// Every stored quantity of a knot, in a fixed order: the centre's light-year cell and
    /// offset, the tangent and the normal and the binormal read from them, the velocity, the
    /// widths, the dispersions, Λ and ψ.
    fn values(k: &TubeKnot) -> Vec<f64> {
        let cell = k.centre().cell().to_array().map(f64::from);
        let mut v = cell.to_vec();
        v.extend(k.centre().offset_metres());
        v.extend(k.frame().into_iter().flatten());
        v.extend(k.velocity().metres_per_second());
        v.extend(k.widths().map(LightYears::value));
        v.extend(k.dispersions().map(MetresPerSecond::value));
        v.extend([k.cumulative_mass().value(), k.psi().value()]);
        v
    }

    /// The golden file first, so that a failing check does not keep `just bless` from writing
    /// it, then order independence.
    #[test]
    fn the_test_streams_tubes_are_pinned_and_order_independent() {
        golden_tubes();
        tubes_are_order_independent();
    }

    /// The bits of each test stream's tube at 64 tracers an arm: its counts, its mass and length,
    /// a digest of every knot's quantities and the arms' ends and roots in full, the along cells
    /// and four cells' gap noise, which no other golden pins (P10.T6.b, as built), and the cells
    /// one sphere touches. They depend on the spray and the reduction, so a change to the
    /// potential, the step rule, the release, the kernel, the frame, the noise or the cells' test
    /// moves them; every architecture the sim's tests run on must reproduce them.
    fn golden_tubes() {
        let mut writer = GoldenWriter::new();
        writer.header(crate::GENERATOR_VERSION.get());
        for (stream, (name, _, _)) in sprayed().streams.iter().enumerate() {
            let before = cartesian_knots();
            let tube = tube(stream);
            // The knots whose centre line kept the Cartesian fit (P10.T6.a's tips), so that a knot
            // moving onto or off the fallback shows by name.
            writer.line(&format!(
                "{name}.cartesian_knots {}",
                cartesian_knots() - before
            ));
            let [trailing, leading] = [Arm::Trailing, Arm::Leading].map(|arm| tube.arm(arm));
            writer.line(&format!(
                "{name}.tracers {} {}",
                trailing.tracers(),
                leading.tracers()
            ));
            writer.f64(&format!("{name}.mass"), tube.mass().value());
            writer.f64(&format!("{name}.trailing.mass"), trailing.mass().value());
            writer.f64(&format!("{name}.length"), tube.length().value());
            let all: Vec<f64> = tube.knots().flat_map(values).collect();
            writer.u64_hex(&format!("{name}.knots.digest"), f64_digest(&all));
            for k in [0, 255, 256, 511] {
                let knot = tube.knots().nth(k).expect("512 knots");
                let [x, y, z] = knot.centre().to_light_years_f64();
                for (axis, value) in ["x", "y", "z"].iter().zip([x, y, z]) {
                    writer.f64(&format!("{name}.knot.{k}.centre.{axis}"), value);
                }
                let [normal, binormal] = knot.widths();
                writer.f64(&format!("{name}.knot.{k}.width.normal"), normal.value());
                writer.f64(&format!("{name}.knot.{k}.width.binormal"), binormal.value());
                writer.f64(&format!("{name}.knot.{k}.psi"), knot.psi().value());
            }
            let cells = tube.along_cells();
            writer.line(&format!("{name}.along_cells {cells}"));
            for cell in [0, cells / 3, cells / 2, cells - 1] {
                writer.f64(&format!("{name}.noise.{cell}"), tube.noise(cell));
            }
            let touched = touched(&tube);
            let flat: Vec<f64> = touched
                .iter()
                .flat_map(|c| {
                    let [a, b] = c.across();
                    [f64::from(c.along()), f64::from(a), f64::from(b)]
                })
                .collect();
            writer.line(&format!("{name}.touched {}", touched.len()));
            writer.u64_hex(&format!("{name}.touched.digest"), f64_digest(&flat));
        }
        golden!("galaxy/global_list/tube", writer.as_str());
    }

    /// A tube is a pure function of its seed, its mass, its stream and its tracers, and so are
    /// its gap noise and the cells a sphere touches: the three tubes in any order give the same
    /// bits.
    fn tubes_are_order_independent() {
        assert_order_independent(&[0, 1, 2], |&stream| {
            let tube = tube(stream);
            let noise: Vec<f64> = (0..tube.along_cells()).map(|c| tube.noise(c)).collect();
            let touched = touched(&tube);
            (tube, noise, touched)
        });
    }

    /// P10.T6.a's golden (main's ruling of 2026-10-30): the bits of two tubes whose arms by the
    /// energy's sign are not ψ's, which `tube.golden`'s test streams never reach. One is the
    /// synthetic [`cloud`](super::cloud), whose arms each cross zero and reach past the other's
    /// tip. The other is `pal5_like` stripped over 20 Myr, whose last release leaves a tracer of ψ
    /// +0.0 at the trailing arm's root. Each gives its arms' counts and masses, the gap noise's
    /// span, a digest of every knot's quantities, its arms' ends' ψ and four cells' noise; every
    /// architecture the sim's tests run on must reproduce them.
    #[test]
    fn tubes_whose_arms_cross_zero_are_pinned() {
        let Sprayed {
            mean_mass, epoch, ..
        } = sprayed();
        let cloud = super::cloud();
        let mut writer = GoldenWriter::new();
        writer.header(crate::GENERATOR_VERSION.get());
        for (name, tube) in [
            ("cloud", super::table(&cloud)),
            (
                "epoch",
                TubeTable::from_tracers(SEED, *mean_mass, &epoch.0, &epoch.1)
                    .expect("enough tracers an arm"),
            ),
        ] {
            let [trailing, leading] = [Arm::Trailing, Arm::Leading].map(|arm| tube.arm(arm));
            writer.line(&format!(
                "{name}.tracers {} {}",
                trailing.tracers(),
                leading.tracers()
            ));
            writer.f64(&format!("{name}.mass"), tube.mass().value());
            writer.f64(&format!("{name}.trailing.mass"), trailing.mass().value());
            let [low, high] = tube.psi_span();
            writer.f64(&format!("{name}.span.low"), low.value());
            writer.f64(&format!("{name}.span.high"), high.value());
            let all: Vec<f64> = tube.knots().flat_map(values).collect();
            writer.u64_hex(&format!("{name}.knots.digest"), f64_digest(&all));
            for (arm, knots) in [("trailing", trailing), ("leading", leading)] {
                let knots = knots.knots();
                writer.f64(&format!("{name}.{arm}.first.psi"), knots[0].psi().value());
                writer.f64(
                    &format!("{name}.{arm}.last.psi"),
                    knots[knots.len() - 1].psi().value(),
                );
            }
            let cells = tube.along_cells();
            writer.line(&format!("{name}.along_cells {cells}"));
            for cell in [0, cells / 3, cells / 2, cells - 1] {
                writer.f64(&format!("{name}.noise.{cell}"), tube.noise(cell));
            }
        }
        golden!("galaxy/global_list/tube_arms", writer.as_str());
    }
}
