//! Plan 11's P11.T8.a: the accreting white dwarf classes against their targets.
//!
//! - The local density of both classes' hosts at plan 03's `sunlike_point` against Pala et al.'s
//!   (2020, MNRAS 494, 3799) 4.8 × 10⁻⁶ pc⁻³ within 30%, pinned as the scratch share rows give it
//!   (a miss is a finding, and `CLASS_SHARES` is not retuned), and the galaxy's total against the
//!   brainstorm's 6–12 × 10⁶ over ten Milky Way seeds, plan 02's sweep printed as a record (slow).
//! - The sampler against the engine's own members (slow): each mark both define, the catalogue's
//!   entries near the solar circle against `brute_force_class_members`, by chi-square in the
//!   entries' deciles, under the scratch table's stated tolerance.
//! - A scan of 10⁶ hosts that runs the binary engine not once (slow).

#![cfg(feature = "testing")]

#[expect(
    dead_code,
    reason = "the classes use only the Sun-like point and the members"
)]
mod common;

use hyperion_sim::galaxy::catalogue_classes::binary::{
    AccretingWdFast, AccretingWdSlow, AwdProcess, AwdSide,
};
use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell};
use hyperion_sim::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::{Galaxy, PointLy};
use hyperion_sim::stellar::Phase;
use hyperion_sim::stellar::binary::scan::{engine_calls, scan_cell};
use hyperion_sim::stellar::binary::{BinaryClass, CvKind};
use hyperion_sim::tables::binary::{AwdDonorKind, MILKY_WAY_FORMED_MASS};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::{Seed, math};
use hyperion_testkit::pins::Tolerance;

/// The seed of plan 02's sweep's galaxy n is this with n in its low bits.
const SWEEP_SEED: u64 = 0x0200_5ee9_0000_0000;

/// The Milky Way fixture's seed.
const SEED: u64 = 0x0b17_08a0_0000_0002;

/// Pala et al.'s (2020, section 5 and table 4) space density of cataclysmic variables, pc⁻³:
/// 4.8 (+0.6, −0.8) × 10⁻⁶, the mid-plane density for a scale height of 280 pc, corrected for
/// their sample's 77% completeness; the Sun-like point is in the plane.
const PALA_DENSITY: f64 = 4.8e-6;

fn milky_way() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// Both classes' processes in `galaxy`.
fn processes(galaxy: &Galaxy) -> [AwdProcess; 2] {
    [AwdSide::Fast, AwdSide::Slow].map(|side| AwdProcess::new(galaxy, side))
}

/// The density of both classes' hosts at the Sun-like point, as the fitted rows give it
/// (P15.T10.b's refit at 27, each donor kind scaled to its observed figure): 5.28 × 10⁻⁶ pc⁻³,
/// 1.10 times Pala et al.'s figure, inside its 30% (the scratch share rows' 6.48 × 10⁻⁶, 1.35
/// times it, missed).
#[test]
fn the_local_density_at_the_sun_like_point() {
    let galaxy = milky_way();
    let point = PointLy::from(&common::sunlike_point(&galaxy));
    let density: f64 = processes(&galaxy)
        .iter()
        .map(|p| p.hosts_density(&galaxy, &point).value())
        .sum::<f64>()
        * LIGHT_YEARS_PER_PARSEC
        * LIGHT_YEARS_PER_PARSEC
        * LIGHT_YEARS_PER_PARSEC;
    let ratio = density / PALA_DENSITY;
    println!("{density:e} pc^-3 at the Sun-like point: {ratio:.4} of Pala et al.'s");
    assert!((ratio - 1.0).abs() < 0.3, "{ratio}");
    let mut pins = hyperion_testkit::pins!("binary_awd", "the_local_density_at_the_sun_like_point");
    pins.within("ratio", ratio, Tolerance::absolute(1e-5));
}

/// The galaxy's expected hosts, both classes, over ten Milky Way seeds (research-leans-10's reading
/// of the plan's "over ten seeds"): each inside the brainstorm's 6–12 × 10⁶.
///
/// The first ten galaxies of plan 02's sweep (`tests/galaxy_sweeps.rs`) are printed as a record,
/// each total over its formed mass's share of the Milky Way fixture's, which the scratch rows'
/// uniform rates make 9 × 10⁶ up to the populations' mix.
#[test]
#[ignore = "slow: twenty galaxies built"]
fn the_galaxy_total_over_ten_seeds() {
    for k in 0..10_u64 {
        let galaxy = Galaxy::from_params(
            Seed::new(MILKY_WAY_SEEDS | k),
            GalaxyParams::milky_way_like(),
        )
        .expect("the Milky Way fixture's gas is mostly neutral");
        let total: f64 = processes(&galaxy)
            .iter()
            .map(|p| p.expected_hosts(&galaxy))
            .sum();
        println!("Milky Way seed {k}: {total:.4e} hosts");
        assert!((6.0e6..=12.0e6).contains(&total), "{k}: {total}");
    }
    for k in 0..10_u64 {
        let galaxy = Galaxy::new(Seed::new(SWEEP_SEED | k));
        let total: f64 = processes(&galaxy)
            .iter()
            .map(|p| p.expected_hosts(&galaxy))
            .sum();
        let formed = galaxy
            .fields()
            .components()
            .iter()
            .fold(0.0, |sum, c| sum + c.count())
            * galaxy.mean_formed_mass().value();
        let per_milky_way = total / (formed / MILKY_WAY_FORMED_MASS);
        println!(
            "sweep seed {k}: {total:.4e} hosts, {per_milky_way:.4e} per Milky Way formed mass"
        );
        assert!(
            per_milky_way.is_finite() && per_milky_way > 0.0,
            "{k}: {per_milky_way}"
        );
    }
}

/// The family of the Milky Way seeds the totals are read over: this with k < 10 in its low bits.
const MILKY_WAY_SEEDS: u64 = 0x0b17_08a0_0000_0100;

/// What both sides define of an accreting white dwarf, one value per mark.
#[derive(Debug, Clone, Copy)]
struct Marks {
    white_dwarf_mass: f64,
    log_donor_mass: f64,
    log_period_hours: f64,
    log_rate: f64,
    log_recurrence: f64,
    magnetic: bool,
    donor: AwdDonorKind,
}

/// The kind of a donor of `phase`, as the catalogue's kinds name them.
fn donor_kind(phase: Phase) -> AwdDonorKind {
    match phase {
        Phase::HeliumWhiteDwarf
        | Phase::HeliumMainSequence
        | Phase::HeliumHertzsprungGap
        | Phase::HeliumGiantBranch => AwdDonorKind::Helium,
        Phase::FirstGiantBranch
        | Phase::CoreHeliumBurning
        | Phase::EarlyAgb
        | Phase::ThermallyPulsingAgb
        | Phase::PostAgb => AwdDonorKind::Giant,
        Phase::Protostar
        | Phase::PreMainSequence
        | Phase::MainSequence
        | Phase::Substellar
        | Phase::HertzsprungGap
        | Phase::CarbonOxygenWhiteDwarf
        | Phase::OxygenNeonWhiteDwarf
        | Phase::NeutronStar
        | Phase::BlackHole
        | Phase::NoRemnant => AwdDonorKind::MainSequence,
    }
}

/// The catalogue's entries of `side` in cells about the solar circle, within 2,500 ly of the
/// Sun-like point along y and out along x to 20,000 ly, the nearest columns first, in and below
/// the plane, until there are `n`. At 27 the fast hosts' 2,000 reach 16,000 ly along x and the
/// slow hosts' 2,000 ly; at 29, on the tables refitted on the generator's own pairs (P15.T10.b),
/// 16,000 ly holds 1,930 fast hosts, so the search runs on past it. Until 27 they lay within 5,000 ly, where 27's cells hold 678 fast hosts: the fitted
/// rows (P15.T10.b's refit at 27, each donor kind scaled to its observed figure) give the fast
/// hosts about a sixth of the scratch rows' rate at the solar circle (the old thin disc's
/// 7.6 × 10⁻⁶ hosts per solar mass formed against 4.8 × 10⁻⁵), and the features' lists hold part
/// of them (P11.T8.f).
fn catalogue_marks(galaxy: &Galaxy, side: AwdSide, n: usize) -> Vec<Marks> {
    let mut marks = Vec::with_capacity(n + 32);
    let cells = (0..40_i32)
        .flat_map(|step| {
            if step == 0 {
                vec![0]
            } else {
                vec![step, -step]
            }
        })
        .flat_map(|x| (46..=55).flat_map(move |y| [(x, y, 0), (x, y, -1)]));
    for (x, y, z) in cells {
        if marks.len() >= n {
            break;
        }
        let key = CatalogueCellKey::new(side.class(), [x, y, z]).expect("a cell");
        let entries = match side {
            AwdSide::Fast => {
                CatalogueClassCell::generate(galaxy, &AccretingWdFast::new(galaxy), key)
                    .into_entries()
            }
            AwdSide::Slow => {
                CatalogueClassCell::generate(galaxy, &AccretingWdSlow::new(galaxy), key)
                    .into_entries()
            }
        };
        marks.extend(entries.iter().map(|entry| {
            let value = entry.value();
            let scan = value.scan();
            let binary = value.binary();
            let magnetic = value
                .stars(galaxy)
                .state_at(UniverseTime::EPOCH)
                .is_some_and(|s| {
                    matches!(
                        s.pairs()[0].class(),
                        BinaryClass::CataclysmicVariable(CvKind::Magnetic)
                    )
                });
            Marks {
                white_dwarf_mass: scan.white_dwarf_mass().value(),
                log_donor_mass: math::log10(binary.donor().mass().value()),
                log_period_hours: math::log10(binary.orbit().period().value() / 3_600.0),
                log_rate: math::log10(scan.transfer_rate().value()),
                log_recurrence: math::log10(scan.recurrence().value()),
                magnetic,
                donor: binary.donor_kind(),
            }
        }));
    }
    assert!(
        marks.len() >= n,
        "{} entries about the solar circle",
        marks.len()
    );
    marks.truncate(n);
    marks
}

/// The engine's prior members of `side` near the solar circle.
fn engine_marks(galaxy: &Galaxy, side: AwdSide, n: usize) -> Vec<Marks> {
    common::binaries::brute_force_class_members(galaxy, side.carved(), n)
        .iter()
        .map(|m| {
            let a = m
                .accreting()
                .expect("an accreting white dwarf member's marks");
            Marks {
                white_dwarf_mass: a.white_dwarf_mass.value(),
                log_donor_mass: math::log10(a.donor_mass.value()),
                log_period_hours: a
                    .period
                    .map_or(f64::NAN, |p| math::log10(p.value() / 3_600.0)),
                log_rate: math::log10(a.transfer_rate.value()),
                log_recurrence: math::log10(a.recurrence.value()),
                magnetic: a.magnetic,
                donor: donor_kind(a.donor_phase),
            }
        })
        .collect()
}

/// A mark's value of one member.
type MarkOf = fn(&Marks) -> f64;

/// The share of `marks` that `f` holds for.
fn share(marks: &[Marks], f: impl Fn(&Marks) -> bool) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "counts of a few thousand")]
    let s = marks.iter().filter(|m| f(m)).count() as f64 / marks.len() as f64;
    s
}

/// The sampler against the engine (P11.T8): for each class, every mark both sides define, the
/// engine's prior members binned in the catalogue's entries' deciles, by chi-square per degree of
/// freedom, and the magnetic and donor shares. The scratch table is hand-made from observed
/// figures, not fitted to the engine (design note 13), so the tolerance is the scratch one: each
/// mark's statistic under [`SCRATCH_CHI_SQUARE_PER_DOF`].
#[test]
#[ignore = "slow: thousands of grid pairs run through the binary engine"]
fn the_sampler_against_the_engine_s_members() {
    let galaxy = milky_way();
    for side in [AwdSide::Fast, AwdSide::Slow] {
        let catalogue = catalogue_marks(&galaxy, side, 2_000);
        let started = std::time::Instant::now();
        let engine = engine_marks(&galaxy, side, MEMBERS);
        println!(
            "{side:?}: {} engine members in {:.1} s",
            engine.len(),
            started.elapsed().as_secs_f64()
        );
        assert_eq!(engine.len(), MEMBERS);
        let marks: [(&str, MarkOf); 5] = [
            ("white dwarf mass", |m| m.white_dwarf_mass),
            ("log donor mass", |m| m.log_donor_mass),
            ("log period", |m| m.log_period_hours),
            ("log rate", |m| m.log_rate),
            ("log recurrence", |m| m.log_recurrence),
        ];
        for (name, f) in marks {
            let expected: Vec<f64> = catalogue.iter().map(f).collect();
            let observed: Vec<f64> = engine.iter().map(f).collect();
            let (per_dof, dof, _) = common::binaries::decile_chi_square(&expected, &observed)
                .expect("hundreds of members on each side");
            let median = |v: &[f64]| {
                let mut s: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
                s.sort_by(f64::total_cmp);
                s[s.len() / 2]
            };
            println!(
                "  {name}: chi2/dof {per_dof:.1} ({dof} dof); median catalogue {:.3}, engine {:.3}",
                median(&expected),
                median(&observed)
            );
            assert!(
                per_dof < SCRATCH_CHI_SQUARE_PER_DOF,
                "{side:?} {name}: {per_dof}"
            );
        }
        for (name, f) in [
            ("magnetic", (|m: &Marks| m.magnetic) as fn(&Marks) -> bool),
            ("helium donor", |m| m.donor == AwdDonorKind::Helium),
            ("giant donor", |m| m.donor == AwdDonorKind::Giant),
        ] {
            println!(
                "  {name}: catalogue {:.3}, engine {:.3}",
                share(&catalogue, f),
                share(&engine, f)
            );
        }
    }
}

/// The engine's members drawn for each class.
const MEMBERS: usize = 300;

/// The scratch table's tolerance: each mark's chi-square per degree of freedom, at
/// [`MEMBERS`] members in ten deciles. Per degree of freedom the statistic is at most the members'
/// count, 300, reached when every member falls in one decile, so this refuses only a mark whose
/// engine members all but fall into one tenth of the catalogue's law. Measured at 600 members (2026-10-31): 52–509 over the
/// ten marks and classes, the fast hosts' periods the worst (the engine's median 18 hours against
/// the table's 3.4), so about half that at 300; P15.T10.b's fit must bring every mark under the
/// 99.9% point, about 3.1 at nine degrees of freedom. On the fitted tables at 27, at 300: 3.5–7.3
/// over the fast hosts' marks and 3.1–13.9 over the slow hosts', their donors' masses the worst
/// (the engine's median 0.030 M☉ against the table's 0.053), all but the slow hosts' periods past
/// that point (a finding).
const SCRATCH_CHI_SQUARE_PER_DOF: f64 = 290.0;

/// A scan of 10⁶ hosts of both classes, cells along the disc's plane from the solar circle
/// inwards, runs the binary engine not once (`scan::engine_calls`), and every slow host lists at
/// most two eruptions.
#[test]
#[ignore = "slow: 10⁶ hosts' scan marks"]
fn a_scan_of_a_million_hosts_runs_no_engine() {
    let galaxy = milky_way();
    let before = engine_calls();
    let mut hosts = Vec::new();
    let mut scanned = 0_usize;
    let started = std::time::Instant::now();
    'cells: for y in (0..51).rev() {
        for x in -24..24 {
            for z in [-1, 0] {
                for process in &processes(&galaxy) {
                    let key = CatalogueCellKey::new(process.class(), [x, y, z]).expect("a cell");
                    hosts.clear();
                    scan_cell(&galaxy, process, key, &mut hosts);
                    assert!(hosts.iter().all(|h| h.eruptions().len() <= 2));
                    scanned += hosts.len();
                    if scanned >= 1_000_000 {
                        break 'cells;
                    }
                }
            }
        }
    }
    println!(
        "{scanned} hosts scanned in {:.2} s",
        started.elapsed().as_secs_f64()
    );
    assert!(scanned >= 1_000_000, "{scanned}");
    assert_eq!(engine_calls(), before);
}
