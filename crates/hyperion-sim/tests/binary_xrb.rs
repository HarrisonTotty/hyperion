//! Plan 11's P11.T8.b: the X-ray binaries' class against its targets (slow).
//!
//! - The galaxy's expected hosts over ten Milky Way seeds against the plan's 0.7–1.5 × 10⁴, and
//!   the black-hole transients against its 1,300 ± 400 (Corral-Santana et al. 2016), as the scratch
//!   share row gives them (`CLASS_SHARES` is not retuned); plan 02's sweep printed as a record.
//! - The sampler against the engine's own members: each mark both define, the engine's members
//!   near the solar circle binned in the catalogue's entries' deciles, by chi-square, under the
//!   scratch table's stated tolerance, and the kinds' shares.
//! - A scan of the galaxy's hosts that runs the binary engine not once. The galaxy holds 10⁴
//!   hosts, so the plan's 10⁶ cannot be scanned in one; the scan covers them all but the far halo's.

#![cfg(feature = "testing")]

#[expect(dead_code, reason = "the class uses only the members")]
mod common;

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::binary::{XrayBinaries, XrbSide};
use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell, ClassId};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::orbit::Eccentricity;
use hyperion_sim::stellar::Phase;
use hyperion_sim::stellar::binary::scan::{engine_calls, scan_cell};
use hyperion_sim::stellar::binary::{BinaryClass, CarvedClass, HmxbKind, XrbKind};
use hyperion_sim::tables::binary::{MILKY_WAY_FORMED_MASS, XrbHostKind};
use hyperion_sim::{Seed, math};

/// The seed of plan 02's sweep's galaxy n is this with n in its low bits.
const SWEEP_SEED: u64 = 0x0200_5ee9_0000_0000;

/// The Milky Way fixture's seed.
const SEED: u64 = 0x0b17_08b0_0000_0002;

fn milky_way() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// The galaxy's expected hosts over ten Milky Way seeds (research-leans-10's reading of the plan's
/// "at Milky Way parameters"): each in the plan's 0.7–1.5 × 10⁴, its black-hole transients in its
/// 1,300 ± 400 and its high-mass share the table's 0.16. The first ten galaxies of plan 02's sweep
/// are printed as a record, each total over its formed mass's share of the Milky Way fixture's,
/// which the scratch row's uniform rates make 10⁴.
#[test]
#[ignore = "slow: twenty galaxies built"]
fn the_galaxy_total_over_ten_seeds() {
    for k in 0..10_u64 {
        let galaxy = Galaxy::from_params(
            Seed::new(MILKY_WAY_SEEDS | k),
            GalaxyParams::milky_way_like(),
        )
        .expect("the Milky Way fixture's gas is mostly neutral");
        let process = XrayBinaries::new(&galaxy);
        let total = process.expected_hosts(&galaxy);
        let black_hole_transients =
            process.expected_hosts_of(&galaxy, XrbHostKind::BlackHoleTransient);
        let high = process.expected_hosts_on(&galaxy, XrbSide::HighMass);
        println!(
            "Milky Way seed {k}: {total:.4e} hosts, {black_hole_transients:.0} black-hole \
             transients, {high:.0} high-mass"
        );
        assert!((7.0e3..=1.5e4).contains(&total), "{k}: {total}");
        assert!((900.0..=1_700.0).contains(&black_hole_transients), "{k}");
        assert!((high / total - 0.16).abs() < 1e-9);
    }
    for k in 0..10_u64 {
        let galaxy = Galaxy::new(Seed::new(SWEEP_SEED | k));
        let total = XrayBinaries::new(&galaxy).expected_hosts(&galaxy);
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
        assert!(per_milky_way.is_finite() && per_milky_way > 0.0);
    }
}

/// The family of the Milky Way seeds the totals are read over: this with k < 10 in its low bits.
const MILKY_WAY_SEEDS: u64 = 0x0b17_08b0_0000_0100;

/// What both sides define of an X-ray binary, one value per mark.
#[derive(Debug, Clone, Copy)]
struct Marks {
    class: BinaryClass,
    black_hole: bool,
    log_accretor_mass: f64,
    log_period_days: f64,
    log_donor_mass: f64,
    log_rate: f64,
    eccentricity: f64,
    recoil_km_s: f64,
    log_since_supernova: f64,
}

/// The catalogue's entries in the disc's plane, the cells within 41,000 ly of the centre in and just
/// below it: about half the galaxy's, in its mix of kinds.
fn catalogue_marks(galaxy: &Galaxy) -> Vec<Marks> {
    let process = XrayBinaries::new(galaxy);
    let mut marks = Vec::new();
    let cells = (-80..80).flat_map(|y| (-80..80).flat_map(move |x| [[x, y, 0], [x, y, -1]]));
    for cell in cells {
        let key = CatalogueCellKey::new(ClassId::XRAY_BINARY, cell).expect("a cell");
        let entries = CatalogueClassCell::generate(galaxy, &process, key).into_entries();
        marks.extend(entries.iter().map(|entry| {
            let value = entry.value();
            let scan = value.scan();
            let binary = value.binary();
            Marks {
                class: scan.kind().class(),
                black_hole: binary.accretor().phase() == Phase::BlackHole,
                log_accretor_mass: math::log10(scan.accretor_mass().value()),
                log_period_days: math::log10(scan.period().value() / 86_400.0),
                log_donor_mass: math::log10(binary.donor().mass().value()),
                log_rate: scan
                    .transfer_rate()
                    .map_or(f64::NAN, |r| math::log10(r.value())),
                eccentricity: binary.orbit().eccentricity().value(),
                recoil_km_s: scan
                    .recoil()
                    .map_or(f64::NAN, |r| r.velocity().speed().value() / 1.0e3),
                log_since_supernova: scan
                    .recoil()
                    .map_or(f64::NAN, |r| math::log10(r.since_supernova().value())),
            }
        }));
    }
    assert!(marks.len() > 4_000, "{} entries", marks.len());
    marks
}

/// The engine's prior members near the solar circle.
fn engine_marks(galaxy: &Galaxy, n: usize) -> Vec<Marks> {
    common::binaries::brute_force_class_members(galaxy, CarvedClass::XrayBinary, n)
        .iter()
        .map(|m| {
            let x = m.xray().expect("an X-ray binary member's marks");
            let high = matches!(x.class, BinaryClass::HighMassXrayBinary(_));
            Marks {
                class: x.class,
                black_hole: x.accretor_phase == Phase::BlackHole,
                log_accretor_mass: math::log10(x.accretor_mass.value()),
                log_period_days: x
                    .period
                    .map_or(f64::NAN, |p| math::log10(p.value() / 86_400.0)),
                log_donor_mass: math::log10(x.donor_mass.value()),
                log_rate: x.transfer_rate.map_or(f64::NAN, |r| math::log10(r.value())),
                eccentricity: x.eccentricity.map_or(f64::NAN, Eccentricity::value),
                recoil_km_s: if high {
                    x.recoil_speed.map_or(f64::NAN, |v| v.value() / 1.0e3)
                } else {
                    f64::NAN
                },
                log_since_supernova: if high {
                    x.since_supernova
                        .map_or(f64::NAN, |t| math::log10(t.value().max(1.0)))
                } else {
                    f64::NAN
                },
            }
        })
        .collect()
}

/// The share of `marks` that `f` holds for.
fn share(marks: &[Marks], f: impl Fn(&Marks) -> bool) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "counts of a few thousand")]
    let s = marks.iter().filter(|m| f(m)).count() as f64 / marks.len() as f64;
    s
}

/// The median of the finite values of `v`.
fn median(v: &[f64]) -> f64 {
    let mut s: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if s.is_empty() {
        return f64::NAN;
    }
    s.sort_by(f64::total_cmp);
    s[s.len() / 2]
}

/// A mark's value of one member.
type MarkOf = fn(&Marks) -> f64;

/// The sampler against the engine (P11.T8): every mark both sides define, the engine's prior
/// members binned in the catalogue's entries' deciles, by chi-square per degree of freedom, and
/// the kinds' shares. The scratch table is hand-made from observed figures, not fitted to the
/// engine (design note 13), so the tolerance is the scratch one: each mark's statistic under
/// [`SCRATCH_CHI_SQUARE_PER_DOF`] of its members.
#[test]
#[ignore = "slow: thousands of grid pairs run through the binary engine"]
fn the_sampler_against_the_engine_s_members() {
    let galaxy = milky_way();
    let catalogue = catalogue_marks(&galaxy);
    let started = std::time::Instant::now();
    let engine = engine_marks(&galaxy, MEMBERS);
    println!(
        "{} engine members in {:.1} s",
        engine.len(),
        started.elapsed().as_secs_f64()
    );
    assert_eq!(engine.len(), MEMBERS);
    let marks: [(&str, MarkOf); 6] = [
        ("log accretor mass", |m| m.log_accretor_mass),
        ("log period", |m| m.log_period_days),
        ("log donor mass", |m| m.log_donor_mass),
        ("log transfer rate", |m| m.log_rate),
        ("recoil", |m| m.recoil_km_s),
        ("log years since the supernova", |m| m.log_since_supernova),
    ];
    let mut compared = 0;
    for (name, f) in marks {
        let expected: Vec<f64> = catalogue.iter().map(f).collect();
        let observed: Vec<f64> = engine.iter().map(f).collect();
        let Some((per_dof, dof, count)) = common::binaries::decile_chi_square(&expected, &observed)
        else {
            println!("  {name}: too few engine members to compare");
            continue;
        };
        println!(
            "  {name}: chi2/dof {per_dof:.1} ({dof} dof, {count} members); median catalogue \
             {:.3}, engine {:.3}",
            median(&expected),
            median(&observed)
        );
        #[expect(
            clippy::cast_precision_loss,
            reason = "a member count of a few hundred"
        )]
        let bound = SCRATCH_CHI_SQUARE_PER_DOF * count as f64;
        assert!(per_dof < bound, "{name}: {per_dof} against {bound}");
        compared += 1;
    }
    // The pooled marks are compared; the recoil's two only if the engine gives high-mass members.
    assert!(compared >= 4, "{compared} marks compared");
    for (name, f) in [
        (
            "persistent low-mass",
            (|m: &Marks| m.class == BinaryClass::LowMassXrayBinary(XrbKind::Persistent))
                as fn(&Marks) -> bool,
        ),
        ("transient low-mass", |m| {
            m.class == BinaryClass::LowMassXrayBinary(XrbKind::Transient)
        }),
        ("symbiotic", |m| {
            m.class == BinaryClass::LowMassXrayBinary(XrbKind::Symbiotic)
        }),
        ("Be/X", |m| {
            m.class == BinaryClass::HighMassXrayBinary(HmxbKind::BeX)
        }),
        ("supergiant", |m| {
            m.class == BinaryClass::HighMassXrayBinary(HmxbKind::Supergiant)
        }),
        ("black hole", |m| m.black_hole),
        ("circular", |m| m.eccentricity < 1e-9),
    ] {
        println!(
            "  {name}: catalogue {:.3}, engine {:.3}",
            share(&catalogue, f),
            share(&engine, f)
        );
    }
    // The catalogue's kinds are the table's, whatever the engine's.
    assert!((share(&catalogue, |m| m.class.is_xray_binary()) - 1.0).abs() < 1e-12);
    let kinds: Vec<_> = XrbHostKind::ALL.iter().map(|k| k.class()).collect();
    assert!(catalogue.iter().all(|m| kinds.contains(&m.class)));
}

/// The engine's members drawn.
const MEMBERS: usize = 200;

/// The scratch table's tolerance, as a share of the members compared: each mark's chi-square per
/// degree of freedom under 0.97 of them, which refuses only a mark whose engine members all fall
/// into one tenth of the catalogue's law (the statistic per degree of freedom is then the members'
/// count). P15.T10.b's fit must bring every mark under the 99.9% point, about 3.1 at nine degrees
/// of freedom.
const SCRATCH_CHI_SQUARE_PER_DOF: f64 = 0.97;

/// A scan of every host of the galaxy, the cells within 51,200 ly of the centre along the plane and
/// 4,096 ly off it, runs the binary engine not once (`scan::engine_calls`); it finds the class's
/// expected hosts but those of the halo beyond, and every high-mass host carries its recoil.
#[test]
#[ignore = "slow: 640,000 cells scanned"]
fn a_scan_of_the_disc_s_hosts_runs_no_engine() {
    let galaxy = milky_way();
    let process = XrayBinaries::new(&galaxy);
    let before = engine_calls();
    let mut hosts = Vec::new();
    let started = std::time::Instant::now();
    for y in -100..100 {
        for x in -100..100 {
            for z in -8..8 {
                let key = CatalogueCellKey::new(ClassId::XRAY_BINARY, [x, y, z]).expect("a cell");
                scan_cell(&galaxy, &process, key, &mut hosts);
            }
        }
    }
    let high = hosts.iter().filter(|h| h.kind().is_high_mass()).count();
    println!(
        "{} hosts ({high} high-mass) scanned in {:.2} s, of {:.0} expected in the galaxy",
        hosts.len(),
        started.elapsed().as_secs_f64(),
        process.expected_hosts(&galaxy)
    );
    #[expect(clippy::cast_precision_loss, reason = "a count of some thousands")]
    let found = hosts.len() as f64 / process.expected_hosts(&galaxy);
    assert!((0.93..1.02).contains(&found), "{found}");
    assert!(
        hosts
            .iter()
            .all(|h| h.recoil().is_some() == h.kind().is_high_mass())
    );
    // The kinds the scan found on each side are the table's shares of that side, within five
    // standard deviations: the black-hole transients realised, not only expected.
    for kinds in [&XrbHostKind::LOW_MASS[..], &XrbHostKind::HIGH_MASS[..]] {
        let on_side: Vec<_> = hosts
            .iter()
            .filter(|h| h.kind().is_high_mass() == kinds[0].is_high_mass())
            .collect();
        let side_expected: f64 = kinds
            .iter()
            .map(|&k| process.expected_hosts_of(&galaxy, k))
            .sum();
        #[expect(clippy::cast_precision_loss, reason = "counts of some thousands")]
        let n = on_side.len() as f64;
        for &kind in kinds {
            let share = process.expected_hosts_of(&galaxy, kind) / side_expected;
            #[expect(clippy::cast_precision_loss, reason = "counts of some thousands")]
            let found = on_side.iter().filter(|h| h.kind() == kind).count() as f64;
            let sigma = (n * share * (1.0 - share)).sqrt();
            println!(
                "{kind:?}: {found} found, {:.1} expected of the side's {n}",
                n * share
            );
            assert!(
                (found - n * share).abs() < 5.0 * sigma + 1.0,
                "{kind:?}: {found}"
            );
        }
    }
    assert_eq!(engine_calls(), before);
}
