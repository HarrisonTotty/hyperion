//! The particle spray's slow tests (plan 10, P10.T5.b): the three test streams of
//! `global_list::testing` at the generator's 2,000 tracers, and a drawn galaxy's streams.
//!
//! The (R, z) grid costs seconds to build, so the test streams read one set of tables, the Milky
//! Way fixture's, and its spray table, as `tests/global_list_orbit.rs` does. The fast checks (the
//! golden file, the table against the leapfrog and the tracers' purity) are `global_list::spray`'s
//! unit tests, which the sim's fast suite also runs on `wasm32-wasip1`; this file needs the
//! `testing` feature, which that run does not turn on.

#![cfg(feature = "testing")]

use std::sync::OnceLock;

use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticDisplacement;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::global_list::spray::{Arm, Spray, SprayForce, SprayParams, Tracer};
use hyperion_sim::galaxy::global_list::testing::{gd1_like, pal5_like, sagittarius_like};
use hyperion_sim::galaxy::global_list::{Debris, StreamSpec};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
use hyperion_sim::math;
use hyperion_sim::units::Seconds;
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;

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

/// `spec`'s spray in the fixture, at the generator's 2,000 tracers.
fn spray(spec: &StreamSpec) -> Spray<'static> {
    let f = fixture();
    Spray::new(SEED, &f.force, &f.tables, spec, &SprayParams::default())
        .expect("a finite stripping time")
}

/// Light-years to metres.
fn m(ly: f64) -> f64 {
    ly * METRES_PER_LIGHT_YEAR
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Whether `tracer`'s ψ has its arm's sign. One released at the epoch has none, and passes.
fn on_its_side(tracer: &Tracer) -> bool {
    let psi = tracer.psi().value();
    match tracer.arm() {
        Arm::Leading => psi >= 0.0,
        Arm::Trailing => psi <= 0.0,
    }
}

/// Mean |ψ| over the arm's tracers in groups by release time, the most recent first: groups of a
/// tenth of the arm, each closed only between release steps, so that a pulse is one group.
fn mean_gain_by_release(tracers: &[Tracer], arm: Arm) -> Vec<f64> {
    let mut on: Vec<&Tracer> = tracers.iter().filter(|t| t.arm() == arm).collect();
    on.sort_by_key(|t| std::cmp::Reverse(t.release_step()));
    let size = on.len() / 10;
    let mut means = Vec::new();
    let mut group: Vec<f64> = Vec::new();
    for (i, t) in on.iter().enumerate() {
        group.push(t.psi().value().abs());
        let closes = on
            .get(i + 1)
            .is_none_or(|next| next.release_step() != t.release_step());
        if group.len() >= size && closes {
            means.push(group.iter().sum::<f64>() / f64::from(u32::try_from(group.len()).unwrap()));
            group.clear();
        }
    }
    if !group.is_empty() {
        means.push(group.iter().sum::<f64>() / f64::from(u32::try_from(group.len()).unwrap()));
    }
    means
}

/// How far `spray`'s mean track departs from its progenitor's orbit within one wrap, over the
/// tidal radius: the tracers of each arm in groups of 50 by ψ, each group's mean position against
/// the nearest point of the progenitor's orbit, from the stripping time before the epoch to as
/// long after, within half a radian of the group's mean ψ in angle swept from the epoch. Groups
/// past one wrap, `|ψ| > 2π`, are left out.
fn departure(spray: &Spray<'_>, spec: &StreamSpec, tracers: &[Tracer]) -> f64 {
    let force = &fixture().force;
    let steps = spray.schedule().step().count();
    let seconds = spray.schedule().step().step().value();
    let swept = |a: [f64; 3], b: [f64; 3]| {
        let c = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        math::atan2(
            (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt(),
            a[0] * b[0] + a[1] * b[1] + a[2] * b[2],
        )
    };
    // The orbit: the spray's own track back to the start, then forward as long past the epoch.
    let (epoch, velocity) = spray.progenitor(steps).expect("the epoch's boundary");
    let mut path = vec![(epoch.metres(), 0.0)];
    let (mut angle, mut previous) = (0.0, epoch.metres());
    for s in (0..steps).rev() {
        let p = spray.progenitor(s).expect("a boundary").0.metres();
        angle -= swept(previous, p);
        path.push((p, angle));
        previous = p;
    }
    let (mut position, mut speed) = (epoch.metres(), velocity.metres_per_second());
    let mut pull = force.acceleration_m_s2(&epoch);
    let mut angle = 0.0;
    for _ in 0..steps {
        let before = position;
        for (v, a) in speed.iter_mut().zip(pull) {
            *v += 0.5 * seconds * a;
        }
        for (x, v) in position.iter_mut().zip(speed) {
            *x += seconds * v;
        }
        pull = force.acceleration_m_s2(&GalacticDisplacement::new(position));
        for (v, a) in speed.iter_mut().zip(pull) {
            *v += 0.5 * seconds * a;
        }
        angle += swept(before, position);
        path.push((position, angle));
    }
    let mut worst = 0.0_f64;
    for arm in [Arm::Leading, Arm::Trailing] {
        let mut on: Vec<&Tracer> = tracers.iter().filter(|t| t.arm() == arm).collect();
        on.sort_by(|a, b| a.psi().value().total_cmp(&b.psi().value()));
        for group in on.chunks(50) {
            let size = f64::from(u32::try_from(group.len()).unwrap());
            let psi = group.iter().map(|t| t.psi().value()).sum::<f64>() / size;
            if psi.abs() > 2.0 * core::f64::consts::PI {
                continue;
            }
            let mean: [f64; 3] = core::array::from_fn(|k| {
                group.iter().map(|t| t.position().metres()[k]).sum::<f64>() / size
            });
            let nearest = path
                .iter()
                .filter(|(_, at)| (at - psi).abs() < 0.5)
                .map(|(p, _)| distance(*p, mean))
                .fold(f64::INFINITY, f64::min);
            if nearest.is_finite() {
                worst = worst.max(nearest);
            }
        }
    }
    worst / m(spec.tidal_radius().value())
}

/// P10.T5.b on the three test streams at the generator's 2,000 tracers: every tracer of
/// `pal5_like` and `gd1_like` is on its arm's side, ψ positive leading and negative trailing, and
/// has its arm's energy at release (P10.T6.a); ψ grows with the time since release on average, in
/// each arm of all three; the weights sum to the tube's mass; and `pal5_like`'s mean track departs
/// from its progenitor's orbit by more than six tidal radii within one wrap, the brainstorm's
/// reason for measuring the track.
///
/// One of `sagittarius_like`'s 2,000 tracers ends on the other side (v23, x86-64), pinned at 10,
/// and the same tracer has the other arm's energy (v24, x86-64), pinned at 2, the ruling's bound;
/// with the progenitor's Plummer sphere, which main's ruling of 2026-10-29 dropped, 27 were on
/// the other side (P10.T5.b, as built). The tracks depart from the orbits by 35.3, 11.2 and 9.4 tidal radii (v23,
/// x86-64).
#[test]
#[ignore = "slow: sprays three test streams of 2,000 tracers, 20,000 steps for the longest"]
fn the_test_streams_arms_lead_and_trail_and_pal5_s_track_leaves_its_orbit() {
    for (name, spec) in test_streams() {
        let spray = spray(&spec);
        let tracers = spray.tracers();
        let total = tracers.iter().fold(0.0, |sum, t| sum + t.weight().value());
        assert!(
            (total / spec.mass().value() - 1.0).abs() < 1e-12,
            "{name}: weights {total}"
        );
        let astray = tracers.iter().filter(|t| !on_its_side(t)).count();
        let strays = tracers.iter().filter(|t| !keeps_its_energy(t)).count();
        let allowed = if name == "sagittarius" { 10 } else { 0 };
        assert!(
            astray <= allowed,
            "{name}: {astray} tracers on the other arm's side"
        );
        // At most 2 of 2,000 a stream (main's ruling of 2026-10-30).
        assert!(
            strays <= allowed.min(2),
            "{name}: {strays} tracers with the other arm's energy"
        );
        for arm in [Arm::Leading, Arm::Trailing] {
            let means = mean_gain_by_release(&tracers, arm);
            assert!(
                means.windows(2).all(|w| w[1] > w[0]),
                "{name}, {arm:?}: |ψ| by release, most recent first, {means:?}"
            );
        }
        let departs = departure(&spray, &spec, &tracers);
        eprintln!(
            "{name}: {astray} astray by ψ, {strays} by the energy; track departs by {departs:.1} \
             tidal radii; σ {:.3}",
            spray.in_plane_spread()
        );
        if name == "pal5" {
            assert!(
                departs > 6.0,
                "pal5_like's track departs by {departs} tidal radii"
            );
        }
    }
}

/// Whether `tracer`'s energy at release is its Lagrange point's arm's, the arm its tube puts it in
/// (`spray`'s "The arms").
fn keeps_its_energy(tracer: &Tracer) -> bool {
    tracer.energy_arm() == tracer.arm()
}

/// P10.T6.a (main's ruling of 2026-10-30) on a drawn Milky Way galaxy's streams: those at the step
/// counts' quantiles (0, 0.1, 0.5, 0.9 and the most), its dwarfs, and up to three of its clouds,
/// the orphans on eccentric orbits (`e ≥ 0.85`) caught near apocentre (at 0.85 of it or more at the
/// epoch) and stripped over four radial periods or fewer, the fewest periods first. A tracer's
/// energy at release is its Lagrange point's arm's for all but at most 2 of a stream's 2,000
/// (0.1 %). ψ's sign, the arms' split before the ruling, is counted and printed, not asserted: it
/// fails on the clouds. Measured at v24, x86-64: no tracer of any sampled stream has the other
/// arm's energy (streams 19, 55, 221, 142 and 153 at the quantiles, the dwarf 305, and the clouds
/// 153, 183 and 244), where ψ's sign puts 0, 9, 0, 0, 1,244, 0, 424 and 161 on the other side.
#[test]
#[ignore = "slow: builds a Milky Way galaxy's debris and sprays up to nine of its streams"]
fn a_drawn_galaxy_s_tracers_keep_their_lagrange_point_s_energy() {
    use hyperion_sim::galaxy::global_list::{MassLoss, StreamOrigin};
    let galaxy = Galaxy::from_params(
        Seed::new(0x0a10_3000_0000_0000),
        GalaxyParams::milky_way_like(),
    )
    .expect("the fixture's parameters are valid")
    .with_full_potential();
    let debris = Debris::generate(&galaxy);
    let tables = galaxy.potential();
    let force = SprayForce::new(tables).expect("the full potential");
    let mut by_steps: Vec<(u32, usize)> = debris
        .streams()
        .iter()
        .enumerate()
        .map(|(k, s)| {
            let fixed = s
                .orbit()
                .fixed_step(Seconds::from(s.stripping_time()))
                .expect("a finite stripping time");
            (fixed.count(), k)
        })
        .collect();
    by_steps.sort_unstable();
    let n = by_steps.len();
    let mut chosen: Vec<usize> = [0, n / 10, n / 2, 9 * n / 10, n - 1]
        .into_iter()
        .map(|q| by_steps[q].1)
        .collect();
    chosen.extend(
        debris
            .streams()
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s.mass_loss(), MassLoss::Pulsed { .. }))
            .map(|(k, _)| k),
    );
    let mut clouds: Vec<(f64, usize)> = debris
        .streams()
        .iter()
        .enumerate()
        .filter_map(|(k, s)| {
            let orbit = s.orbit();
            let [x, y, z] = s.position().metres().map(|c| c / METRES_PER_LIGHT_YEAR);
            let radius = (x * x + y * y + z * z).sqrt();
            let periods = s.stripping_time().value() / orbit.radial_period().value();
            (s.origin() == StreamOrigin::Orphan
                && orbit.eccentricity() >= 0.85
                && radius >= 0.85 * orbit.apocentre().value()
                && periods <= 4.0)
                .then_some((periods, k))
        })
        .collect();
    clouds.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    assert!(!clouds.is_empty(), "the fixture's galaxy holds a cloud");
    for &(_, k) in clouds.iter().take(3) {
        if !chosen.contains(&k) {
            chosen.push(k);
        }
    }
    for k in chosen {
        let spec = &debris.streams()[k];
        let spray = Spray::new(galaxy.seed(), &force, tables, spec, &SprayParams::default())
            .expect("a finite stripping time");
        let tracers = spray.tracers();
        let astray = tracers.iter().filter(|t| !on_its_side(t)).count();
        let strays = tracers.iter().filter(|t| !keeps_its_energy(t)).count();
        eprintln!(
            "stream {k} ({:?}), {} steps: {strays} of {} with the other arm's energy, {astray} on \
             the other side by ψ",
            spec.origin(),
            spray.schedule().step().count(),
            tracers.len()
        );
        assert!(
            strays * 1_000 <= tracers.len(),
            "stream {k}: {strays} of {} with the other arm's energy",
            tracers.len()
        );
    }
}
