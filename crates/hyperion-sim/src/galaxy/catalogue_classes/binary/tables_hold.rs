//! The tests of P15.T10.b's hold, [`BINARY_TABLES_VERSION`]: its tripwire; a galaxy under the
//! version's rules reads the scratch tables and one under the fitted arm the fitted ones, in its
//! processes and its class shares; the fitted tables' shapes; and, slow, the fitted rows' Milky
//! Way counts and each fitted sampler's marks against the engine's members by a two-sample
//! Kolmogorov–Smirnov test at 1%, over the half of a 10⁷-record census that `hyperion-fit`'s tasks
//! `binary_awd` and `binary_xrb` were not fitted to.
//!
//! The version in force is held bit for bit by the goldens: the held arm is the code of version 26
//! verbatim. The bump to 27 deletes the tripwire and the held arm, and these become tests of the
//! version's own galaxy.

use std::sync::OnceLock;

use hyperion_testkit::stats::ks_two_sample;
use hyperion_testkit::threads::in_shares;

use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell};
use crate::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
use crate::galaxy::imf::{BandShares, MassBand};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::{Galaxy, POPULATIONS, PointLy, Population};
use crate::math;
use crate::orbit::Eccentricity;
use crate::stellar::binary::CarvedClass;
use crate::stellar::binary::light_curves::XrayAccretor;
use crate::stellar::binary::sampler::ClassSampler;
use crate::stellar::binary::testing::XrayMarks;
use crate::stellar::binary::testing::{
    CensusPart, CensusRecords, CensusSite, ClassMember, class_census_in,
};
use crate::tables::binary::{AwdDonorKind, SamplerKey, XrbHostKind};
use crate::time::LIGHT_CROSSING_L;
use crate::units::consts::SECONDS_PER_DAY;

/// The Milky Way fixture's seed: `hyperion-fit`'s manifests `binary_awd.toml`'s and
/// `binary_xrb.toml`'s, so that the census here runs the cells the fits left.
const SEED: u64 = 0x0b17_15a0_0000_0001;

/// Every hold at the version in force but the binary tables', which are fitted.
fn fitted_rules() -> GalaxyRules {
    GalaxyRules {
        binary_tables: BinaryTableRules::Fitted,
        ..GalaxyRules::of_version(GENERATOR_VERSION)
    }
}

/// The fixture under the generator version's rules: the held arm.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// The fixture with the fitted tables.
fn fitted() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            fitted_rules(),
        )
        .expect("the Milky Way fixture builds")
    })
}

/// P15.T10.b's tables are held until the 26 → 27 bump: the rules in force read the scratch ones,
/// and so do 26's.
#[test]
fn the_binary_tables_are_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < BINARY_TABLES_VERSION,
        "the 26 → 27 bump puts P15.T10.b's fitted tables on the record: delete \
         `BINARY_TABLES_VERSION`, `BinaryTableRules` with its `Scratch` arm, \
         `GalaxyRules::binary_tables`, `Galaxy::with_rules`' handling of it and this module; refit \
         `binary_awd` and `binary_xrb` at 27 and let `AWD_FAST_SAMPLER_26`, \
         `AWD_SLOW_SAMPLER_26`, `XRB_SAMPLER_26` and `CLASS_SHARES_26` replace `AWD_SAMPLER`, \
         `XRB_SAMPLER` and `CLASS_SHARES`"
    );
    assert_eq!(held().rules().binary_tables, BinaryTableRules::Scratch);
    assert_eq!(
        GalaxyRules::of_version(GENERATOR_VERSION).binary_tables,
        BinaryTableRules::Scratch
    );
    assert_eq!(
        GalaxyRules::of_version(crate::GENERATOR_VERSION).binary_tables,
        BinaryTableRules::Scratch
    );
    assert_eq!(
        GalaxyRules::every_27_hold().binary_tables,
        BinaryTableRules::Fitted
    );
}

/// D3: a galaxy under the version's rules reads the scratch tables, in both classes' processes and
/// in its class shares; one under the fitted arm reads the fitted ones, each class its own
/// sampler, and is another galaxy. Each galaxy's class share of a population's band is its table's
/// carved hosts there over the band's systems (`FeatureShares::set_class_shares`), so the two
/// galaxies' shares stand to each other as their tables' carved hosts.
#[test]
fn a_galaxy_carries_the_version_s_binary_tables() {
    assert_ne!(held(), fitted());
    assert_eq!(BinaryTableRules::Scratch.class_shares(), &CLASS_SHARES);
    assert_eq!(BinaryTableRules::Fitted.class_shares(), &CLASS_SHARES_26);
    for side in [AwdSide::Fast, AwdSide::Slow] {
        let process = AwdProcess::new(held(), side);
        assert_eq!(process.sampler().table(), &AWD_SAMPLER, "{side:?}");
        let hosts = process.expected_hosts(held());
        assert!((hosts / 4.5e6 - 1.0).abs() < 1e-9, "{side:?}: {hosts}");
        let fitted_process = AwdProcess::new(fitted(), side);
        assert_eq!(
            fitted_process.sampler().table(),
            BinaryTableRules::Fitted.awd_sampler(side),
            "{side:?}"
        );
        assert_ne!(fitted_process.sampler().table(), &AWD_SAMPLER, "{side:?}");
    }
    assert_eq!(XrayBinaries::new(held()).sampler().table(), &XRB_SAMPLER);
    assert_eq!(
        XrayBinaries::new(fitted()).sampler().table(),
        &XRB_SAMPLER_26
    );
    assert_eq!(BinaryTableRules::Fitted.xrb_sampler(), &XRB_SAMPLER_26);
    let carved = |table: &ClassShareTable, population, band| {
        table.rows().iter().fold(0.0, |sum, row| {
            sum + row.hosts_per_formed_mass(population) * row.layer_share(band)
        })
    };
    for population in POPULATIONS {
        for band in [MassBand::C, MassBand::D, MassBand::E] {
            let held_share = held().feature_shares().class_share(population, band);
            let fitted_share = fitted().feature_shares().class_share(population, band);
            let expected = carved(&CLASS_SHARES_26, population, band)
                / carved(&CLASS_SHARES, population, band);
            assert!(
                (fitted_share - held_share * expected).abs() <= 1e-12 * held_share.abs(),
                "{population:?} {band:?}: {fitted_share} against {held_share} × {expected}"
            );
        }
    }
}

/// `Galaxy::with_rules` takes a galaxy to the fitted tables as a build under them does: the same
/// galaxy, its class shares installed again.
#[test]
fn with_rules_installs_the_fitted_class_shares_as_a_build_does() {
    let moved = held().clone().with_rules(fitted_rules());
    assert_eq!(&moved, fitted());
    assert_eq!(moved.feature_shares(), fitted().feature_shares());
}

/// The fitted tables are in shape: every residual finite and non-decreasing, every span and limit
/// ordered, the donors' kinds and the layers' shares summing to 1, the rates finite and not
/// negative; the fast sampler's recurrence lies all within L and the slow one's all beyond it, so
/// that each class draws over all its ranks; and the classes not yet fitted keep their scratch
/// rows.
#[test]
fn the_fitted_tables_are_in_shape() {
    let log_l = math::log10(LIGHT_CROSSING_L.as_julian_years_f64());
    for side in [AwdSide::Fast, AwdSide::Slow] {
        let table = BinaryTableRules::Fitted.awd_sampler(side);
        assert_eq!(table.marks().len(), AwdMark::ALL.len());
        for mark in AwdMark::ALL {
            let law = table.mark(mark);
            assert!(
                law.residual().iter().all(|r| r.is_finite()),
                "{side:?} {mark:?}"
            );
            assert!(
                law.residual().windows(2).all(|w| w[0] <= w[1]),
                "{side:?} {mark:?}"
            );
            let (lo, hi) = law.limits();
            assert!(lo <= hi, "{side:?} {mark:?}");
            if let Some(median) = law.median() {
                let (first, last) = median.span();
                assert!(first < last, "{side:?} {mark:?}");
                assert!(median.nodes().iter().all(|v| v.is_finite()));
            }
            assert_eq!(law.median().is_none(), mark == AwdMark::Recurrence);
        }
        let kinds = table.pick(AwdPick::DonorKind).weights();
        assert_eq!(kinds.len(), AwdDonorKind::ALL.len());
        assert!((kinds.iter().sum::<f64>() - 1.0).abs() < 1e-12, "{side:?}");
        let recurrence = table.mark(AwdMark::Recurrence).residual();
        match side {
            AwdSide::Fast => assert!(recurrence[16] <= log_l, "{}", recurrence[16]),
            AwdSide::Slow => assert!(recurrence[0] > log_l, "{}", recurrence[0]),
        }
        let process = AwdProcess::new(fitted(), side);
        assert_eq!(process.recurrence_ranks(), (0.0, 1.0), "{side:?}");
        let row = CLASS_SHARES_26.row(side.carved());
        let layers = MassBand::ALL
            .iter()
            .fold(0.0, |sum, &band| sum + row.layer_share(band));
        assert!((layers - 1.0).abs() < 1e-12, "{side:?}: {layers}");
        for population in POPULATIONS {
            let rate = row.hosts_per_formed_mass(population);
            assert!(rate.is_finite() && rate >= 0.0, "{side:?} {population:?}");
        }
    }
    for mark in XrbMark::ALL {
        let law = XRB_SAMPLER_26.mark(mark);
        assert_eq!(law.name(), XRB_SAMPLER.mark(mark).name());
        assert!(law.residual().iter().all(|r| r.is_finite()), "{mark:?}");
        assert!(law.residual().windows(2).all(|w| w[0] <= w[1]), "{mark:?}");
        let (lo, hi) = law.limits();
        assert!(lo <= hi, "{mark:?}");
        assert_eq!(
            law.median().is_some(),
            mark == XrbMark::LowMassDonorMass,
            "{mark:?}"
        );
    }
    let weights = |pick| XRB_SAMPLER_26.pick(pick).weights().iter().sum::<f64>();
    assert_eq!(XRB_SAMPLER_26.pick(XrbPick::LowMassKind).weights().len(), 4);
    assert_eq!(
        XRB_SAMPLER_26.pick(XrbPick::HighMassKind).weights().len(),
        2
    );
    let both = weights(XrbPick::LowMassKind) + weights(XrbPick::HighMassKind);
    assert!((both - 1.0).abs() < 1e-12, "{both}");
    let row = CLASS_SHARES_26.row(CarvedClass::XrayBinary);
    let layers = MassBand::ALL
        .iter()
        .fold(0.0, |sum, &band| sum + row.layer_share(band));
    assert!((layers - 1.0).abs() < 1e-12, "{layers}");
    for population in POPULATIONS {
        let rate = row.hosts_per_formed_mass(population);
        assert!(rate.is_finite() && rate >= 0.0, "{population:?}");
    }
    for class in [CarvedClass::StellarMerger, CarvedClass::NeutronStarMerger] {
        assert_eq!(
            CLASS_SHARES_26.row(class),
            CLASS_SHARES.row(class),
            "{class:?}"
        );
    }
    assert_eq!(AwdMark::Recurrence.index(), 0);
    assert_eq!(SamplerKey::index(AwdPick::DonorKind), 0);
}

/// The fitted rows at Milky Way parameters (slow): over ten Milky Way seeds (research-leans-10),
/// both classes' expected hosts inside the brainstorm's 6–12 × 10⁶; the two classes' split, the
/// hosts' density at the Sun-like point against Pala et al.'s (2020, MNRAS 494, 3799)
/// 4.8 × 10⁻⁶ pc⁻³ and the galaxy's novae a year, printed and pinned; and under one candidate in
/// ten that the site's thinning keeps refused by the scan or the class's test (P11.T8), in cells
/// about the solar circle.
#[test]
#[ignore = "slow: ten galaxies built"]
fn the_fitted_rows_give_the_milky_way_counts() {
    for k in 0..10_u64 {
        let galaxy = Galaxy::from_params_with_rules(
            Seed::new(0x0b17_08a0_0000_0100 | k),
            GalaxyParams::milky_way_like(),
            fitted_rules(),
        )
        .expect("the Milky Way fixture builds");
        let [fast, slow] = [AwdSide::Fast, AwdSide::Slow]
            .map(|side| AwdProcess::new(&galaxy, side).expected_hosts(&galaxy));
        println!(
            "Milky Way seed {k}: {:.4e} hosts, {:.4e} fast and {:.4e} slow",
            fast + slow,
            fast,
            slow
        );
        assert!(
            (6.0e6..=12.0e6).contains(&(fast + slow)),
            "{k}: {}",
            fast + slow
        );
    }
    let galaxy = fitted();
    let sun = PointLy::from(
        &GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the root cube"),
    );
    let density = [AwdSide::Fast, AwdSide::Slow]
        .into_iter()
        .map(|side| {
            AwdProcess::new(galaxy, side)
                .hosts_density(galaxy, &sun)
                .value()
        })
        .sum::<f64>()
        * LIGHT_YEARS_PER_PARSEC
        * LIGHT_YEARS_PER_PARSEC
        * LIGHT_YEARS_PER_PARSEC;
    println!(
        "{density:e} pc^-3 at the Sun-like point, {:.3} of Pala et al.'s 4.8e-6",
        density / 4.8e-6
    );
    assert!(
        (density / PINNED_LOCAL_DENSITY - 1.0).abs() < 1e-6,
        "{density}"
    );
    // The galaxy's novae a year: each class's hosts times the mean of 1 ÷ P_rec over its fitted
    // recurrence's ranks, at the midpoints of 10⁴ equal steps.
    let steps = 10_000_u32;
    let novae: f64 = [AwdSide::Fast, AwdSide::Slow]
        .into_iter()
        .map(|side| {
            let process = AwdProcess::new(galaxy, side);
            let sampler = process.sampler();
            let mean = (0..steps).fold(0.0, |sum, i| {
                let rank = (f64::from(i) + 0.5) / f64::from(steps);
                sum + math::exp10(-sampler.value_at(AwdMark::Recurrence, 0.0, rank))
            }) / f64::from(steps);
            process.expected_hosts(galaxy) * mean
        })
        .sum();
    println!("{novae:.1} novae a year, against the brainstorm's 30–50");
    assert!((novae / PINNED_NOVAE_A_YEAR - 1.0).abs() < 1e-6, "{novae}");
    let (mut thinned, mut kept) = (0_u32, 0_u32);
    for side in [AwdSide::Fast, AwdSide::Slow] {
        let process = AwdProcess::new(galaxy, side);
        for x in -3..3 {
            for z in [-1, 0] {
                let key = CatalogueCellKey::new(side.class(), [x, 50, z]).expect("a cell");
                let bound = process.bound(galaxy, &key);
                for index in 0..candidate_count(galaxy.seed(), &key, bound) {
                    let id = key.candidate_id(index, 0).expect("a candidate's ID");
                    thinned += u32::from(process.site(galaxy, &key, id, bound).is_some());
                }
                let entries = match side {
                    AwdSide::Fast => {
                        CatalogueClassCell::generate(galaxy, &AccretingWdFast::new(galaxy), key)
                            .into_entries()
                            .len()
                    }
                    AwdSide::Slow => {
                        CatalogueClassCell::generate(galaxy, &AccretingWdSlow::new(galaxy), key)
                            .into_entries()
                            .len()
                    }
                };
                kept += u32::try_from(entries).expect("a cell's few entries");
            }
        }
    }
    let refused = thinned - kept;
    println!("{refused} of {thinned} thinned candidates refused");
    assert!(thinned > 100, "{thinned}");
    assert!(
        f64::from(refused) < 0.1 * f64::from(thinned),
        "{refused} of {thinned}"
    );
}

/// The galaxy's novae a year under the fitted tables, as measured (at generator version 25, on its
/// refit; 1,159 at 24): 22–37 times the brainstorm's 30–50 (Shafter 2017, ApJ 834, 196, gives 50,
/// +31 −23), from the engine's fast hosts' short recurrences, which the fit carries over, and the
/// fast sampler's lowest sixteenth of ranks, which spreads its hosts down to five-year recurrences
/// (at 24 the engine's own members gave 435 a year; a finding).
const PINNED_NOVAE_A_YEAR: f64 = 1_104.966_535_226_393_3;

/// Both classes' hosts' density at the Sun-like point under the fitted rows, pc⁻³, as measured (at
/// generator version 25, on its refit).
const PINNED_LOCAL_DENSITY: f64 = 6.354_014_921_231_408e-6;

/// The census's records, layers C–E together: the fit's, `binary_awd.toml`'s.
const CENSUS_RECORDS: u64 = 10_000_000;

/// The parts every census's cells are cut into, the fit's; the fit takes the first half of them
/// and the tests here the second.
const CENSUS_PARTS: u64 = 64;

/// The threads the census runs on: the test's reservation in `.config/nextest.toml`.
const CENSUS_THREADS: usize = 16;

/// The bands a census runs, whose primaries can make a member.
const CENSUS_BANDS: [MassBand; 3] = [MassBand::C, MassBand::D, MassBand::E];

/// A census of `records` at `site` of the populations `whose`, on the parts the fits left: each
/// band's records in its share of the systems, the parts joined in order, on [`CENSUS_THREADS`]
/// threads.
fn census(
    galaxy: &Galaxy,
    site: CensusSite,
    whose: CensusRecords,
    records: u64,
) -> Vec<ClassMember> {
    let shares = BandShares::of(galaxy.mass_function());
    let in_bands = CENSUS_BANDS
        .iter()
        .fold(0.0, |sum, &band| sum + shares.share(band));
    let half = CENSUS_PARTS / 2;
    let jobs: Vec<(MassBand, u64)> = CENSUS_BANDS
        .iter()
        .flat_map(|&band| (half..CENSUS_PARTS).map(move |k| (band, k)))
        .collect();
    in_shares(jobs.len(), CENSUS_THREADS, |i| {
        let (band, k) = jobs[i];
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a record count under 2⁵³, rounded to a whole count"
        )]
        let in_band = (records as f64 * shares.share(band) / in_bands).round() as u64;
        class_census_in(
            galaxy,
            band,
            site,
            whose,
            in_band.div_ceil(half),
            CensusPart::new(k, CENSUS_PARTS),
        )
        .into_members()
    })
    .into_iter()
    .flatten()
    .collect()
}

/// What both sides define of an accreting white dwarf, one value a mark compared, in
/// [`AWD_COMPARED`]'s order, and the donor's kind.
#[derive(Debug, Clone, Copy)]
struct AwdValues {
    values: [f64; 8],
    donor: AwdDonorKind,
}

/// The marks compared, with their units.
const AWD_COMPARED: [&str; 8] = [
    "log recurrence (yr)",
    "white dwarf mass (M_sun)",
    "log rate (M_sun/yr)",
    "log period (h)",
    "log donor mass (M_sun)",
    "log primary initial mass (M_sun)",
    "log donor initial mass (M_sun)",
    "log age (yr)",
];

/// The greatest log₁₀ recurrence compared, years: the fitted slow sampler's cap, at which an
/// engine member's longer recurrence is held too.
const RECURRENCE_CAP: f64 = 10.0;

/// The engine's accreting white dwarf members of `side` among `members`.
fn engine_values(members: &[ClassMember], side: AwdSide) -> Vec<AwdValues> {
    members
        .iter()
        .filter(|m| m.class() == side.carved())
        .filter_map(|m| {
            let a = m.accreting()?;
            let values = [
                math::log10(a.recurrence.value()).min(RECURRENCE_CAP),
                a.white_dwarf_mass.value(),
                math::log10(a.transfer_rate.value()),
                math::log10(a.period?.value() / 3_600.0),
                math::log10(a.donor_mass.value()),
                math::log10(a.white_dwarf_initial_mass.value()),
                math::log10(a.donor_initial_mass.value()),
                math::log10(m.record().age_at_epoch().value()),
            ];
            values.iter().all(|v| v.is_finite()).then_some(AwdValues {
                values,
                donor: a.donor_kind(),
            })
        })
        .collect()
}

/// The fitted galaxy's entries of `side` in the slab of the census, its class cells of 512 ly
/// within 12,288 ly along x, from 24,064 to 28,160 ly along y and 1,536 ly along z.
fn catalogue_values(galaxy: &Galaxy, side: AwdSide) -> Vec<AwdValues> {
    let mut out = Vec::new();
    for x in -24..24 {
        for y in 47..55 {
            for z in -3..3 {
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
                out.extend(entries.iter().map(|entry| {
                    let value = entry.value();
                    let (scan, binary, history) = (value.scan(), value.binary(), value.history());
                    AwdValues {
                        values: [
                            math::log10(scan.recurrence().value()).min(RECURRENCE_CAP),
                            scan.white_dwarf_mass().value(),
                            math::log10(scan.transfer_rate().value()),
                            math::log10(binary.orbit().period().value() / 3_600.0),
                            math::log10(binary.donor().mass().value()),
                            math::log10(history.primary_initial_mass().value()),
                            math::log10(history.donor_initial_mass().value()),
                            math::log10(history.age_at_epoch().value()),
                        ],
                        donor: binary.donor_kind(),
                    }
                }));
            }
        }
    }
    out
}

/// The share of `values` whose donor is of `kind`.
fn kind_share(values: &[AwdValues], kind: AwdDonorKind) -> f64 {
    let n = values.iter().filter(|v| v.donor == kind).count();
    #[expect(clippy::cast_precision_loss, reason = "counts of thousands")]
    let share = n as f64 / values.len() as f64;
    share
}

/// The fitted samplers against the engine (P15.T10.b's acceptance; slow): for each class, each
/// mark both sides define, the fitted galaxy's entries in the census's slab against the engine's
/// members in the half of the census the fit left, by a two-sample Kolmogorov–Smirnov test at 1%.
/// A mark that misses is a finding, pinned at its measured distance in [`AWD_MISSES`].
#[test]
#[ignore = "slow: a census of 10⁷ records through the binary engine"]
fn the_accreting_white_dwarf_samplers_against_the_engine() {
    let members = census(
        held(),
        CensusSite::SOLAR_CIRCLE,
        CensusRecords::All,
        CENSUS_RECORDS,
    );
    let mut misses = Vec::new();
    for side in [AwdSide::Fast, AwdSide::Slow] {
        let engine = engine_values(&members, side);
        let catalogue = catalogue_values(fitted(), side);
        println!(
            "{side:?}: {} engine members, {} catalogue entries",
            engine.len(),
            catalogue.len()
        );
        for kind in AwdDonorKind::ALL {
            println!(
                "  {kind:?} donors: engine {:.4}, catalogue {:.4}",
                kind_share(&engine, kind),
                kind_share(&catalogue, kind)
            );
        }
        for (k, name) in AWD_COMPARED.iter().enumerate() {
            let mut a: Vec<f64> = catalogue.iter().map(|v| v.values[k]).collect();
            let mut b: Vec<f64> = engine.iter().map(|v| v.values[k]).collect();
            let ks = ks_two_sample(&mut a, &mut b);
            let median = |v: &[f64]| v[v.len() / 2];
            println!(
                "  {name}: D {}, p {:.3e}; median catalogue {:.4}, engine {:.4}",
                ks.statistic,
                ks.p_value,
                median(&a),
                median(&b)
            );
            let pinned = AWD_MISSES.iter().find(|m| m.0 == side && m.1 == k);
            let held_as_pinned = pinned.is_some_and(|m| (ks.statistic - m.2).abs() < 1e-6);
            if (pinned.is_none() && ks.p_value < KS_LEVEL) || (pinned.is_some() && !held_as_pinned)
            {
                misses.push(format!(
                    "{side:?} {name}: D {}, p {}",
                    ks.statistic, ks.p_value
                ));
            }
        }
    }
    assert!(misses.is_empty(), "{misses:#?}");
}

/// The level of the plan's Kolmogorov–Smirnov tests (P15.T10.b): 1%.
const KS_LEVEL: f64 = 0.01;

/// The marks whose Kolmogorov–Smirnov test misses at 1%, each a finding (the outbox's findings):
/// the class, the mark's place in [`AWD_COMPARED`] and its distance, as measured to a millionth.
/// Plan 11's shape, a residual of 17 ranks with one spread at every condition, holds the fitted
/// marks' distances to some 0.02–0.03 in the fit's own sample, and further where the spread
/// follows the condition: the slow hosts' periods and donor masses, whose helium donors' periods
/// run from under an hour to days. The fast hosts' ages follow too from the class's floor on the age,
/// [`LIFETIME_MARGIN`] times the primary's single-star lifetime, which the engine's binaries pass
/// under. At generator version 25, on the refitted tables, the fast hosts' primary and donor
/// initial masses pass at 1% (p 0.076 and 0.058) and are no longer pinned.
const AWD_MISSES: [(AwdSide, usize, f64); 13] = [
    (AwdSide::Fast, 1, 0.064_646_787_242_449_6),
    (AwdSide::Fast, 2, 0.061_621_222_487_845_85),
    (AwdSide::Fast, 3, 0.091_212_050_040_853_7),
    (AwdSide::Fast, 4, 0.086_297_947_756_192_48),
    (AwdSide::Fast, 7, 0.103_950_426_545_477_04),
    (AwdSide::Slow, 0, 0.020_076_105_497_966_124),
    (AwdSide::Slow, 1, 0.070_039_647_822_084_27),
    (AwdSide::Slow, 2, 0.060_175_646_102_016_23),
    (AwdSide::Slow, 3, 0.198_388_011_322_418_54),
    (AwdSide::Slow, 4, 0.198_949_608_541_687_03),
    (AwdSide::Slow, 5, 0.055_094_700_210_750_86),
    (AwdSide::Slow, 6, 0.107_057_831_651_133_58),
    (AwdSide::Slow, 7, 0.060_881_423_325_645_834),
];

/// The fitted X-ray binaries at Milky Way parameters (slow): over ten Milky Way seeds, the
/// class's expected hosts inside P11.T8.b's 0.7–1.5 × 10⁴ and its black-hole transients against
/// its 1,300 ± 400 (Corral-Santana et al. 2016, A&A 587, A61), pinned as measured, a finding: the
/// engine's black holes are 75% of its low-mass hosts (70% at 24) of the class's kinds, its neutron stars'
/// low-mass X-ray binaries being mostly ultracompact, which no kind holds; and the high-mass
/// hosts, the young thin disc's, printed.
#[test]
#[ignore = "slow: ten galaxies built"]
fn the_fitted_x_ray_binaries_give_the_milky_way_counts() {
    for k in 0..10_u64 {
        let galaxy = Galaxy::from_params_with_rules(
            Seed::new(0x0b17_08a0_0000_0100 | k),
            GalaxyParams::milky_way_like(),
            fitted_rules(),
        )
        .expect("the Milky Way fixture builds");
        let process = XrayBinaries::new(&galaxy);
        let total = process.expected_hosts(&galaxy);
        let transients = process.expected_hosts_of(&galaxy, XrbHostKind::BlackHoleTransient);
        let high = process.expected_hosts_on(&galaxy, XrbSide::HighMass);
        println!(
            "Milky Way seed {k}: {total:.4e} hosts, {transients:.1} black-hole transients, \
             {high:.1} high-mass"
        );
        assert!((0.7e4..=1.5e4).contains(&total), "{k}: {total}");
        assert!(
            (transients / PINNED_BLACK_HOLE_TRANSIENTS - 1.0).abs() < 1e-6,
            "{k}: {transients}"
        );
    }
}

/// The black-hole transients at Milky Way parameters under the fitted tables, as measured (at
/// generator version 25, on its refit).
const PINNED_BLACK_HOLE_TRANSIENTS: f64 = 7_536.849_840_543_357;

/// The young thin disc's own census of `hyperion-fit`'s `binary_xrb.toml`: its site and records.
const YOUNG_SITE: CensusSite = CensusSite::new([0, 0, 0], [40_000, 40_000, 600]);

/// The young thin disc's census's records, layers C–E together.
const YOUNG_RECORDS: u64 = 3_000_000;

/// The draws through the table a mark is compared on.
const DRAWS: u32 = 100_000;

/// The least members of the engine a mark is compared on: the fit's `min_members`.
const MIN_MEMBERS: usize = 50;

/// What an X-ray binary member of the engine gives `mark`, in the mark's unit, as the fit takes
/// it (`hyperion-fit`'s `tasks::binary_classes::xrb::xrb_pair`): its condition, the period's
/// log₁₀ in hours for the low-mass donor's mass and zero for the rest, and its value; `None` for a
/// member of no kind or of another, or where the mark is not drawn from the engine.
fn xrb_pair(x: &XrayMarks, mark: XrbMark) -> Option<(f64, f64)> {
    let kind = x.host_kind()?;
    let log = |v: f64| (v > 0.0).then(|| math::log10(v));
    let low = !kind.is_high_mass();
    let neutron = matches!(kind.accretor(), XrayAccretor::NeutronStar);
    let (bex, supergiant) = (kind == XrbHostKind::BeX, kind == XrbHostKind::Supergiant);
    let log_hours = x.period.and_then(|p| log(p.value() / 3_600.0));
    let log_days = x.period.and_then(|p| log(p.value() / SECONDS_PER_DAY));
    let recoil = x.recoil_speed.map(|v| v.value() / 1.0e3);
    let log_since = x.since_supernova.and_then(|y| log(y.value()));
    let only = |holds: bool, value: Option<f64>| if holds { Some((0.0, value?)) } else { None };
    let pair = match mark {
        XrbMark::NeutronStarMass => only(neutron, Some(x.accretor_mass.value())),
        XrbMark::BlackHoleMass => only(!neutron, Some(x.accretor_mass.value())),
        XrbMark::LowMassPeriod => only(low, log_hours),
        XrbMark::LowMassDonorMass => {
            if low {
                Some((log_hours?, log(x.donor_mass.value())?))
            } else {
                None
            }
        }
        XrbMark::LowMassRateExcess => only(low, x.rate_excess_dex),
        XrbMark::NeutronStarRecurrence | XrbMark::BlackHoleRecurrence => None,
        XrbMark::BexPeriod => only(bex, log_days),
        XrbMark::BexEccentricity => only(bex, x.eccentricity.map(Eccentricity::value)),
        XrbMark::BexDonorMass => only(bex, Some(x.donor_mass.value())),
        XrbMark::SupergiantPeriod => only(supergiant, log_days),
        XrbMark::SupergiantDonorMass => only(supergiant, Some(x.donor_mass.value())),
        XrbMark::SupergiantRate => only(supergiant, x.transfer_rate.and_then(|r| log(r.value()))),
        XrbMark::BexRecoil => only(bex, recoil),
        XrbMark::SupergiantRecoil => only(supergiant, recoil),
        XrbMark::BexSinceSupernova => only(bex, log_since),
        XrbMark::SupergiantSinceSupernova => only(supergiant, log_since),
        XrbMark::NeutronStarProgenitor => only(neutron, log(x.accretor_initial_mass.value())),
        XrbMark::BlackHoleProgenitor => only(!neutron, log(x.accretor_initial_mass.value())),
        XrbMark::DonorInitialExcess => only(
            low,
            log(x.donor_initial_mass.value() / x.donor_mass.value()),
        ),
    };
    pair.filter(|(c, v)| c.is_finite() && v.is_finite())
}

/// [`DRAWS`] values of `mark` drawn through the fitted sampler at evenly spread ranks, given
/// `conditions` drawn the same way for the low-mass donor's mass: hosts drawn through the table,
/// as the class's scan draws the mark.
fn table_draws(mark: XrbMark, conditions: &[f64]) -> Vec<f64> {
    let sampler = ClassSampler::new(&XRB_SAMPLER_26);
    (0..DRAWS)
        .map(|i| {
            let rank = (f64::from(i) + 0.5) / f64::from(DRAWS);
            let condition = conditions.get(usize::try_from(i).expect("a draw's index"));
            // The condition's ranks are the golden ratio's multiples, so that they and the mark's
            // are spread independently.
            let mixed = (f64::from(i) * 0.618_033_988_749_894_8).fract();
            match condition {
                Some(&c) => sampler.value_at(mark, c, mixed),
                None => sampler.value_at(mark, 0.0, rank),
            }
        })
        .collect()
}

/// The fitted X-ray binaries' sampler against the engine (P15.T10.b's acceptance; slow): for each
/// mark fitted from the engine, values drawn through the table against the engine's members of the
/// class's kinds in the half of the censuses the fit left, the slab's for the low-mass and history
/// marks and the young thin disc's for the high-mass ones, by a two-sample Kolmogorov–Smirnov test
/// at 1%. The class's hosts near the Sun are too few for a sample of its entries, so the table is
/// drawn directly, as its scan draws it. A mark that misses is a finding, pinned at its measured
/// distance in [`XRB_MISSES`]; a mark the engine gives under [`MIN_MEMBERS`] members, which the fit
/// kept as the scratch table's, is not compared.
#[test]
#[ignore = "slow: a census of 10⁷ records through the binary engine"]
fn the_x_ray_binary_sampler_against_the_engine() {
    let of_class = |members: Vec<ClassMember>| -> Vec<XrayMarks> {
        members
            .iter()
            .filter(|m| m.class() == CarvedClass::XrayBinary)
            .filter_map(|m| m.xray().copied())
            .collect()
    };
    let slab = of_class(census(
        held(),
        CensusSite::SOLAR_CIRCLE,
        CensusRecords::All,
        CENSUS_RECORDS,
    ));
    let young = of_class(census(
        held(),
        YOUNG_SITE,
        CensusRecords::Of(Population::YoungThinDisc),
        YOUNG_RECORDS,
    ));
    let of_kind = slab.iter().filter(|x| x.host_kind().is_some()).count();
    println!(
        "{} slab members, {of_kind} of a kind; {} young members",
        slab.len(),
        young.len()
    );
    let periods = table_draws(XrbMark::LowMassPeriod, &[]);
    let mut misses = Vec::new();
    for (k, mark) in XrbMark::ALL.into_iter().enumerate() {
        let high_mass = XRB_SAMPLER_26.mark(mark).name().contains("bex")
            || XRB_SAMPLER_26.mark(mark).name().contains("supergiant");
        let pool = if high_mass { &young } else { &slab };
        let mut engine: Vec<f64> = pool
            .iter()
            .filter_map(|x| xrb_pair(x, mark))
            .map(|p| p.1)
            .collect();
        let name = XRB_SAMPLER_26.mark(mark).name();
        if engine.len() < MIN_MEMBERS {
            println!("  {name}: {} engine members, not compared", engine.len());
            continue;
        }
        let conditions: &[f64] = if mark == XrbMark::LowMassDonorMass {
            &periods
        } else {
            &[]
        };
        let mut drawn = table_draws(mark, conditions);
        let ks = ks_two_sample(&mut drawn, &mut engine);
        println!(
            "  {name}: {} engine members, D {}, p {:.3e}; median table {:.4}, engine {:.4}",
            engine.len(),
            ks.statistic,
            ks.p_value,
            drawn[drawn.len() / 2],
            engine[engine.len() / 2]
        );
        let pinned = XRB_MISSES.iter().find(|m| m.0 == k);
        let held_as_pinned = pinned.is_some_and(|m| (ks.statistic - m.1).abs() < 1e-6);
        if (pinned.is_none() && ks.p_value < KS_LEVEL) || (pinned.is_some() && !held_as_pinned) {
            misses.push(format!("{name}: D {}, p {}", ks.statistic, ks.p_value));
        }
    }
    let kinds: Vec<String> = XrbHostKind::ALL
        .iter()
        .map(|&kind| {
            let n = slab.iter().filter(|x| x.host_kind() == Some(kind)).count();
            format!("{kind:?} {n}")
        })
        .collect();
    println!("  slab members by kind: {}", kinds.join(", "));
    assert!(misses.is_empty(), "{misses:#?}");
}

/// The X-ray binaries' marks whose Kolmogorov–Smirnov test misses at 1%, each a finding: the
/// mark's place in [`XrbMark`]'s order and its distance, as measured to a millionth. The two
/// unconditional laws' 17 ranks miss a tail of long periods and the black holes' heaviest
/// progenitors (the same two at generator version 25, on the refit).
const XRB_MISSES: [(usize, f64); 2] =
    [(2, 0.076_333_827_160_493_8), (18, 0.111_183_103_448_275_87)];
