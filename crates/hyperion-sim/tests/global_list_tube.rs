//! The tube table's slow tests (plan 10, P10.T6.a–c): the three test streams of
//! `global_list::testing`, reduced from the generator's 2,000 tracers, the strays of
//! `sagittarius_like` under several seeds, the power of `gd1_like`'s gap noise, and the cells a
//! sphere touches.
//!
//! The (R, z) grid costs seconds to build, so the test streams read one set of tables, the Milky
//! Way fixture's, and its spray table, as `tests/global_list_spray.rs` does, and each stream is
//! sprayed once for every test here. The reduction's fast checks, on a synthetic stream, are
//! `global_list::tube`'s unit tests; this file needs the `testing` feature, which the
//! `wasm32-wasip1` run does not turn on.

#![cfg(feature = "testing")]

use std::sync::OnceLock;

use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::global_list::spray::{self, Arm, Spray, SprayForce, SprayParams, Tracer};
use hyperion_sim::galaxy::global_list::testing::{gd1_like, pal5_like, sagittarius_like};
use hyperion_sim::galaxy::global_list::tube::{
    ACROSS_CELLS, CUT_WIDTHS, GAP_NOISE_AMPLITUDE, KNOTS_PER_ARM, TubeCell, TubeKnot,
};
use hyperion_sim::galaxy::global_list::{StreamSpec, TubeTable};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::rng::{ObjectKey, Stream, tags};
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
use hyperion_sim::units::{LightYears, SolarMasses};
use hyperion_sim::{Seed, math};

const SEED: Seed = Seed::new(0x0a10_5b00_0000_0000);

struct Fixture {
    tables: PotentialTables,
    force: SprayForce,
    /// The halo's mean system mass, which cuts a tube's along cells.
    mean_mass: SolarMasses,
}

fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let params = GalaxyParams::milky_way_like();
        let tables = PotentialTables::full(&MassModel::new(&params));
        let force = SprayForce::new(&tables).expect("the full tables");
        Fixture {
            tables,
            force,
            mean_mass: params.mean_system_mass(Population::Halo),
        }
    })
}

/// A test stream, its tracers at the generator's 2,000 and its tube.
struct Sprayed {
    name: &'static str,
    spec: StreamSpec,
    tracers: Vec<Tracer>,
    table: TubeTable,
}

/// `spec`'s tracers in the fixture under `seed`, and their tube.
fn tube(seed: Seed, spec: &StreamSpec) -> (Vec<Tracer>, TubeTable) {
    let f = fixture();
    let spray = Spray::new(seed, &f.force, &f.tables, spec, &SprayParams::default())
        .expect("a finite stripping time");
    let tracers = spray.tracers();
    let table = TubeTable::from_tracers(seed, f.mean_mass, spec, &tracers)
        .expect("enough tracers in each arm");
    (tracers, table)
}

/// The three test streams, sprayed once: `pal5_like`, `gd1_like` and `sagittarius_like`.
fn sprayed() -> &'static [Sprayed; 3] {
    static SPRAYED: OnceLock<[Sprayed; 3]> = OnceLock::new();
    SPRAYED.get_or_init(|| {
        let tables = &fixture().tables;
        [
            ("pal5", pal5_like(tables)),
            ("gd1", gd1_like(tables)),
            ("sagittarius", sagittarius_like(tables)),
        ]
        .map(|(name, spec)| {
            let (tracers, table) = tube(SEED, &spec);
            Sprayed {
                name,
                spec,
                tracers,
                table,
            }
        })
    })
}

fn ly(p: &GalacticPosition) -> [f64; 3] {
    p.to_light_years_f64()
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).expect("a test's count fits a u32"))
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

/// The polyline's length through `points`, ly.
fn path(points: &[[f64; 3]]) -> f64 {
    points.windows(2).map(|w| norm(sub(w[1], w[0]))).sum()
}

/// The tracers of each arm by ψ's sign, trailing then leading, in the order of increasing ψ, as
/// the table splits them.
fn by_sign(tracers: &[Tracer]) -> [Vec<&Tracer>; 2] {
    let mut along: Vec<&Tracer> = tracers.iter().collect();
    along.sort_by(|a, b| a.psi().value().total_cmp(&b.psi().value()));
    let split = along.partition_point(|t| t.psi().value() < 0.0);
    let leading = along.split_off(split);
    [along, leading]
}

/// The ruling's length check (main's ruling of 2026-10-29): the tracers' own path, through the
/// weighted centroids of each arm's tracers in bins of 48 by ψ in the order of increasing ψ, and
/// the knots' arc length over the same stretch of the tube, from the first bin's middle by mass
/// to the last's, in light-years. The bins' path stops about 24 tracers short of each tip, where a
/// stream's oldest tracers lie sparse, so the knots' path from tip to tip is longer.
fn lengths(tracers: &[Tracer], table: &TubeTable) -> [f64; 2] {
    let total = table.mass().value();
    let mut behind = 0.0;
    let mut centroids = Vec::new();
    let mut middles = Vec::new();
    for arm in by_sign(tracers) {
        for bin in arm.chunks(48) {
            let mass: f64 = bin.iter().map(|t| t.weight().value()).sum();
            centroids.push(core::array::from_fn(|axis| {
                bin.iter()
                    .map(|t| t.weight().value() * t.position().metres()[axis])
                    .sum::<f64>()
                    / mass
                    / METRES_PER_LIGHT_YEAR
            }));
            middles.push((behind + 0.5 * mass) / total);
            behind += mass;
        }
    }
    let (first, last) = (middles[0], middles[middles.len() - 1]);
    let mut knots = vec![ly(&table.locate(first, 0.0, 0.0))];
    knots.extend(
        table
            .knots()
            .filter(|k| {
                let u = k.cumulative_mass().value() / total;
                u > first && u < last
            })
            .map(|k| ly(&k.centre())),
    );
    knots.push(ly(&table.locate(last, 0.0, 0.0)));
    [path(&centroids), path(&knots)]
}

/// The along coordinate whose ψ is `psi`, by bisection of the table's ψ(u), which never falls.
fn along(table: &TubeTable, psi: f64) -> f64 {
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..60 {
        let middle = f64::midpoint(low, high);
        if table.psi(middle).value() < psi {
            low = middle;
        } else {
            high = middle;
        }
    }
    f64::midpoint(low, high)
}

/// A tracer's coordinates in the tube at its ψ: its offsets from the centre line there along the
/// normal and the binormal, in local widths, and along the tangent, in light-years.
fn tube_coordinates(table: &TubeTable, tracer: &Tracer) -> [f64; 3] {
    let u = along(table, tracer.psi().value());
    let centre = ly(&table.locate(u, 0.0, 0.0));
    let x = tracer
        .position()
        .metres()
        .map(|c| c / METRES_PER_LIGHT_YEAR);
    let d = sub(x, centre);
    let step = |a: f64, b: f64| sub(ly(&table.locate(u, a, b)), centre);
    let (normal, binormal) = (step(1.0, 0.0), step(0.0, 1.0));
    let ahead = sub(ly(&table.locate((u + 1e-4).min(1.0), 0.0, 0.0)), centre);
    let tangent = ahead.map(|c| c / norm(ahead));
    [
        dot(d, normal) / dot(normal, normal),
        dot(d, binormal) / dot(binormal, binormal),
        dot(d, tangent),
    ]
}

/// P10.T6.a on the three test streams at the generator's 2,000 tracers: every knot's frame
/// orthonormal to 10⁻¹² and right-handed, never flipping; the knots in order of ψ; the arms'
/// masses adding up to the tracers'; and each tube's length within 5 % of its tracers' own path
/// over the same stretch (main's ruling of 2026-10-29, which restates the plan's "near 82,000 ly"
/// and "near 159,000 ly"). Measured at v23, x86-64: 217,410 ly against 218,861 (`pal5_like`),
/// 112,540 against 113,681 (`gd1_like`) and 724,505 against 711,460 (`sagittarius_like`); from
/// tip to tip 281,431, 135,899 and 814,020 ly, against the brainstorm's 30,000–160,000 ly for
/// globular streams.
#[test]
#[ignore = "slow: sprays three test streams of 2,000 tracers, 20,000 steps for the longest"]
fn the_test_streams_tubes_follow_their_tracers() {
    for Sprayed {
        name,
        tracers,
        table,
        ..
    } in sprayed()
    {
        let knots: Vec<&TubeKnot> = table.knots().collect();
        assert_eq!(knots.len(), 512, "{name}");
        for (j, k) in knots.iter().enumerate() {
            let [t, n, b] = k.frame();
            for v in [t, n, b] {
                assert!((dot(v, v) - 1.0).abs() < 1e-12, "{name}, knot {j}");
            }
            for (p, q) in [(t, n), (t, b), (n, b)] {
                assert!(dot(p, q).abs() < 1e-12, "{name}, knot {j}");
            }
            let handed = [
                t[1] * n[2] - t[2] * n[1],
                t[2] * n[0] - t[0] * n[2],
                t[0] * n[1] - t[1] * n[0],
            ];
            assert!(norm(sub(handed, b)) < 1e-12, "{name}, knot {j}");
        }
        let turns: Vec<f64> = knots
            .windows(2)
            .map(|w| dot(w[0].frame()[1], w[1].frame()[1]))
            .collect();
        let least = turns.iter().copied().fold(1.0_f64, f64::min);
        assert!(least > 0.0, "{name}: the frame flips ({least})");
        assert!(
            knots.windows(2).all(|w| w[1].psi() >= w[0].psi()),
            "{name}: the knots' ψ falls"
        );
        let weights = tracers.iter().fold(0.0, |sum, t| sum + t.weight().value());
        assert!(
            (table.mass().value() / weights - 1.0).abs() < 1e-12,
            "{name}"
        );
        let [bins, knotted] = lengths(tracers, table);
        let length = table.length().value();
        // Recorded for the owner: the length over the central 90 % of the mass, and how far the
        // binormal strays from each knot's own orbital pole, the mean angular momentum's
        // direction.
        let central = path(
            &(0..=1_000)
                .map(|i| ly(&table.locate(0.05 + 0.9 * f64::from(i) / 1e3, 0.0, 0.0)))
                .collect::<Vec<_>>(),
        );
        let off_pole = knots
            .iter()
            .map(|k| {
                let (x, v) = (ly(&k.centre()), k.velocity().metres_per_second());
                let pole = [
                    x[1] * v[2] - x[2] * v[1],
                    x[2] * v[0] - x[0] * v[2],
                    x[0] * v[1] - x[1] * v[0],
                ];
                (dot(pole, k.frame()[2]) / norm(pole)).clamp(-1.0, 1.0)
            })
            .fold(1.0_f64, f64::min);
        eprintln!(
            "{name}: tube {length:.0} ly from tip to tip, {central:.0} ly over the central 90 % \
             of its mass, ψ {:.3} to {:.3}; over the bins' stretch {knotted:.0} ly against the \
             bins' path {bins:.0} ly ({:+.2} %); least normal-to-normal cosine {least:.4}; least \
             binormal-to-pole cosine {off_pole:.4}",
            knots[0].psi().value(),
            knots[511].psi().value(),
            100.0 * (knotted / bins - 1.0)
        );
        assert!(
            (knotted / bins - 1.0).abs() < 0.05,
            "{name}: {knotted} ly against the bins' {bins}"
        );
    }
}

/// The two globular streams' widths and dispersions, the medians over their knots, against plan
/// 10's brackets: `pal5_like`'s widths are inside its 120–380 ly and its binormal dispersion
/// inside its 0.9–3.5 km/s; its other two dispersions and all of `gd1_like`'s widths and
/// dispersions miss low. All are pinned as found, to a quarter, since main's ruling of
/// 2026-10-29 records them before any tuning (`f_t` = 1, the sourced lever, is not pulled).
/// Measured at v23, x86-64: `pal5_like`'s widths 156 and 252 ly and dispersions 0.55, 0.81 and
/// 1.11 km/s against 0.9–3.5 km/s; `gd1_like`'s widths 52 and 56 ly against 100–260 ly and
/// dispersions 0.16, 0.49 and 0.24 km/s against 0.7–1.3 km/s. The dispersions are along the
/// tangent, the normal and the binormal.
#[test]
#[ignore = "slow: sprays three test streams of 2,000 tracers, 20,000 steps for the longest"]
fn the_globular_streams_widths_and_dispersions_are_pinned() {
    let pinned: [([f64; 2], [f64; 3]); 2] = [
        ([156.0, 252.0], [0.55, 0.81, 1.11]),
        ([52.0, 56.0], [0.16, 0.49, 0.24]),
    ];
    for (Sprayed { name, table, .. }, (widths_pinned, dispersions_pinned)) in
        sprayed()[..2].iter().zip(pinned)
    {
        let knots: Vec<&TubeKnot> = table.knots().collect();
        let widths: [f64; 2] =
            core::array::from_fn(|a| median(knots.iter().map(|k| k.widths()[a].value()).collect()));
        let dispersions: [f64; 3] = core::array::from_fn(|a| {
            median(
                knots
                    .iter()
                    .map(|k| k.dispersions()[a].value() / 1e3)
                    .collect(),
            )
        });
        eprintln!(
            "{name}: widths {:.0} and {:.0} ly; dispersions {:.2}, {:.2} and {:.2} km/s",
            widths[0], widths[1], dispersions[0], dispersions[1], dispersions[2]
        );
        if *name == "pal5" {
            assert!(
                widths.iter().all(|w| (120.0..=380.0).contains(w)),
                "{name}: widths {widths:?}"
            );
        }
        for (measured, pin) in widths.iter().zip(widths_pinned) {
            assert!(
                (measured / pin - 1.0).abs() < 0.25,
                "{name}: width {measured} against {pin}"
            );
        }
        for (measured, pin) in dispersions.iter().zip(dispersions_pinned) {
            assert!(
                (measured / pin - 1.0).abs() < 0.25,
                "{name}: dispersion {measured} against {pin}"
            );
        }
    }
}

/// `gd1_like`'s cluster dissolved 4.5 Gyr ago, so its tube holds a gap at the progenitor: within
/// the central 5 % of its ψ it holds under a tenth of what a tube even in ψ would, where
/// `pal5_like`'s, still releasing, holds more. Measured at v23, x86-64: 0.0033 and 0.198 of the
/// tubes' masses against 0.05.
#[test]
#[ignore = "slow: sprays three test streams of 2,000 tracers, 20,000 steps for the longest"]
fn an_orphan_s_tube_holds_its_cluster_s_gap() {
    for Sprayed { name, table, .. } in &sprayed()[..2] {
        let knots: Vec<&TubeKnot> = table.knots().collect();
        let span = knots[511].psi().value() - knots[0].psi().value();
        let half = 0.025 * span;
        let share = along(table, half) - along(table, -half);
        eprintln!(
            "{name}: |ψ| < {half:.3} holds {share:.4} of the tube against 0.05 for an even one"
        );
        if *name == "gd1" {
            assert!(share < 0.005, "{name}: the gap holds {share}");
        } else {
            assert!(
                share > 0.05,
                "{name}: the progenitor's neighbourhood holds {share}"
            );
        }
    }
}

/// The line density per unit arc length is higher near a centre line's apocentric points (local
/// maxima of galactocentric radius over 16 knots each way) than near its pericentric points, for
/// each group of `tracers` that `group` names separately: each density the group's tracers within
/// 20,000 ly of the point along the centre line, where a tracer's place along it is its ψ's, and a
/// group compared where the central 90 % of its tracers spans an apocentric and a pericentric
/// point with their reach. Returns the groups compared.
fn assert_pile_up(
    name: &str,
    tracers: &[Tracer],
    table: &TubeTable,
    group: impl Fn(&Tracer) -> u32,
) -> u32 {
    const WINDOW: usize = 16;
    const REACH: f64 = 20_000.0;
    let knots: Vec<&TubeKnot> = table.knots().collect();
    let centres: Vec<[f64; 3]> = knots.iter().map(|k| ly(&k.centre())).collect();
    let mut s = vec![0.0];
    for w in centres.windows(2) {
        let last = s[s.len() - 1];
        s.push(last + norm(sub(w[1], w[0])));
    }
    let r: Vec<f64> = centres.iter().map(|c| norm(*c)).collect();
    let (mut apocentres, mut pericentres) = (Vec::new(), Vec::new());
    for k in WINDOW..r.len() - WINDOW {
        let around = &r[k - WINDOW..=k + WINDOW];
        if around.iter().all(|&x| x <= r[k]) {
            apocentres.push(k);
        }
        if around.iter().all(|&x| x >= r[k]) {
            pericentres.push(k);
        }
    }
    eprintln!(
        "{name}: apocentric points at {:?} ly, pericentric at {:?} ly",
        apocentres.iter().map(|&k| r[k].round()).collect::<Vec<_>>(),
        pericentres
            .iter()
            .map(|&k| r[k].round())
            .collect::<Vec<_>>()
    );
    let psis: Vec<f64> = knots.iter().map(|k| k.psi().value()).collect();
    let s_at = |psi: f64| {
        let j = psis.partition_point(|&p| p <= psi).clamp(1, psis.len() - 1) - 1;
        let f = ((psi - psis[j]) / (psis[j + 1] - psis[j])).clamp(0.0, 1.0);
        s[j] + f * (s[j + 1] - s[j])
    };
    let mut groups: Vec<u32> = tracers.iter().map(&group).collect();
    groups.sort_unstable();
    groups.dedup();
    let mut compared = 0;
    for g in groups {
        let mine: Vec<f64> = tracers
            .iter()
            .filter(|t| group(t) == g)
            .map(|t| s_at(t.psi().value()))
            .collect();
        let mut sorted = mine.clone();
        sorted.sort_by(f64::total_cmp);
        let (low, high) = (sorted[sorted.len() / 20], sorted[sorted.len() * 19 / 20]);
        let density = |k: usize| {
            count(mine.iter().filter(|&&x| (x - s[k]).abs() < REACH).count())
                / (2.0 * REACH * count(mine.len()))
        };
        let inside = |k: &&usize| s[**k] - REACH > low && s[**k] + REACH < high;
        let near: [Vec<f64>; 2] = [&apocentres, &pericentres]
            .map(|points| points.iter().filter(inside).map(|&k| density(k)).collect());
        eprintln!(
            "{name}: group {g}, {} tracers over {low:.0}–{high:.0} ly along: per light-year, \
             {:?} near apocentres and {:?} near pericentres",
            mine.len(),
            near[0],
            near[1]
        );
        if near.iter().all(|d| !d.is_empty()) {
            let mean = |d: &[f64]| d.iter().sum::<f64>() / count(d.len());
            assert!(
                mean(&near[0]) > mean(&near[1]),
                "{name}, group {g}: {near:?}"
            );
            compared += 1;
        }
    }
    compared
}

/// `sagittarius_like` releases in three pulses at its pericentres, and for each pulse's tracers
/// separately the line density per unit arc length is higher near the centre line's apocentric
/// points than near its pericentric points ([`assert_pile_up`]; main's ruling of 2026-10-29,
/// which rewords the plan's "a dwarf shows the pile-up at apocentre").
///
/// Measured at v23, x86-64: `sagittarius_like`'s centre line has one apocentric point, at about
/// 162,000 ly, and two pericentric, at about 66,000 and 60,000 ly; the oldest pulse, 914 tracers,
/// spans them all, with 4.0 × 10⁻⁶ of its tracers a light-year near the apocentre against 5 ×
/// 10⁻⁸ and 3 × 10⁻⁸ near the pericentres; the two later pulses span no apocentric point. Both
/// pericentric points lie within 0.7 rad of the progenitor, between the oldest pulse's two clumps,
/// which have drifted some 1.6 and 2.4 rad from it, so that contrast, some 100 times against
/// radii some 2.5 times apart, is mostly the pulse's own clumping; its leading clump peaks at the
/// apocentre. The stars' slowing alone would give about the radii's ratio (line density times
/// speed is constant along the track, and the speed at an apsis goes as one over its radius), but
/// `pal5_like`'s steady release cannot show it here: its track runs from a pericentre at one tip
/// through one apocentre to a pericentre at the other, both pericentres outside the central 90 %
/// of its tracers.
#[test]
#[ignore = "slow: sprays three test streams of 2,000 tracers, 20,000 steps for the longest"]
fn a_dwarf_s_pulses_pile_up_at_apocentre() {
    let Sprayed { tracers, table, .. } = &sprayed()[2];
    let pulses = assert_pile_up("sagittarius", tracers, table, Tracer::release_step);
    assert!(
        pulses > 0,
        "no pulse spans an apocentric and a pericentric point"
    );
}

/// Every tracer whose Lagrange point's arm is not its ψ's lies inside the other arm's tube at
/// its ψ (main's ruling of 2026-10-29): within the tube's cut at four widths there, or within two
/// of the stream's tidal radii of the progenitor at the epoch, where the arms' roots meet and
/// Fardal et al.'s releases start, at a mean of two tidal radii out (main's ruling of
/// 2026-10-30). The strays weigh at most 0.5 % of their tube. Checked on `sagittarius_like` under
/// the tests' seed and eight more, and on the globular streams (they have none). Measured at v23,
/// x86-64: two strays in the nine sprays, both at the root: under the tests' seed at ψ 0.038,
/// 2.16 and 4.78 widths out, 0.80 tidal radii from the progenitor, and under seed 7 at ψ 0.052,
/// 3.82 and 4.53 widths out, 0.88 tidal radii from it.
#[test]
#[ignore = "slow: sprays sagittarius_like under eight seeds"]
fn the_strays_lie_inside_the_other_arm_s_tube() {
    let spec = sagittarius_like(&fixture().tables);
    let seeds: Vec<(String, Vec<Tracer>, TubeTable)> = (1..=8_u64)
        .map(|seed| {
            let (tracers, table) = tube(Seed::new(seed), &spec);
            (format!("sagittarius, seed {seed}"), tracers, table)
        })
        .collect();
    let mut strays = 0;
    for (name, spec, tracers, table) in seeds
        .iter()
        .map(|(name, tracers, table)| (name.as_str(), &spec, &tracers[..], table))
        .chain(
            sprayed()
                .iter()
                .map(|s| (s.name, &s.spec, &s.tracers[..], &s.table)),
        )
    {
        let progenitor = spec.position().metres().map(|c| c / METRES_PER_LIGHT_YEAR);
        let tidal_radius = spec.tidal_radius().value();
        let (mut astray, mut weight) = (Vec::new(), 0.0);
        for t in tracers {
            let by_sign = if t.psi().value() >= 0.0 {
                Arm::Leading
            } else {
                Arm::Trailing
            };
            if by_sign != t.arm() {
                let x = t.position().metres().map(|c| c / METRES_PER_LIGHT_YEAR);
                let out = norm(sub(x, progenitor)) / tidal_radius;
                astray.push((t.psi().value(), tube_coordinates(table, t), out));
                weight += t.weight().value();
            }
        }
        for (psi, [a, b, along], out) in &astray {
            eprintln!(
                "{name}: stray at ψ {psi:.4}, {a:.2} and {b:.2} widths out and {along:.0} ly \
                 along, {out:.2} tidal radii from the progenitor"
            );
            assert!(
                a.abs().max(b.abs()) <= 4.0 || *out <= 2.0,
                "{name}: a stray at ψ {psi} lies outside the tube's cut and away from its root"
            );
        }
        let share = weight / table.mass().value();
        assert!(
            share <= 0.005,
            "{name}: the strays weigh {share} of the tube"
        );
        strays += astray.len();
    }
    assert!(strays > 0, "no stray to check");
}

/// The arc length along `table`'s centre line, ly, from the trailing arm's tip to the point at
/// along coordinate `u`, through the knots.
fn arc_at(table: &TubeTable, u: f64) -> f64 {
    let knots: Vec<&TubeKnot> = table.knots().collect();
    let mut s = vec![0.0];
    for w in knots.windows(2) {
        let last = s[s.len() - 1];
        s.push(last + norm(sub(ly(&w[1].centre()), ly(&w[0].centre()))));
    }
    // The knots' Λ rises within each arm; between the roots it does not, so no u falls there.
    let target = u.clamp(0.0, 1.0) * table.mass().value();
    let trailing = table.arm(Arm::Trailing).mass().value();
    let per_arm = usize::try_from(KNOTS_PER_ARM).expect("256 knots fit a usize");
    let (start, end) = if target < trailing {
        (0, per_arm)
    } else {
        (per_arm, 2 * per_arm)
    };
    let arm = &knots[start..end];
    let after = arm.partition_point(|k| k.cumulative_mass().value() <= target);
    match after {
        0 => s[start],
        n if n == arm.len() => s[end - 1],
        n => {
            let (low, high) = (
                arm[n - 1].cumulative_mass().value(),
                arm[n].cumulative_mass().value(),
            );
            let f = (target - low) / (high - low);
            s[start + n - 1] + f * (s[start + n] - s[start + n - 1])
        }
    }
}

/// P10.T6.b on `gd1_like`: the power of its gap noise's density contrast along the tube peaks at
/// scales of a few thousand light-years, and the contrast's rms is the amplitude's.
///
/// The contrast is each along cell's noise less 1, constant over the cell's arc of the centre
/// line ([`arc_at`]). Its Fourier modes over the tube's length are exact integrals over the cells,
/// and their power is averaged over 512 seeds of noise on the tests' tracers. The power is read
/// at the scale `1 ÷ k`, the axis Bovy, Erkal and Sanders (2017, MNRAS 466, 628) plot stream
/// density power spectra against, and taken here per logarithmic interval of `k`, `k P(k)`, so
/// that a peak is defined (lattice noise's `P(k)` is flat at large scales). The peak must lie at
/// 1–10 thousand light-years, stand at least twice the longest mode's, and stand five times every
/// mode's at scales under 1,000 ly. The contrast's rms, weighted by arc length, must be within
/// 10 % of `√(exp(σ²) − 1)`.
///
/// Measured at v23, x86-64, over 512 seeds: 128 along cells over 135,899 ly, so the nodes lie
/// about 8,500 ly apart; the ideal count, 7,000 M☉ over 64 of the halo's 0.606 M☉, is 180.5, just
/// under the step to 256 at 181.0. `k P(k)` peaks at mode 5, a wavelength of 27,180 ly and a
/// scale of 4,326 ly, with mode 6 (3,605 ly) at 0.92 of it. Mode 2, the largest outside the
/// scales' bracket, lies at 0.68 of the peak and the longest mode at 0.36, each at least 4
/// standard errors from its bracket (a mode's power over 512 seeds is good to 4.4 %); the largest
/// under 1,000 ly lies at 0.085. The rms is 0.299 against 0.307 at σ = 0.3.
#[test]
#[ignore = "slow: sprays three test streams of 2,000 tracers, 20,000 steps for the longest"]
fn gd1_like_s_gap_noise_peaks_at_a_few_thousand_light_years() {
    const SEEDS: u32 = 512;
    let Sprayed {
        spec,
        tracers,
        table,
        ..
    } = &sprayed()[1];
    let cells = table.along_cells();
    let edges: Vec<f64> = (0..=cells)
        .map(|c| arc_at(table, f64::from(c) / f64::from(cells)))
        .collect();
    let length = edges[edges.len() - 1] - edges[0];
    let tau = 2.0 * std::f64::consts::PI;
    // Every mode whose scale is at least 500 ly, well short of the median cell's 681 ly.
    let modes: Vec<u32> = (1..=u32::MAX)
        .take_while(|&m| length / (tau * f64::from(m)) >= 500.0)
        .collect();
    let mut power = vec![0.0; modes.len()];
    let mut variance = 0.0;
    for seed in 1..=SEEDS {
        let noisy = TubeTable::from_tracers(
            Seed::new(u64::from(seed)),
            fixture().mean_mass,
            spec,
            tracers,
        )
        .expect("enough tracers in each arm");
        let contrast: Vec<f64> = (0..cells).map(|c| noisy.noise(c) - 1.0).collect();
        variance += contrast
            .iter()
            .zip(edges.windows(2))
            .map(|(d, w)| d * d * (w[1] - w[0]) / length)
            .sum::<f64>()
            / f64::from(SEEDS);
        for (p, &mode) in power.iter_mut().zip(&modes) {
            let k = tau * f64::from(mode) / length;
            // ∫ e^(−iks) ds over a cell is [sin ks + i cos ks] ÷ k between its ends.
            let (mut re, mut im) = (0.0, 0.0);
            for (d, w) in contrast.iter().zip(edges.windows(2)) {
                let (sin_b, cos_b) = math::sin_cos(k * (w[1] - edges[0]));
                let (sin_a, cos_a) = math::sin_cos(k * (w[0] - edges[0]));
                re += d * (sin_b - sin_a) / k;
                im += d * (cos_b - cos_a) / k;
            }
            // The mode's share of the contrast's variance, both signs of k, times k's mode number:
            // `k P(k)` up to a constant.
            *p +=
                f64::from(mode) * 2.0 * (re * re + im * im) / (length * length) / f64::from(SEEDS);
        }
    }
    let scale = |m: u32| length / (tau * f64::from(m));
    let (peak, &highest) = power
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .expect("modes to compare");
    let at = modes[peak];
    let near: Vec<u32> = modes
        .iter()
        .zip(&power)
        .filter(|(_, p)| **p >= 0.9 * highest)
        .map(|(m, _)| *m)
        .collect();
    let short = modes
        .iter()
        .zip(&power)
        .filter(|(m, _)| scale(**m) < 1_000.0)
        .map(|(_, p)| *p)
        .fold(0.0, f64::max);
    let rms = variance.sqrt();
    let expected = (math::exp(GAP_NOISE_AMPLITUDE * GAP_NOISE_AMPLITUDE) - 1.0).sqrt();
    for (m, p) in modes.iter().zip(&power).take(16) {
        eprintln!(
            "gd1: mode {m}, a wavelength of {:.0} ly and a scale of {:.0} ly: k P(k) {:.3} of the \
             peak",
            length / f64::from(*m),
            scale(*m),
            p / highest
        );
    }
    eprintln!(
        "gd1: {cells} along cells over {length:.0} ly; k P(k) peaks at mode {at}, a wavelength of \
         {:.0} ly and a scale of {:.0} ly, modes {near:?} within 10 % of it; {:.1} times the \
         longest mode's and {:.1} times the largest under 1,000 ly; the contrast's rms {rms:.3} \
         against {expected:.3} at σ = {GAP_NOISE_AMPLITUDE}",
        length / f64::from(at),
        scale(at),
        highest / power[0],
        highest / short
    );
    assert!(
        (1_000.0..=10_000.0).contains(&scale(at)),
        "the power peaks at a scale of {} ly",
        scale(at)
    );
    assert!(highest >= 2.0 * power[0], "{power:?}");
    assert!(highest >= 5.0 * short, "{power:?}");
    assert!(
        (rms / expected - 1.0).abs() < 0.1,
        "{rms} against {expected}"
    );
}

/// `TubeTable::build` sprays the stream in the galaxy and cuts its tube with the galaxy's seed and
/// its halo's mean system mass (P10.T6.b): bit for bit the table `from_tracers` makes of the same
/// spray with those two, and not the one another seed makes.
#[test]
#[ignore = "slow: builds a galaxy's full potential and sprays sagittarius_like twice"]
fn build_cuts_the_tube_with_the_galaxy_s_seed_and_its_halo_s_mean_mass() {
    let galaxy = Galaxy::from_params(SEED, GalaxyParams::milky_way_like())
        .expect("the fixture's parameters are valid")
        .with_full_potential();
    let spec = sagittarius_like(galaxy.potential());
    let built = TubeTable::build(&galaxy, &spec).expect("the full potential");
    let tracers = spray::spray(&galaxy, &spec).expect("the full potential");
    let halo = galaxy.mean_system_mass(Population::Halo);
    let reduced = |seed| {
        TubeTable::from_tracers(seed, halo, &spec, &tracers).expect("enough tracers in each arm")
    };
    assert_eq!(built, reduced(galaxy.seed()));
    assert_ne!(built, reduced(Seed::new(1)));
}

/// The cell `(along, [a, b])` of the point at along coordinate `u` and across coordinates `a` and
/// `b` within the cut, in a tube of `cells` along cells.
fn cell_of(u: f64, across: [f64; 2], cells: u32) -> (u32, [u8; 2]) {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "each index is floored and clamped to its cells first"
    )]
    let index = |x: f64, n: u32| x.floor().clamp(0.0, f64::from(n - 1)) as u32;
    let across = across.map(|c| {
        let j = index(
            (c + CUT_WIDTHS) * f64::from(ACROSS_CELLS) / (2.0 * CUT_WIDTHS),
            u32::from(ACROSS_CELLS),
        );
        u8::try_from(j).expect("below 64")
    });
    (index(u * f64::from(cells), cells), across)
}

/// Spheres about one test tube: about random points within and beyond the cut, and about the
/// middles of the five segments where the frame turns most, of radii from 10 ly to 30,000 ly.
fn spheres(table: &TubeTable, n: u32) -> Vec<(f64, GalacticPosition, f64)> {
    let mut draws = Stream::open(
        Seed::new(0x0a10_6c04),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy_item(u64::from(table.stream().get())),
    );
    let mut uniform = move || draws.uniform();
    let knots: Vec<&TubeKnot> = table.knots().collect();
    let total = table.mass().value();
    let mut turns: Vec<(f64, f64)> = knots
        .windows(2)
        .filter(|pair| pair[1].cumulative_mass().value() > pair[0].cumulative_mass().value())
        .map(|pair| {
            let (n0, n1) = (pair[0].frame()[1], pair[1].frame()[1]);
            let middle = f64::midpoint(
                pair[0].cumulative_mass().value(),
                pair[1].cumulative_mass().value(),
            );
            (dot(n0, n1), middle / total)
        })
        .collect();
    turns.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut centres: Vec<f64> = turns.iter().take(5).map(|t| t.1).collect();
    centres.extend((0..n).map(|_| uniform()));
    centres
        .into_iter()
        .map(|u| {
            let (a, b) = (12.0 * uniform() - 6.0, 12.0 * uniform() - 6.0);
            let radius = 10.0 * math::powf(3_000.0, uniform());
            (u, table.locate(u, a, b), radius)
        })
        .collect()
}

/// `cells_touching` finds every cell that holds a point of a test tube within a sphere (P10.T6.c),
/// on the generator's tubes, where `sagittarius_like`'s frame turns by up to 54° from knot to
/// knot (P10.T6.a, as built): points drawn within the cut at along coordinates about each
/// sphere's, every one inside the sphere in a cell it found, the cells ascending and unique. The
/// table, its 512 knots of 168 bytes and its 510 segments' reaches, holds about 90 kB, against
/// the brainstorm's 80 kB.
#[test]
#[ignore = "slow: sprays three test streams of 2,000 tracers, 20,000 steps for the longest"]
fn cells_touching_holds_every_point_of_the_test_tubes_in_a_sphere() {
    for Sprayed { name, table, .. } in sprayed() {
        let cells = table.along_cells();
        let mut draws = Stream::open(
            Seed::new(0x0a10_6c05),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy_item(u64::from(table.stream().get())),
        );
        let mut uniform = move || draws.uniform();
        let (mut inside, mut found) = (0_u32, 0_usize);
        for (u_centre, centre, radius) in spheres(table, 40) {
            let touched: Vec<TubeCell> = table
                .cells_touching(&centre, LightYears::new(radius))
                .collect();
            assert!(
                touched.windows(2).all(|w| w[0] < w[1]),
                "{name}: ascending and unique"
            );
            found += touched.len();
            let set: std::collections::BTreeSet<(u32, [u8; 2])> =
                touched.iter().map(|c| (c.along(), c.across())).collect();
            for k in 0..3_000_u32 {
                // Half the points within 0.003 of the centre's along coordinate, half within 0.03.
                let spread = if k % 2 == 0 { 0.003 } else { 0.03 };
                let u = (u_centre + spread * (2.0 * uniform() - 1.0)).clamp(0.0, 1.0 - 1e-12);
                let across = [8.0 * uniform() - 4.0, 8.0 * uniform() - 4.0];
                let point = table.locate(u, across[0], across[1]);
                if point.distance_to(&centre).value() <= radius * METRES_PER_LIGHT_YEAR {
                    inside += 1;
                    let cell = cell_of(u, across, cells);
                    assert!(
                        set.contains(&cell),
                        "{name}: {cell:?} of ({u}, {across:?}) within {radius} ly of the \
                         sphere about u = {u_centre}"
                    );
                }
            }
        }
        eprintln!(
            "{name}: {inside} points inside 45 spheres, each in a cell found; {found} cells \
             found; the table holds {} bytes",
            table.byte_size()
        );
        assert!(
            inside > 2_000,
            "{name}: {inside} points drawn inside the spheres"
        );
        assert!(table.byte_size() < 100_000, "{}", table.byte_size());
    }
}
