//! The spray's unit tests: the release schedule (P10.T5.a) on Kepler orbits, and the table, the
//! tracers and the golden file (P10.T5.b) on the Milky Way fixture.

use super::*;
use crate::coords::{GalacticDisplacement, GalacticVelocity};
use crate::galaxy::global_list::spec::SphericalOrbit;
use crate::galaxy::global_list::{StreamNumber, StreamOrigin};
use crate::units::{LightYears, SolarMassesPerYear, Years};

/// G M in (km/s)² ly for 10¹¹ M☉, as `spec`'s tests take it.
const GM: f64 = 4.300_9e-3 * 3.261_563_777 * 1e11;

const SEED: Seed = Seed::new(0x0a10_5a00_0000_0000);

/// A stream on a Kepler orbit between 20,000 and 60,000 ly, stripped over 3 Gyr, of 2 × 10⁴
/// M☉ lost by a progenitor of `progenitor` M☉ as `mass_loss` says.
fn stream(number: u16, progenitor: f64, mass_loss: MassLoss) -> StreamSpec {
    let orbit = SphericalOrbit::from_turning_points(|r| -GM / r, 20_000.0, 60_000.0)
        .expect("Kepler turning points bound an orbit");
    StreamSpec {
        number: StreamNumber::new(number).expect("below the limit"),
        origin: StreamOrigin::Orphan,
        position: GalacticDisplacement::new([0.0; 3]),
        velocity: GalacticVelocity::new([0.0; 3]),
        orbit,
        stripping_time: Years::new(3e9),
        mass: SolarMasses::new(2e4),
        progenitor_mass: SolarMasses::new(progenitor),
        tidal_radius: LightYears::new(100.0),
        mass_loss,
        stars: None,
    }
}

fn steady() -> StreamSpec {
    stream(3, 3e4, MassLoss::Steady(SolarMassesPerYear::new(2e4 / 3e9)))
}

fn orphan() -> StreamSpec {
    stream(
        4,
        0.0,
        MassLoss::Dissolved {
            ago: Years::new(1.2e9),
        },
    )
}

/// A dwarf of 5 × 10⁵ M☉ left in its core after three pulses of 30% each: the tube holds
/// `5 × 10⁵ × (0.7⁻³ − 1)` M☉.
fn dwarf() -> StreamSpec {
    let core = 5e5;
    StreamSpec {
        mass: SolarMasses::new(core * (math::powi(0.7, -3) - 1.0)),
        ..stream(
            5,
            core,
            MassLoss::Pulsed {
                fraction: 0.3,
                last_pericentre: Years::new(2e8),
                pericentres: 3,
            },
        )
    }
}

fn schedule(spec: &StreamSpec, pericentres: &[u32]) -> ReleaseSchedule {
    ReleaseSchedule::new(SEED, spec, &SprayParams::default(), pericentres)
        .expect("a finite stripping time")
}

/// P10.T5.a: the weights sum to the tube's mass, for each kind of mass loss, and each release
/// frees two tracers of half its stratum's mass.
#[test]
fn weights_sum_to_the_tube_s_mass() {
    let pericentres = [100, 2_000, 4_000, 6_000];
    for spec in [steady(), orphan(), dwarf()] {
        let schedule = schedule(&spec, &pericentres);
        let releases = schedule.releases();
        assert_eq!(releases.len(), 1_000);
        let total = schedule.total_weight().value();
        assert!(
            (total / spec.mass().value() - 1.0).abs() < 1e-12,
            "{:?}: {total} against {}",
            spec.mass_loss(),
            spec.mass().value()
        );
        let each = spec.mass().value() / 2_000.0;
        for r in releases {
            assert!((r.weight().value() / each - 1.0).abs() < 1e-15);
        }
    }
}

/// P10.T5.a: an orphan releases nothing after its dissolution, 1.2 Gyr before the epoch, and
/// its releases spread over the 1.8 Gyr before it.
#[test]
fn an_orphan_releases_nothing_after_its_dissolution() {
    let spec = orphan();
    let schedule = schedule(&spec, &[]);
    let step = schedule.step();
    let h = Years::from(step.step()).value();
    let dissolved = 3e9 - 1.2e9;
    let latest = schedule
        .releases()
        .iter()
        .map(Release::step)
        .max()
        .expect("a thousand releases");
    assert!(
        f64::from(latest) * h <= dissolved,
        "a release {} Gyr after the start, past the dissolution at {} Gyr",
        f64::from(latest) * h / 1e9,
        dissolved / 1e9
    );
    // The last stratum ends at the dissolution: the latest release lies within a stratum of
    // it, 1.8 Gyr ÷ 1,000.
    assert!(f64::from(latest) * h > dissolved - 2.0 * 1.8e6 - h);
    // Nothing is bound after it, and its cluster shrinks to nothing as it dissolves.
    assert!(schedule.bound_mass(step.count()).value().abs() < 1e-9);
    let last = schedule.releases().last().expect("a thousand releases");
    assert!(last.bound_mass().value() < 2e4 / 1_000.0);
    assert!((schedule.bound_mass(0).value() / 2e4 - 1.0).abs() < 1e-12);
}

/// A living globular releases at a constant rate: one release in each thousandth of the
/// stripping time, in time order, with the bound mass falling from `m₀ + M` to `m₀`.
#[test]
fn a_steady_stream_releases_evenly_and_its_mass_falls_to_the_present() {
    let spec = steady();
    let schedule = schedule(&spec, &[]);
    let steps = f64::from(schedule.step().count());
    for (i, r) in (0_u32..).zip(schedule.releases()) {
        let (from, to) = (
            f64::from(i) / 1_000.0 * steps,
            f64::from(i + 1) / 1_000.0 * steps,
        );
        let at = f64::from(r.step());
        assert!(at >= from - 0.5 && at <= to + 0.5, "release {i} at {at}");
        let share = 1.0 - (r.bound_mass().value() - 3e4) / 2e4;
        assert!(
            share * steps >= from - 1e-6 && share * steps <= to + 1e-6,
            "release {i}"
        );
    }
    assert!((schedule.bound_mass(0).value() - 5e4).abs() < 1e-9);
    assert!((schedule.bound_mass(schedule.step().count()).value() - 3e4).abs() < 1e-9);
    let masses: Vec<f64> = (0..=schedule.step().count())
        .map(|s| schedule.bound_mass(s).value())
        .collect();
    assert!(masses.windows(2).all(|w| w[1] <= w[0]));
}

/// A dwarf's pulses fall on the integrated orbit's pericentres, the specification's count of
/// the most recent, each with its share of the tube: `0.7⁻³ : 0.7⁻² : 0.7⁻¹` of the core's
/// mass, oldest first. With none inside the stripping time, the spherical approximation's
/// stand in.
#[test]
fn a_dwarf_releases_in_pulses_at_its_pericentres() {
    let spec = dwarf();
    let integrated = [100, 2_000, 4_000, 6_000];
    let schedule = schedule(&spec, &integrated);
    let count = |at: u32| {
        schedule
            .releases()
            .iter()
            .filter(|r| r.step() == at)
            .count()
    };
    assert_eq!(count(100), 0, "the oldest pericentre is beyond the count");
    let weights = [
        math::powi(0.7, -3),
        math::powi(0.7, -2),
        math::powi(0.7, -1),
    ];
    let sum: f64 = weights.iter().sum();
    let mut released = 0;
    for (at, w) in [2_000, 4_000, 6_000].into_iter().zip(weights) {
        let n = count(at);
        released += n;
        // Stratified: within one release of its share of the thousand.
        let expected = 1_000.0 * w / sum;
        assert!(
            (f64::from(u32::try_from(n).unwrap()) - expected).abs() <= 1.0,
            "{n} releases at {at} against {expected}"
        );
    }
    assert_eq!(released, 1_000);
    // The bound mass steps down at each pulse to the core's at the last.
    let core = spec.progenitor_mass().value();
    let tube = spec.mass().value();
    assert!((schedule.bound_mass(1_999).value() / (core + tube) - 1.0).abs() < 1e-12);
    assert!((schedule.bound_mass(6_000).value() / core - 1.0).abs() < 1e-12);
    // Before the last pulse the core held its mass over 0.7.
    let after_two = core / 0.7;
    assert!(schedule.bound_mass(4_000).value() > core);
    assert!((schedule.bound_mass(4_000).value() / after_two - 1.0).abs() < 1e-12);

    // Pericentres beyond the grid only: the spherical approximation's, 0.2 Gyr before the
    // epoch and one radial period apart, three of them inside 3 Gyr.
    let fallback = schedule_spherical(&spec);
    let period = spec.orbit().radial_period().value();
    let steps = f64::from(fallback.step().count());
    for k in 0..3_u32 {
        let ago = 2e8 + f64::from(k) * period;
        let at = (steps * (1.0 - ago / 3e9)).round();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a rounded step count in 0..=steps, far inside u32"
        )]
        let at = at as u32;
        assert!(
            fallback.releases().iter().any(|r| r.step() == at),
            "no pulse at {at}"
        );
    }
}

fn schedule_spherical(spec: &StreamSpec) -> ReleaseSchedule {
    schedule(spec, &[u32::MAX])
}

/// The same seed and stream give the same schedule; another stream number draws another.
#[test]
fn the_same_stream_gives_the_same_schedule() {
    let spec = steady();
    assert_eq!(schedule(&spec, &[]), schedule(&spec, &[]));
    let other = StreamSpec {
        number: StreamNumber::new(9).unwrap(),
        ..steady()
    };
    assert_ne!(schedule(&spec, &[]), schedule(&other, &[]));
    let seed = ReleaseSchedule::new(Seed::new(1), &spec, &SprayParams::default(), &[]).unwrap();
    assert_ne!(schedule(&spec, &[]), seed);
}

/// The release offsets' Gaussians, cut at ±2.5 dispersions and widened by `cut_widening`, keep
/// the dispersion they were fitted with (main's ruling of 2026-10-29): the cut distribution's
/// variance by a stratified midpoint rule over its distribution function, the draw's own
/// inverse transform, is the widening's inverse square to 10⁻⁸ (the rule's own error is 2.6 ×
/// 10⁻⁹). The spray's own draws are checked on the Milky Way fixture below.
#[test]
fn the_widening_restores_a_cut_gaussian_s_variance() {
    let widening = cut_widening();
    assert!((widening - 1.0476).abs() < 5e-5, "{widening}");
    let below = 0.5 * math::erfc(OFFSET_CUT * core::f64::consts::FRAC_1_SQRT_2);
    let between = 1.0 - 2.0 * below;
    let n = 100_000_u32;
    let variance = (0..n)
        .map(|i| {
            let q = math::normal_quantile(below + (f64::from(i) + 0.5) / f64::from(n) * between);
            q * q
        })
        .sum::<f64>()
        / f64::from(n);
    assert!(
        (variance * widening * widening - 1.0).abs() < 1e-8,
        "{variance} against {}",
        1.0 / (widening * widening)
    );
}

/// A spray is shared by reference between a pool's threads (ruling 146.3).
#[test]
fn a_spray_and_its_table_are_shared_between_threads() {
    fn shared<T: Sync + Send>() {}
    shared::<Spray<'static>>();
    shared::<SprayForce>();
}

/// The Milky Way fixture's full tables and their spray table, built once for the checks below.
mod milky_way {
    use std::sync::OnceLock;

    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::galaxy::global_list::orbit::Leapfrog;
    use crate::galaxy::global_list::testing::{gd1_like, pal5_like, sagittarius_like};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::potential::MassModel;
    use crate::units::Megayears;

    const SEED: Seed = Seed::new(0x0a10_5b00_0000_0000);

    struct Fixture {
        tables: PotentialTables,
        force: SprayForce,
    }

    fn fixture() -> &'static Fixture {
        static FIXTURE: OnceLock<Fixture> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let tables = PotentialTables::full(&MassModel::new(&GalaxyParams::milky_way_like()));
            let force = SprayForce::new(&tables).expect("the full tables");
            Fixture { tables, force }
        })
    }

    /// The three test streams, by name, in the fixture's potential.
    fn test_streams() -> [(&'static str, StreamSpec); 3] {
        let tables = &fixture().tables;
        [
            ("pal5", pal5_like(tables)),
            ("gd1", gd1_like(tables)),
            ("sagittarius", sagittarius_like(tables)),
        ]
    }

    /// `spec`'s spray in the fixture, of `n` tracers an arm, in the galaxy of `seed`.
    fn spray(seed: Seed, spec: &StreamSpec, n: u32) -> Spray<'static> {
        let f = fixture();
        let params = SprayParams::new(NonZeroU32::new(n).expect("not zero"));
        Spray::new(seed, &f.force, &f.tables, spec, &params).expect("a finite stripping time")
    }

    /// Light-years to metres.
    fn m(ly: f64) -> f64 {
        ly * METRES_PER_LIGHT_YEAR
    }

    fn length(v: [f64; 3]) -> f64 {
        dot(v, v).sqrt()
    }

    /// The spray's checks on the Milky Way fixture, each below on one build of its tables, in
    /// turn: the golden file first, so that a failing check does not keep `just bless` from
    /// writing it, then the table against its own gradient and the leapfrog's field, and the
    /// tracers' purity and weights. Here rather than in an integration test, so that the sim's
    /// fast suite compares the golden on `wasm32-wasip1` too, where `global_list::testing`'s
    /// feature is off.
    #[test]
    fn the_spray_is_pinned_its_table_conservative_and_its_tracers_pure() {
        golden_sprays();
        the_table_is_its_potential_s_gradient_and_near_the_leapfrog_s();
        tracers_are_pure_functions_of_their_index_and_weigh_the_tube();
        the_drawn_offsets_keep_their_dispersions_within_the_cut();
    }

    /// The bits of each test stream's specification and of a spray of four tracers an arm, and of
    /// the spray table at three points, one beyond its last node. They depend on the potential
    /// tables, the step rule, the release offsets and the table, so a change to any moves them;
    /// the integration is the leapfrog's arithmetic, which every architecture the sim's tests run
    /// on must reproduce.
    fn golden_sprays() {
        let mut writer = GoldenWriter::new();
        writer.header(crate::GENERATOR_VERSION.get());
        writer.f64("cut_widening", cut_widening());
        let force = &fixture().force;
        for (name, [x, y, z]) in [
            ("sun", [26_000.0, 0.0, 30.0]),
            ("halo", [-60_000.0, 40_000.0, 90_000.0]),
            ("far", [700_000.0, 300_000.0, -1_200_000.0]),
        ] {
            let at = GalacticDisplacement::new([m(x), m(y), m(z)]);
            writer.f64(
                &format!("table.{name}.potential"),
                force.potential_j_kg(&at),
            );
            for (axis, a) in ["x", "y", "z"].iter().zip(force.acceleration_m_s2(&at)) {
                writer.f64(&format!("table.{name}.acceleration.{axis}"), a);
            }
        }
        for (name, spec) in test_streams() {
            writer.f64(
                &format!("{name}.stripping_time"),
                spec.stripping_time().value(),
            );
            writer.f64(&format!("{name}.mass"), spec.mass().value());
            writer.f64(&format!("{name}.tidal_radius"), spec.tidal_radius().value());
            let spray = spray(SEED, &spec, 4);
            writer.line(&format!("{name}.steps {}", spray.schedule().step().count()));
            writer.f64(&format!("{name}.in_plane_spread"), spray.in_plane_spread());
            for (i, t) in spray.tracers().iter().enumerate() {
                let label = format!("{name}.tracer.{i}");
                writer.line(&format!(
                    "{label}.release {:?} {}",
                    t.arm(),
                    t.release_step()
                ));
                writer.f64(&format!("{label}.weight"), t.weight().value());
                let (p, v) = (t.position().metres(), t.velocity().metres_per_second());
                for (axis, (x, u)) in ["x", "y", "z"].iter().zip(p.iter().zip(v)) {
                    writer.f64(&format!("{label}.position.{axis}"), *x);
                    writer.f64(&format!("{label}.velocity.{axis}"), u);
                }
                writer.f64(&format!("{label}.psi"), t.psi().value());
            }
        }
        golden!("galaxy/global_list/spray", writer.as_str());
    }

    /// Ruling 146.3's table: its force is the exact gradient of its own potential, by central
    /// differences to 10⁻⁶, inside the table off the slabs about the plane and the axis and in
    /// its continuation beyond the last node; and it stays near the leapfrog's field, the potential
    /// to 2 × 10⁻⁵ and the force to 10⁻³, P10.T2.a's bound against the mass model, over 1–300 kpc
    /// at nine latitudes (measured on the pod, 2026-10-28, at v23: 1.1 × 10⁻⁵ and 2.5 × 10⁻⁴, the
    /// latter at 270 kpc).
    fn the_table_is_its_potential_s_gradient_and_near_the_leapfrog_s() {
        let Fixture { tables, force } = fixture();
        let leapfrog =
            Leapfrog::new(tables, Seconds::from(Megayears::new(1.0))).expect("the full tables");
        let gradient_holds = |p: [f64; 3], what: &str| {
            let a = force.acceleration_m_s2(&GalacticDisplacement::new(p));
            let step = 1e-4 * length(p);
            for axis in 0..3 {
                let shifted = |by: f64| {
                    let mut q = p;
                    q[axis] += by;
                    force.potential_j_kg(&GalacticDisplacement::new(q))
                };
                let slope = -(shifted(step) - shifted(-step)) / (2.0 * step);
                assert!(
                    (slope - a[axis]).abs() < 1e-6 * length(a),
                    "{what}, axis {axis}: {slope:e} against {:e}",
                    a[axis]
                );
            }
        };
        let (mut worst_potential, mut worst_force) = (0.0_f64, 0.0_f64);
        for k in 0..60_u32 {
            // 1–300 kpc, log-spaced.
            let r = 3_261.6 * math::powf(300.0, f64::from(k) / 59.0);
            for latitude in [0.0_f64, 1.0, 5.0, 10.0, 30.0, 50.0, 70.0, 85.0, 90.0] {
                let (sin, cos) = math::sin_cos(latitude.to_radians());
                let p = [m(r * cos * 0.6), m(r * cos * 0.8), m(r * sin)];
                let at = GalacticDisplacement::new(p);
                let phi = force.potential_j_kg(&at);
                worst_potential =
                    worst_potential.max((phi / leapfrog.potential_j_kg(&at) - 1.0).abs());
                let (fast, exact) = (
                    force.acceleration_m_s2(&at),
                    leapfrog.acceleration_m_s2(&at),
                );
                let difference = [fast[0] - exact[0], fast[1] - exact[1], fast[2] - exact[2]];
                worst_force = worst_force.max(length(difference) / length(exact));
                if (5.0..=85.0).contains(&latitude) {
                    gradient_holds(p, &format!("{r:.0} ly at latitude {latitude}°"));
                }
            }
        }
        // Beyond the last node, 1.1 × 10⁶ ly: in R, in |z|, and in both.
        for p in [[1.5e6, 2e5, 3e5], [2e5, -1e5, 1.4e6], [-1.3e6, 9e5, -1.6e6]] {
            gradient_holds(p.map(m), &format!("{p:?} ly, beyond the table"));
        }
        assert!(worst_potential < 2e-5, "{worst_potential:e}");
        assert!(worst_force < 1e-3, "{worst_force:e}");
    }

    /// The offsets a spray draws (main's ruling of 2026-10-29): over 20,000 tracers' words, the
    /// in-plane offsets' variances about their means are eq. 21's dispersion squared and the
    /// vertical ones' [`VERTICAL_SPREAD`]'s, each to 3 %, a variance's error being about 1 % here
    /// and the cut without the widening 8.9 % narrow; and every offset lies within the cut, at
    /// [`OFFSET_CUT`] widened dispersions. For `pal5_like`'s dispersion, 0.278, and
    /// `sagittarius_like`'s, the cap of 0.4.
    fn the_drawn_offsets_keep_their_dispersions_within_the_cut() {
        let streams = test_streams();
        for (name, spec) in [&streams[0], &streams[2]] {
            let sprayed = spray(SEED, spec, 4);
            let spread = sprayed.in_plane_spread();
            let n = 20_000_u32;
            let mut squares = [0.0; 4];
            for i in 0..n {
                let offsets = sprayed.offsets(i);
                let deviations = [
                    offsets[0] - RADIAL_OFFSET,
                    offsets[1] - TANGENTIAL_OFFSET,
                    offsets[2],
                    offsets[3],
                ];
                for (k, (d, sigma)) in deviations
                    .into_iter()
                    .zip([spread, spread, VERTICAL_SPREAD, VERTICAL_SPREAD])
                    .enumerate()
                {
                    assert!(
                        d.abs() <= OFFSET_CUT * cut_widening() * sigma * (1.0 + 1e-12),
                        "{name}: tracer {i}'s offset {k}, {d}, beyond the cut"
                    );
                    squares[k] += d * d;
                }
            }
            for (k, (square, sigma)) in squares
                .into_iter()
                .zip([spread, spread, VERTICAL_SPREAD, VERTICAL_SPREAD])
                .enumerate()
            {
                let variance = square / f64::from(n);
                assert!(
                    (variance / (sigma * sigma) - 1.0).abs() < 0.03,
                    "{name}: offset {k}'s variance {variance} against {}",
                    sigma * sigma
                );
            }
        }
    }

    /// A tracer is a pure function of its index, so a pool may integrate them in any order and
    /// gather them by index (ruling 146.3), over the three streams together too; the same seed
    /// gives the same spray twice and another seed another; the tracers' weights sum to the tube's
    /// mass (P10.T5.a, through the spray); and the arms leave in pairs.
    fn tracers_are_pure_functions_of_their_index_and_weigh_the_tube() {
        let streams = test_streams();
        let sprays: Vec<Spray<'static>> = streams
            .iter()
            .map(|(_, spec)| spray(SEED, spec, 6))
            .collect();
        let keys: Vec<(usize, u32)> = (0..sprays.len())
            .flat_map(|s| (0..sprays[s].tracer_count()).map(move |i| (s, i)))
            .collect();
        assert_eq!(keys.len(), 36);
        assert_order_independent(&keys, |&(s, i)| sprays[s].tracer(i));
        for ((name, spec), sprayed) in streams.iter().zip(&sprays) {
            let tracers = sprayed.tracers();
            assert_eq!(
                spray(SEED, spec, 6).tracers(),
                tracers,
                "{name}: the same seed"
            );
            assert_ne!(
                spray(Seed::new(1), spec, 6).tracers(),
                tracers,
                "{name}: another seed"
            );
            let total = tracers.iter().fold(0.0, |sum, t| sum + t.weight().value());
            assert!(
                (total / spec.mass().value() - 1.0).abs() < 1e-12,
                "{name}: {total} against {}",
                spec.mass().value()
            );
            for pair in tracers.chunks(2) {
                assert_eq!(
                    [pair[0].arm(), pair[1].arm()],
                    [Arm::Leading, Arm::Trailing]
                );
                assert_eq!(pair[0].release_step(), pair[1].release_step());
            }
        }
    }
}
