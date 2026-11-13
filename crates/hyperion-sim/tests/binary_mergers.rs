//! Plan 11's P11.T8.c–d: the merger classes against their targets.
//!
//! - The stellar mergers' sampler against the engine's own mergers (slow): each mark both define,
//!   the catalogue's entries near the solar circle against `brute_force_merger_members`, weighted
//!   as mergers in the source horizon, by chi-square in the entries' deciles, under the scratch
//!   table's stated tolerance.
//! - A scan of every stellar merger of the galaxy, which runs the binary engine not once (slow).
//! - The neutron-star mergers' expected count at Milky Way parameters against the plan's 8–12, and
//!   ten Milky Way seeds' realised counts against it, through `assert_poisson_count` (slow).
//! - The neutron-star mergers' sampler against the engine's own (slow), as the stellar mergers'.

#![cfg(feature = "testing")]

#[expect(dead_code, reason = "the mergers use only the members")]
mod common;

use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::binary::{NeutronStarMergers, StellarMergers};
use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell, ClassId};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::math;
use hyperion_sim::stellar::Phase;
use hyperion_sim::stellar::binary::scan::{engine_calls, scan_merger_cell, scan_nsm_cell};
use hyperion_sim::stellar::binary::testing::MergerMember;
use hyperion_sim::stellar::binary::{CarvedClass, Component, SegmentKind};
use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

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
/// which no galaxy holds), runs the binary engine not once (`scan::engine_calls`, each thread's
/// own); the realised count is the expected one less the scan's main-sequence check, pinned as
/// measured. The cells are split by x between [`common::binaries::MEMBER_STREAMS`] threads.
#[test]
#[ignore = "slow: every stellar merger's scan marks"]
fn a_scan_of_the_galaxy_s_mergers_runs_no_engine() {
    let galaxy = milky_way();
    let process = StellarMergers::new(&galaxy);
    let started = std::time::Instant::now();
    let slice = |x0: i32| {
        let before = engine_calls();
        let mut mergers = Vec::new();
        let mut scanned = 0_usize;
        for x in [x0, x0 + 16] {
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
        (scanned, engine_calls() - before)
    };
    let mut scanned = 0_usize;
    #[cfg(not(target_family = "wasm"))]
    std::thread::scope(|scope| {
        let handles: Vec<_> = (-16..0).map(|x0| scope.spawn(move || slice(x0))).collect();
        for handle in handles {
            let (count, calls) = handle.join().expect("a slice's scan finishes");
            assert_eq!(calls, 0);
            scanned += count;
        }
    });
    #[cfg(target_family = "wasm")]
    for x0 in -16..0 {
        let (count, calls) = slice(x0);
        assert_eq!(calls, 0);
        scanned += count;
    }
    let expected = process.hosts().expected_hosts(&galaxy);
    #[expect(clippy::cast_precision_loss, reason = "a count of some 10⁵")]
    let ratio = scanned as f64 / expected;
    println!(
        "{scanned} mergers scanned in {:.1} s, {ratio:.4} of the {expected:.0} expected",
        started.elapsed().as_secs_f64()
    );
    assert_eq!(scanned, SCANNED_MERGERS);
}

/// The fixture's realised stellar mergers, as measured at generator version 31 on the fitted
/// merger tables (P15.T10.b): 75,361, 0.9093 of the 82,880 expected. At 27 and 28 (the same on
/// both trees), 85,075, 0.9489 of the 89,659 expected. At 25, 89,116, 0.9902 of the 9 × 10⁴
/// expected (89,106 at 24).
const SCANNED_MERGERS: usize = 75_361;

/// Every neutron-star merger cell of `galaxy`, scanned: the galaxy's realised mergers.
fn every_nsm(galaxy: &Galaxy) -> Vec<hyperion_sim::stellar::binary::scan::NsmScanMarks> {
    let process = NeutronStarMergers::new(galaxy);
    let mut mergers = Vec::new();
    for x in -16..16 {
        for y in -16..16 {
            for z in -16..16 {
                let key =
                    CatalogueCellKey::new(ClassId::NEUTRON_STAR_MERGER, [x, y, z]).expect("a cell");
                scan_nsm_cell(galaxy, &process, key, &mut mergers);
            }
        }
    }
    mergers
}

/// The neutron-star mergers at Milky Way parameters (P11.T8.d): each of ten Milky Way seeds
/// expects ten in the source horizon, inside the plan's 8–12 (the brainstorm's "about 10"), and
/// each seed's realised count, and the ten's sum, pass `assert_poisson_count` against it.
#[test]
#[ignore = "slow: ten galaxies' every merger cell"]
fn ten_milky_way_seeds_realise_their_neutron_star_mergers() {
    let mut realised = 0_u64;
    let mut expected = 0.0;
    for k in 0..10_u64 {
        let galaxy = Galaxy::from_params(Seed::new(NSM_SEEDS | k), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral");
        let mean = NeutronStarMergers::new(&galaxy)
            .hosts()
            .expected_hosts(&galaxy);
        assert!((8.0..=12.0).contains(&mean), "seed {k}: {mean}");
        let mergers = every_nsm(&galaxy);
        let count = u64::try_from(mergers.len()).expect("a few mergers");
        let pairs = mergers
            .iter()
            .filter(|m| m.kind() == hyperion_sim::tables::binary::NsmPairKind::DoubleNeutronStar)
            .count();
        println!("seed {k}: {count} mergers ({pairs} of two neutron stars) against {mean:.3}");
        assert!(
            mergers
                .iter()
                .all(|m| m.remnant_phase() == Phase::BlackHole)
        );
        assert_poisson_count(
            &format!("seed {k}'s neutron-star mergers"),
            count,
            mean,
            ALPHA,
        );
        realised += count;
        expected += mean;
    }
    println!("ten seeds: {realised} against {expected:.2}");
    assert_poisson_count("ten seeds' neutron-star mergers", realised, expected, ALPHA);
}

/// The seeds of the ten Milky Way galaxies, with the seed's number in the low bits.
const NSM_SEEDS: u64 = 0x0b17_08d0_0000_0100;

/// The neutron-star mergers' sampler against the engine (P11.T8.d): the engine's own mergers of
/// layer-E pairs near the solar circle, weighted as mergers in the source horizon, against the
/// catalogue's, in the pair's total mass by chi-square per degree of freedom under the scratch
/// tolerance, and in the shares of each kind of pair. The catalogue's mergers are the ten seeds'.
#[test]
#[ignore = "slow: layer-E grid pairs run through the binary engine"]
fn the_nsm_sampler_against_the_engine_s_mergers() {
    let mut catalogue = Vec::new();
    for k in 0..10_u64 {
        let galaxy = Galaxy::from_params(Seed::new(NSM_SEEDS | k), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral");
        catalogue.extend(every_nsm(&galaxy));
    }
    let galaxy = milky_way();
    let started = std::time::Instant::now();
    let members = common::binaries::brute_force_merger_members(
        &galaxy,
        CarvedClass::NeutronStarMerger,
        NSM_MEMBERS,
    );
    println!(
        "{} engine neutron-star mergers in {:.1} s",
        members.len(),
        started.elapsed().as_secs_f64()
    );
    assert_eq!(members.len(), NSM_MEMBERS);
    let expected: Vec<f64> = catalogue
        .iter()
        .map(|m| math::log10(m.total_mass().value()))
        .collect();
    let observed: Vec<(f64, f64)> = members
        .iter()
        .map(|m| {
            let mass: f64 = m.before().stars().iter().map(|s| s.mass().value()).sum();
            (math::log10(mass), m.weight())
        })
        .collect();
    let (per_dof, effective) = decile_chi_square(&expected, &observed);
    println!("  log total mass: chi2/dof {per_dof:.1} (effective {effective:.0} of {NSM_MEMBERS})");
    assert!(per_dof < SCRATCH_CHI_SQUARE_PER_DOF, "{per_dof}");
    let total: f64 = members.iter().map(MergerMember::weight).sum();
    let neutron_stars: f64 = members
        .iter()
        .filter(|m| {
            m.before()
                .stars()
                .iter()
                .all(|s| s.phase() == Phase::NeutronStar)
        })
        .map(MergerMember::weight)
        .sum();
    #[expect(clippy::cast_precision_loss, reason = "counts of some hundreds")]
    let catalogue_share = catalogue
        .iter()
        .filter(|m| m.kind() == hyperion_sim::tables::binary::NsmPairKind::DoubleNeutronStar)
        .count() as f64
        / catalogue.len() as f64;
    println!(
        "  two neutron stars: engine {:.3} weighted, catalogue {catalogue_share:.3} ({} mergers)",
        neutron_stars / total,
        catalogue.len()
    );
}

/// The engine's neutron-star mergers drawn.
const NSM_MEMBERS: usize = 600;
