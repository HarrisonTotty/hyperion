//! Plan 11's P11.T8.c–d: the merger classes against their targets.
//!
//! - The stellar mergers' sampler against the engine's own mergers (slow): each mark both define,
//!   the catalogue's entries near the solar circle against `brute_force_merger_members`, weighted
//!   as mergers in the source horizon, by chi-square in the entries' deciles, under the scratch
//!   table's stated tolerance.
//! - A scan of every stellar merger of the galaxy, which runs the binary engine not once (slow).

#![cfg(feature = "testing")]

#[expect(dead_code, reason = "the mergers use only the members")]
mod common;

use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::binary::StellarMergers;
use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell, ClassId};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::math;
use hyperion_sim::stellar::binary::scan::{engine_calls, scan_merger_cell};
use hyperion_sim::stellar::binary::{CarvedClass, Component, SegmentKind};

/// The Milky Way fixture's seed.
const SEED: u64 = 0x0b17_08c0_0000_0002;

fn milky_way() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// What both sides define of a stellar merger, one value per mark.
#[derive(Debug, Clone, Copy)]
struct Marks {
    log_total_mass: f64,
    mass_ratio: f64,
    log_primary_mass: f64,
    before: SegmentKind,
}

/// The chi-square per degree of freedom of the `observed` values, each with its weight, against
/// `expected`'s deciles: the weighted counts scaled to the members' effective number,
/// (Σw)² ÷ Σw², and that number.
fn decile_chi_square(expected: &[f64], observed: &[(f64, f64)]) -> (f64, f64) {
    let mut sorted: Vec<f64> = expected.to_vec();
    sorted.sort_by(f64::total_cmp);
    let edges: Vec<f64> = (1..10).map(|k| sorted[k * sorted.len() / 10]).collect();
    let mut weights = [0.0_f64; 10];
    for &(v, w) in observed {
        weights[edges.partition_point(|&e| e <= v)] += w;
    }
    let total: f64 = observed.iter().map(|(_, w)| w).sum();
    let squares: f64 = observed.iter().map(|(_, w)| w * w).sum();
    let effective = total * total / squares;
    let each = effective / 10.0;
    let statistic: f64 = weights
        .iter()
        .map(|w| {
            let o = w / total * effective;
            (o - each) * (o - each) / each
        })
        .sum();
    (statistic / 9.0, effective)
}

/// A mark's value of one merger.
type MarkOf = fn(&Marks) -> f64;

/// The weighted share of `observed` that `f` holds for.
fn weighted_share(observed: &[(Marks, f64)], f: impl Fn(&Marks) -> bool) -> f64 {
    let total: f64 = observed.iter().map(|(_, w)| w).sum();
    observed
        .iter()
        .filter(|(m, _)| f(m))
        .map(|(_, w)| w)
        .sum::<f64>()
        / total
}

/// The catalogue's stellar mergers in cells about the solar circle, until there are `n`.
fn catalogue_marks(galaxy: &Galaxy, n: usize) -> Vec<Marks> {
    let process = StellarMergers::new(galaxy);
    let mut marks = Vec::with_capacity(n + 64);
    'cells: for x in [0, -1, 1, -2, 2, -3, 3] {
        for y in 5..=7 {
            for z in [0, -1] {
                if marks.len() >= n {
                    break 'cells;
                }
                let key =
                    CatalogueCellKey::new(ClassId::STELLAR_MERGER, [x, y, z]).expect("a cell");
                let cell = CatalogueClassCell::generate(galaxy, &process, key);
                marks.extend(cell.entries().iter().map(|entry| {
                    let scan = entry.value().scan();
                    let (m1, m2) = (scan.primary_mass().value(), scan.secondary_mass().value());
                    Marks {
                        log_total_mass: math::log10(m1 + m2),
                        mass_ratio: m2 / m1,
                        log_primary_mass: math::log10(m1),
                        before: SegmentKind::Contact,
                    }
                }));
            }
        }
    }
    assert!(
        marks.len() >= n,
        "{} mergers about the solar circle",
        marks.len()
    );
    marks.truncate(n);
    marks
}

/// The engine's stellar mergers near the solar circle, each with its weight.
fn engine_marks(galaxy: &Galaxy, n: usize) -> Vec<(Marks, f64)> {
    common::binaries::brute_force_merger_members(galaxy, CarvedClass::StellarMerger, n)
        .iter()
        .map(|m| {
            let before = m.before();
            let [a, b] = before.stars().map(|s| s.mass().value());
            let (heavy, light) = (a.max(b), a.min(b));
            let marks = Marks {
                log_total_mass: math::log10(a + b),
                mass_ratio: light / heavy,
                log_primary_mass: math::log10(m.record().primary_initial_mass().value()),
                before: before.kind(),
            };
            (marks, m.weight())
        })
        .collect()
}

/// The stellar mergers' sampler against the engine (P11.T8.c): every mark both sides define, the
/// engine's weighted mergers binned in the catalogue's entries' deciles, by chi-square per degree
/// of freedom, and the share of the engine's mergers that were in contact before. The scratch
/// table is hand-made, not fitted to the engine (design note 13), so the tolerance is the scratch
/// one: each mark's statistic under [`SCRATCH_CHI_SQUARE_PER_DOF`].
#[test]
#[ignore = "slow: thousands of grid pairs run through the binary engine"]
fn the_merger_sampler_against_the_engine_s_mergers() {
    let galaxy = milky_way();
    let catalogue = catalogue_marks(&galaxy, 1_000);
    let started = std::time::Instant::now();
    let engine = engine_marks(&galaxy, MEMBERS);
    println!(
        "{} engine mergers in {:.1} s",
        engine.len(),
        started.elapsed().as_secs_f64()
    );
    assert_eq!(engine.len(), MEMBERS);
    let marks: [(&str, MarkOf); 3] = [
        ("log total mass", |m| m.log_total_mass),
        ("mass ratio", |m| m.mass_ratio),
        ("log primary mass", |m| m.log_primary_mass),
    ];
    for (name, f) in marks {
        let expected: Vec<f64> = catalogue.iter().map(f).collect();
        let observed: Vec<(f64, f64)> = engine.iter().map(|(m, w)| (f(m), *w)).collect();
        let (per_dof, effective) = decile_chi_square(&expected, &observed);
        let mut sorted = expected.clone();
        sorted.sort_by(f64::total_cmp);
        println!(
            "  {name}: chi2/dof {per_dof:.1} (effective {effective:.0} of {MEMBERS}); \
             median catalogue {:.3}",
            sorted[sorted.len() / 2]
        );
        assert!(per_dof < SCRATCH_CHI_SQUARE_PER_DOF, "{name}: {per_dof}");
    }
    let unweighted: Vec<(Marks, f64)> = engine.iter().map(|(m, _)| (*m, 1.0)).collect();
    for kind in [
        SegmentKind::Contact,
        SegmentKind::Detached,
        SegmentKind::StableTransfer {
            donor: Component::Primary,
        },
        SegmentKind::StableTransfer {
            donor: Component::Secondary,
        },
        SegmentKind::CommonEnvelope,
    ] {
        println!(
            "  {kind:?} before T: engine {:.3} weighted, {:.3} unweighted; catalogue {}",
            weighted_share(&engine, |m| m.before == kind),
            weighted_share(&unweighted, |m| m.before == kind),
            u8::from(kind == SegmentKind::Contact)
        );
    }
}

/// The engine's mergers drawn.
const MEMBERS: usize = 3_000;

/// The scratch table's tolerance, as P11.T8.a's: each mark's chi-square per degree of freedom at
/// the members' effective number in ten deciles, which refuses only a mark whose engine mergers
/// all but fall into one tenth of the catalogue's law. P15.T10.b's fit must bring every mark under
/// the 99.9% point, about 3.1 at nine degrees of freedom.
const SCRATCH_CHI_SQUARE_PER_DOF: f64 = 290.0;

/// A scan of every stellar merger in the galaxy, 9 × 10⁴ expected (fewer than the plan's 10⁶,
/// which no galaxy holds), runs the binary engine not once (`scan::engine_calls`); the realised
/// count is the expected one less the scan's main-sequence check, pinned as measured.
#[test]
#[ignore = "slow: every stellar merger's scan marks"]
fn a_scan_of_the_galaxy_s_mergers_runs_no_engine() {
    let galaxy = milky_way();
    let process = StellarMergers::new(&galaxy);
    let before = engine_calls();
    let started = std::time::Instant::now();
    let mut mergers = Vec::new();
    let mut scanned = 0_usize;
    for x in -16..16 {
        for y in -16..16 {
            for z in -16..16 {
                let key =
                    CatalogueCellKey::new(ClassId::STELLAR_MERGER, [x, y, z]).expect("a cell");
                mergers.clear();
                scan_merger_cell(&galaxy, &process, key, &mut mergers);
                scanned += mergers.len();
            }
        }
    }
    let expected = process.hosts().expected_hosts(&galaxy);
    #[expect(clippy::cast_precision_loss, reason = "a count of some 10⁵")]
    let ratio = scanned as f64 / expected;
    println!(
        "{scanned} mergers scanned in {:.1} s, {ratio:.4} of the {expected:.0} expected",
        started.elapsed().as_secs_f64()
    );
    assert_eq!(engine_calls(), before);
    assert!((0.9..=1.0).contains(&ratio), "{ratio}");
}
