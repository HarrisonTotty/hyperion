//! The tests of P15.T10.b's hold, [`BINARY_TABLES_VERSION`]: its tripwire; a galaxy under the
//! version's rules reads the scratch tables and one under every 27 hold the fitted ones, refitted
//! at 27, in its processes and its class shares; the fitted tables' shapes; and, slow, the fitted
//! rows' Milky Way counts over ten seeds and each fitted sampler's marks against the engine's
//! members at 27 by a weighted two-sample Kolmogorov–Smirnov test, over the half of a 10⁷-record
//! census that `hyperion-fit`'s tasks `binary_awd` and `binary_xrb` were not fitted to.
//!
//! The tests are at the shape's resolution (P15.T10.b's refit, lean of 2026-11-02): a mark passes
//! where its distance from the engine's held-out half is within the 1% critical distance of its
//! distance from the half it was fitted to, its in-sample distance, which 17 ranks a law leave
//! even on the members they were fitted to. The engine's members are weighted so that their kinds
//! are in the table's shares, as the fits weight them.
//!
//! The version in force is held bit for bit by the goldens: the held arm is the code of version 26
//! verbatim. The bump to 27 deletes the tripwire and the held arm, and these become tests of the
//! version's own galaxy.

use std::sync::OnceLock;

use hyperion_testkit::stats::{KolmogorovSmirnov, ks_critical_distance, ks_two_sample_weighted};

use super::*;
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
use crate::planetary::testing::in_shares;
use crate::stellar::Phase;
use crate::stellar::binary::CarvedClass;
use crate::stellar::binary::light_curves::XrayAccretor;
use crate::stellar::binary::sampler::ClassSampler;
use crate::stellar::binary::testing::XrayMarks;
use crate::stellar::binary::testing::{
    CensusPart, CensusRecords, CensusSite, ClassMember, class_census_in,
};
use crate::tables::binary::{
    AWD_FAST_SAMPLER_27, AWD_SLOW_SAMPLER_27, AwdDonorKind, BLACK_HOLE_TRANSIENT_SHARE,
    CLASS_SHARES_27, SamplerKey, XrbHostKind,
};
use crate::time::LIGHT_CROSSING_L;
use crate::units::consts::SECONDS_PER_DAY;
use crate::{GENERATOR_VERSION, GeneratorVersion, Seed};

/// The Milky Way fixture's seed: `hyperion-fit`'s manifests `binary_awd.toml`'s and
/// `binary_xrb.toml`'s, so that the census here runs the cells the fits left.
const SEED: u64 = 0x0b17_15a0_0000_0001;

/// The refit's generator version: the manifests'.
const REFIT_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Every hold at 27, the fitted tables' among them.
const fn fitted_rules() -> GalaxyRules {
    GalaxyRules::every_27_hold()
}

/// The fixture under the generator version's rules: the held arm.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// The fits' census galaxy: every hold at 27 with the scratch tables
/// (`Galaxy::from_params_for_binary_refit`).
fn census_galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_for_binary_refit(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            REFIT_VERSION,
        )
        .expect("the Milky Way fixture builds")
    })
}

/// The fixture under every 27 hold, the fitted tables among them.
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
         `GalaxyRules::binary_tables`, `Galaxy::with_rules`' handling of it, \
         `Galaxy::from_params_for_binary_refit` and this module, and let `AWD_FAST_SAMPLER_27`, \
         `AWD_SLOW_SAMPLER_27`, `XRB_SAMPLER_27` and `CLASS_SHARES_27`, fitted at 27, replace \
         `AWD_SAMPLER`, `XRB_SAMPLER` and `CLASS_SHARES`"
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
    assert_eq!(fitted_rules().binary_tables, BinaryTableRules::Fitted);
    assert_eq!(
        census_galaxy().rules().binary_tables,
        BinaryTableRules::Scratch
    );
}

/// D3: a galaxy under the version's rules reads the scratch tables, in both classes' processes and
/// in its class shares; one under every 27 hold reads the fitted ones, each class its own
/// sampler, and is another galaxy. Each galaxy's class share of a population's band is its table's
/// carved hosts there over the band's systems (`FeatureShares::set_class_shares`), so the census
/// galaxy's shares and the fitted one's, every other hold alike, stand to each other as their
/// tables' carved hosts.
#[test]
fn a_galaxy_carries_the_version_s_binary_tables() {
    assert_ne!(held(), fitted());
    assert_eq!(BinaryTableRules::Scratch.class_shares(), &CLASS_SHARES);
    assert_eq!(BinaryTableRules::Fitted.class_shares(), &CLASS_SHARES_27);
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
    assert_eq!(
        BinaryTableRules::Fitted.awd_sampler(AwdSide::Fast),
        &AWD_FAST_SAMPLER_27
    );
    assert_eq!(
        BinaryTableRules::Fitted.awd_sampler(AwdSide::Slow),
        &AWD_SLOW_SAMPLER_27
    );
    assert_eq!(XrayBinaries::new(held()).sampler().table(), &XRB_SAMPLER);
    assert_eq!(
        XrayBinaries::new(fitted()).sampler().table(),
        &XRB_SAMPLER_27
    );
    assert_eq!(BinaryTableRules::Scratch.xrb_sampler(), &XRB_SAMPLER);
    assert_eq!(BinaryTableRules::Fitted.xrb_sampler(), &XRB_SAMPLER_27);
    let carved = |table: &ClassShareTable, population, band| {
        table.rows().iter().fold(0.0, |sum, row| {
            sum + row.hosts_per_formed_mass(population) * row.layer_share(band)
        })
    };
    for population in POPULATIONS {
        for band in [MassBand::C, MassBand::D, MassBand::E] {
            let scratch_share = census_galaxy()
                .feature_shares()
                .class_share(population, band);
            let fitted_share = fitted().feature_shares().class_share(population, band);
            let expected = carved(&CLASS_SHARES_27, population, band)
                / carved(&CLASS_SHARES, population, band);
            assert!(
                (fitted_share - scratch_share * expected).abs() <= 1e-12 * scratch_share.abs(),
                "{population:?} {band:?}: {fitted_share} against {scratch_share} × {expected}"
            );
        }
    }
}

/// `Galaxy::with_rules` takes a galaxy of the version in force to the fitted tables as a build
/// under them does: the same galaxy, its class shares installed again. From 26 the field share
/// reads the tables when the galaxy is built, so a galaxy at 27 is built under its tables instead.
#[test]
fn with_rules_installs_the_fitted_class_shares_as_a_build_does() {
    let rules = GalaxyRules {
        binary_tables: BinaryTableRules::Fitted,
        ..GalaxyRules::of_version(GENERATOR_VERSION)
    };
    let built =
        Galaxy::from_params_with_rules(Seed::new(SEED), GalaxyParams::milky_way_like(), rules)
            .expect("the Milky Way fixture builds");
    let moved = held().clone().with_rules(rules);
    assert_eq!(moved, built);
    assert_eq!(moved.feature_shares(), built.feature_shares());
}

/// Every law of `law` is finite and non-decreasing: its own and, if it has them, its nodes'.
fn assert_laws_in_shape(law: &crate::tables::binary::SamplerMark, what: &str) {
    let in_shape =
        |row: &[f64; 17]| row.iter().all(|r| r.is_finite()) && row.windows(2).all(|w| w[0] <= w[1]);
    assert!(in_shape(law.residual()), "{what}");
    if let Some(rows) = law.node_residuals() {
        for (k, row) in rows.iter().enumerate() {
            assert!(in_shape(row), "{what}, node {k}");
        }
    }
    let (lo, hi) = law.limits();
    assert!(lo <= hi, "{what}");
    if let Some(median) = law.median() {
        let (first, last) = median.span();
        assert!(first < last, "{what}");
        assert!(median.nodes().iter().all(|v| v.is_finite()), "{what}");
    }
}

/// The fitted tables are in shape: every residual law finite and non-decreasing, at every node
/// of a conditional mark, which has its node laws; every span and limit ordered; the donors' kinds
/// and the layers' shares summing to 1, the rates finite and not negative; the fast sampler's
/// recurrence lies all within L and the slow one's all beyond it, so that each class draws over
/// all its ranks; the X-ray binaries' kinds all nine, summing to 1, the black-hole transients'
/// pinned; and the classes not yet fitted keep their scratch rows.
#[test]
fn the_fitted_tables_are_in_shape() {
    let log_l = math::log10(LIGHT_CROSSING_L.as_julian_years_f64());
    for side in [AwdSide::Fast, AwdSide::Slow] {
        let table = BinaryTableRules::Fitted.awd_sampler(side);
        assert_eq!(table.marks().len(), AwdMark::ALL.len());
        for mark in AwdMark::ALL {
            let law = table.mark(mark);
            assert_laws_in_shape(law, &format!("{side:?} {mark:?}"));
            assert_eq!(law.median().is_none(), mark == AwdMark::Recurrence);
            assert_eq!(
                law.node_residuals().is_none(),
                mark == AwdMark::Recurrence,
                "{side:?} {mark:?}"
            );
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
        let row = CLASS_SHARES_27.row(side.carved());
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
        let law = XRB_SAMPLER_27.mark(mark);
        assert_eq!(law.name(), XRB_SAMPLER.mark(mark).name());
        assert_laws_in_shape(law, &format!("{mark:?}"));
        let conditional = mark == XrbMark::LowMassDonorMass;
        assert_eq!(law.median().is_some(), conditional, "{mark:?}");
        assert_eq!(law.node_residuals().is_some(), conditional, "{mark:?}");
    }
    let low = XRB_SAMPLER_27.pick(XrbPick::LowMassKind).weights();
    let high = XRB_SAMPLER_27.pick(XrbPick::HighMassKind).weights();
    assert_eq!(low.len(), XrbHostKind::LOW_MASS.len());
    assert_eq!(high.len(), XrbHostKind::HIGH_MASS.len());
    let both = low.iter().chain(high).sum::<f64>();
    assert!((both - 1.0).abs() < 1e-12, "{both}");
    let transients = XrbHostKind::LOW_MASS
        .iter()
        .position(|&k| k == XrbHostKind::BlackHoleTransient)
        .expect("a low-mass kind");
    assert!((low[transients] - BLACK_HOLE_TRANSIENT_SHARE).abs() < 1e-15);
    let row = CLASS_SHARES_27.row(CarvedClass::XrayBinary);
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
            CLASS_SHARES_27.row(class),
            CLASS_SHARES.row(class),
            "{class:?}"
        );
    }
    assert_eq!(AwdMark::Recurrence.index(), 0);
    assert_eq!(SamplerKey::index(AwdPick::DonorKind), 0);
}

/// Shafter's (2017, ApJ 834, 196) galactic nova rate, a year: 50, +31 −23, so 27–81.
const NOVAE_A_YEAR: (f64, f64) = (27.0, 81.0);

/// Pala et al.'s (2020, MNRAS 494, 3799) density of cataclysmic variables within 150 pc, pc⁻³.
const PALA_DENSITY: f64 = 4.8e-6;

/// The galaxy's novae a year in `galaxy`: each accreting class's hosts times the mean of 1 ÷ P
/// over its fitted recurrence's ranks on its side of L, at the midpoints of 10⁴ equal steps.
fn novae_a_year(galaxy: &Galaxy) -> f64 {
    let steps = 10_000_u32;
    [AwdSide::Fast, AwdSide::Slow]
        .into_iter()
        .map(|side| {
            let process = AwdProcess::new(galaxy, side);
            let sampler = process.sampler();
            let (lo, hi) = process.recurrence_ranks();
            let mean = (0..steps).fold(0.0, |sum, i| {
                let rank = lo + (hi - lo) * (f64::from(i) + 0.5) / f64::from(steps);
                sum + math::exp10(-sampler.value_at(AwdMark::Recurrence, 0.0, rank))
            }) / f64::from(steps);
            process.expected_hosts(galaxy) * mean
        })
        .sum()
}

/// The fitted rows at Milky Way parameters under every 27 hold (slow): over ten Milky Way seeds
/// (research-leans-10), both classes' expected hosts inside the brainstorm's 6–12 × 10⁶, their
/// density at the Sun-like point within 30% of Pala et al.'s 4.8 × 10⁻⁶ pc⁻³ (each donor kind is
/// scaled to its own figure, so the cataclysmic variables meet it and the AM Canum Venaticorum
/// stars add theirs), and the galaxy's novae a year inside Shafter's 27–81 ([`NOVAE_A_YEAR`]), the
/// brainstorm's 30–50 printed beside it; and under one candidate in ten that the site's thinning
/// keeps refused by the scan or the class's test (P11.T8), in cells about the solar circle.
#[test]
#[ignore = "slow: ten galaxies built"]
fn the_fitted_rows_give_the_milky_way_counts() {
    let sun = PointLy::from(
        &GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the root cube"),
    );
    let pc3 = LIGHT_YEARS_PER_PARSEC * LIGHT_YEARS_PER_PARSEC * LIGHT_YEARS_PER_PARSEC;
    for k in 0..10_u64 {
        let galaxy = Galaxy::from_params_with_rules(
            Seed::new(0x0b17_08a0_0000_0100 | k),
            GalaxyParams::milky_way_like(),
            fitted_rules(),
        )
        .expect("the Milky Way fixture builds");
        let [fast, slow] = [AwdSide::Fast, AwdSide::Slow]
            .map(|side| AwdProcess::new(&galaxy, side).expected_hosts(&galaxy));
        let density = [AwdSide::Fast, AwdSide::Slow]
            .into_iter()
            .map(|side| {
                AwdProcess::new(&galaxy, side)
                    .hosts_density(&galaxy, &sun)
                    .value()
            })
            .sum::<f64>()
            * pc3;
        let novae = novae_a_year(&galaxy);
        println!(
            "Milky Way seed {k}: {:.4e} hosts, {fast:.4e} fast and {slow:.4e} slow; {density:.3e} \
             pc^-3 at the Sun-like point, {:.3} of Pala et al.'s; {novae:.1} novae a year against \
             Shafter's 27-81 (the brainstorm's 30-50)",
            fast + slow,
            density / PALA_DENSITY
        );
        assert!(
            (6.0e6..=12.0e6).contains(&(fast + slow)),
            "{k}: {}",
            fast + slow
        );
        assert!((density / PALA_DENSITY - 1.0).abs() < 0.3, "{k}: {density}");
        assert!(
            (NOVAE_A_YEAR.0..=NOVAE_A_YEAR.1).contains(&novae),
            "{k}: {novae}"
        );
    }
    let galaxy = fitted();
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

/// The census's records, layers C–E together: the fit's, `binary_awd.toml`'s.
const CENSUS_RECORDS: u64 = 10_000_000;

/// The parts every census's cells are cut into, the fit's; the fit takes the first half of them
/// and the tests here the second.
const CENSUS_PARTS: u64 = 64;

/// The threads the census runs on: the test's reservation in `.config/nextest.toml`.
const CENSUS_THREADS: usize = 16;

/// The bands a census runs, whose primaries can make a member.
const CENSUS_BANDS: [MassBand; 3] = [MassBand::C, MassBand::D, MassBand::E];

/// The two halves of a census's parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Half {
    /// Parts 0–31, which the fits took.
    Fitted,
    /// Parts 32–63, which they left.
    Held,
}

/// A census of `records` at `site` of the populations `whose` in the census galaxy, on `half` of
/// the parts: each band's records in its share of the systems, as the fit cuts them, the parts
/// joined in order, on [`CENSUS_THREADS`] threads.
fn census(site: CensusSite, whose: CensusRecords, records: u64, half: Half) -> Vec<ClassMember> {
    let galaxy = census_galaxy();
    let shares = BandShares::of(galaxy.mass_function());
    let in_bands = CENSUS_BANDS
        .iter()
        .fold(0.0, |sum, &band| sum + shares.share(band));
    let parts = CENSUS_PARTS / 2;
    let first = match half {
        Half::Fitted => 0,
        Half::Held => parts,
    };
    let jobs: Vec<(MassBand, u64)> = CENSUS_BANDS
        .iter()
        .flat_map(|&band| (first..first + parts).map(move |k| (band, k)))
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
            in_band.div_ceil(parts),
            CensusPart::new(k, CENSUS_PARTS),
        )
        .into_members()
    })
    .into_iter()
    .flatten()
    .collect()
}

/// The level of the plan's Kolmogorov–Smirnov tests (P15.T10.b): 1%.
const KS_LEVEL: f64 = 0.01;

/// `values`, each of a kind, weighted so that each kind's share of their weight is its share in
/// `shares` among the kinds present, as `hyperion-fit`'s `weighted_to_shares` weights a fit's
/// members; a kind of no share drops out.
fn weighted_to(values: &[(f64, usize)], shares: &[f64]) -> Vec<(f64, f64)> {
    let mut counts = vec![0_u32; shares.len()];
    for &(_, k) in values {
        counts[k] += 1;
    }
    let n = f64::from(u32::try_from(values.len()).expect("a sample under 2³²"));
    values
        .iter()
        .filter(|(_, k)| shares[*k] > 0.0)
        .map(|&(v, k)| (v, shares[k] * n / f64::from(counts[k])))
        .collect()
}

/// A mark's comparison at the shape's resolution: the distances of the table's sample from the
/// engine's held-out half and from its fitted half, and the 1% critical distance of the first.
#[derive(Debug, Clone, Copy)]
struct Resolved {
    held_out: KolmogorovSmirnov,
    in_sample: f64,
    critical: f64,
}

impl Resolved {
    /// The comparison of `table` against the engine's `held_out` and `fitted` halves, each
    /// `(value, weight)` pairs.
    fn of(table: &[(f64, f64)], held_out: &[(f64, f64)], fitted: &[(f64, f64)]) -> Self {
        let held_out = ks_two_sample_weighted(&mut table.to_vec(), &mut held_out.to_vec());
        let in_sample = ks_two_sample_weighted(&mut table.to_vec(), &mut fitted.to_vec());
        Self {
            held_out,
            in_sample: in_sample.statistic,
            critical: ks_critical_distance(held_out.effective_n, KS_LEVEL),
        }
    }

    /// Whether the held-out distance is within the critical distance of the in-sample one.
    fn passes(&self) -> bool {
        self.held_out.statistic <= self.critical + self.in_sample
    }
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

/// `kind`'s place in [`AwdDonorKind::ALL`].
fn donor_index(kind: AwdDonorKind) -> usize {
    AwdDonorKind::ALL
        .iter()
        .position(|&k| k == kind)
        .expect("every kind is in `AwdDonorKind::ALL`")
}

/// Mark `k`'s values of `values`, weighted so that their donors are in `shares`.
fn awd_weighted(values: &[AwdValues], k: usize, shares: &[f64]) -> Vec<(f64, f64)> {
    let of_kind: Vec<(f64, usize)> = values
        .iter()
        .map(|v| (v.values[k], donor_index(v.donor)))
        .collect();
    weighted_to(&of_kind, shares)
}

/// The fitted samplers against the engine at 27 (P15.T10.b's acceptance; slow): for each class,
/// each mark both sides define, the fitted galaxy's entries in the census's slab against the
/// engine's members, weighted so that their donor kinds are in the table's shares, at the shape's
/// resolution (module documentation). A mark that misses is a finding, pinned at its measured
/// distance in [`AWD_MISSES`]. And the fast table's mean of 1 ÷ P, the novae a host, within 10% of
/// its held-out engine members'.
#[test]
#[ignore = "slow: two censuses of 10⁷ records through the binary engine"]
fn the_accreting_white_dwarf_samplers_against_the_engine() {
    let halves = [Half::Held, Half::Fitted].map(|half| {
        census(
            CensusSite::SOLAR_CIRCLE,
            CensusRecords::All,
            CENSUS_RECORDS,
            half,
        )
    });
    let mut misses = Vec::new();
    for side in [AwdSide::Fast, AwdSide::Slow] {
        let [held_out, fitted_half] = [&halves[0], &halves[1]].map(|m| engine_values(m, side));
        let catalogue = catalogue_values(fitted(), side);
        let table = BinaryTableRules::Fitted.awd_sampler(side);
        let shares = table.pick(AwdPick::DonorKind).weights();
        println!(
            "{side:?}: {} held-out and {} fitted engine members, {} catalogue entries",
            held_out.len(),
            fitted_half.len(),
            catalogue.len()
        );
        for kind in AwdDonorKind::ALL {
            println!(
                "  {kind:?} donors: engine {:.4}, table {:.4}, catalogue {:.4}",
                kind_share(&held_out, kind),
                shares[donor_index(kind)],
                kind_share(&catalogue, kind)
            );
        }
        for (k, name) in AWD_COMPARED.iter().enumerate() {
            let drawn: Vec<(f64, f64)> = catalogue.iter().map(|v| (v.values[k], 1.0)).collect();
            let r = Resolved::of(
                &drawn,
                &awd_weighted(&held_out, k, shares),
                &awd_weighted(&fitted_half, k, shares),
            );
            println!(
                "  {name}: D {} (p {:.3e}) against {:.6} + in-sample {:.6}",
                r.held_out.statistic, r.held_out.p_value, r.critical, r.in_sample
            );
            let pinned = AWD_MISSES.iter().find(|m| m.0 == side && m.1 == k);
            let held_as_pinned = pinned.is_some_and(|m| (r.held_out.statistic - m.2).abs() < 1e-6);
            if (pinned.is_none() && !r.passes()) || (pinned.is_some() && !held_as_pinned) {
                misses.push(format!(
                    "{side:?} {name}: D {} against {} + {}",
                    r.held_out.statistic, r.critical, r.in_sample
                ));
            }
        }
        if side == AwdSide::Fast {
            let novae = |values: &[(f64, f64)]| {
                let total = values.iter().fold(0.0, |sum, v| sum + v.1);
                values
                    .iter()
                    .fold(0.0, |sum, &(x, w)| sum + w * math::exp10(-x))
                    / total
            };
            let engine = novae(&awd_weighted(&held_out, 0, shares));
            let process = AwdProcess::new(fitted(), side);
            let steps = 10_000_u32;
            let (lo, hi) = process.recurrence_ranks();
            let sampler = process.sampler();
            let drawn = (0..steps).fold(0.0, |sum, i| {
                let rank = lo + (hi - lo) * (f64::from(i) + 0.5) / f64::from(steps);
                sum + math::exp10(-sampler.value_at(AwdMark::Recurrence, 0.0, rank))
            }) / f64::from(steps);
            println!(
                "  1/P a fast host: table {drawn:.4e}, engine {engine:.4e}, {:.3}",
                drawn / engine
            );
            assert!(
                (drawn / engine - 1.0).abs() < 0.1,
                "{drawn} against {engine}"
            );
        }
    }
    assert!(misses.is_empty(), "{misses:#?}");
}

/// The marks whose Kolmogorov–Smirnov test misses at the shape's resolution, each a finding (the
/// outbox's findings): the class, the mark's place in [`AWD_COMPARED`] and its held-out distance,
/// as measured to a millionth.
const AWD_MISSES: [(AwdSide, usize, f64); 0] = [];

/// Fortin et al.'s (2023, A&A 671, A149) catalogue of the Galaxy's high-mass X-ray binaries: 152
/// (recalled; the pod reads no web, so not read at source).
const CATALOGUED_HIGH_MASS: f64 = 152.0;

/// Corral-Santana et al.'s (2016, A&A 587, A61) black-hole transients, as P11.T8.b takes them:
/// 1,300 ± 400.
const BLACK_HOLE_TRANSIENTS: (f64, f64) = (1_300.0, 400.0);

/// The fitted X-ray binaries at Milky Way parameters under every 27 hold (slow): over ten Milky
/// Way seeds, the class's expected hosts inside P11.T8.b's 0.7–1.5 × 10⁴, its black-hole
/// transients inside 1,300 ± 400 ([`BLACK_HOLE_TRANSIENTS`]), and its high-mass hosts, the young
/// thin disc's, at least the catalogued ([`CATALOGUED_HIGH_MASS`]).
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
             {high:.1} high-mass against Fortin et al.'s {CATALOGUED_HIGH_MASS}"
        );
        assert!((0.7e4..=1.5e4).contains(&total), "{k}: {total}");
        let (transients_wanted, spread) = BLACK_HOLE_TRANSIENTS;
        assert!(
            (transients - transients_wanted).abs() <= spread,
            "{k}: {transients}"
        );
        assert!(high >= CATALOGUED_HIGH_MASS, "{k}: {high}");
    }
}

/// The young thin disc's own census of `hyperion-fit`'s `binary_xrb.toml`: its site and records.
const YOUNG_SITE: CensusSite = CensusSite::new([0, 0, 0], [40_000, 40_000, 600]);

/// The young thin disc's census's records, layers C–E together.
const YOUNG_RECORDS: u64 = 3_000_000;

/// The draws through the table a mark is compared on.
const DRAWS: u32 = 100_000;

/// The least members of the engine a mark is compared on, in each half: the fit's `min_members`.
const MIN_MEMBERS: usize = 50;

/// What an X-ray binary member of the engine gives `mark`, as the fit takes it at 27
/// (`hyperion-fit`'s `tasks::binary_classes::xrb::xrb_pair`): its condition, the period's log₁₀ in
/// hours for the low-mass donor's mass and zero for the rest, and its value, for a member of a
/// kind the class draws the mark for, its compact star its kind's where the mark is the compact
/// star's; `None` otherwise, or where the mark is not drawn from the engine.
fn xrb_pair(x: &XrayMarks, mark: XrbMark) -> Option<(f64, f64)> {
    use XrbHostKind::{
        BeX, BlackHoleHighMass, BlackHolePersistent, BlackHoleTransient, NeutronStarPersistent,
        NeutronStarTransient, Supergiant, Symbiotic, Ultracompact,
    };
    let kind = x.member_kind()?;
    let log = |v: f64| (v > 0.0).then(|| math::log10(v));
    let own_compact = match kind.accretor() {
        XrayAccretor::NeutronStar => x.accretor_phase == Phase::NeutronStar,
        XrayAccretor::BlackHole => x.accretor_phase == Phase::BlackHole,
    };
    let hydrogen_low = matches!(
        kind,
        NeutronStarPersistent | NeutronStarTransient | BlackHolePersistent | BlackHoleTransient
    );
    let log_minutes = x.period.and_then(|p| log(p.value() / 60.0));
    let log_hours = x.period.and_then(|p| log(p.value() / 3_600.0));
    let log_days = x.period.and_then(|p| log(p.value() / SECONDS_PER_DAY));
    let log_rate = x.transfer_rate.and_then(|r| log(r.value()));
    let recoil = x.recoil_speed.map(|v| v.value() / 1.0e3);
    let log_since = x.since_supernova.and_then(|y| log(y.value()));
    let only = |holds: bool, value: Option<f64>| if holds { Some((0.0, value?)) } else { None };
    let accretor = Some(x.accretor_mass.value());
    let pair = match mark {
        XrbMark::NeutronStarMass => only(
            own_compact
                && matches!(
                    kind,
                    NeutronStarPersistent
                        | NeutronStarTransient
                        | BeX
                        | Supergiant
                        | Ultracompact
                        | Symbiotic
                ),
            accretor,
        ),
        XrbMark::BlackHoleMass => only(
            own_compact && matches!(kind, BlackHolePersistent | BlackHoleTransient),
            accretor,
        ),
        XrbMark::BlackHoleHighMassAccretorMass => {
            only(own_compact && kind == BlackHoleHighMass, accretor)
        }
        XrbMark::LowMassPeriod => only(hydrogen_low, log_hours),
        XrbMark::LowMassDonorMass => {
            if hydrogen_low {
                Some((log_hours?, log(x.donor_mass.value())?))
            } else {
                None
            }
        }
        XrbMark::LowMassRateExcess => only(hydrogen_low, x.rate_excess_dex),
        XrbMark::NeutronStarRecurrence
        | XrbMark::BlackHoleRecurrence
        | XrbMark::UltracompactRecurrence
        | XrbMark::SymbioticFilling => None,
        XrbMark::BexPeriod => only(kind == BeX, log_days),
        XrbMark::BexEccentricity => only(kind == BeX, x.eccentricity.map(Eccentricity::value)),
        XrbMark::BexDonorMass => only(kind == BeX, Some(x.donor_mass.value())),
        XrbMark::BexRecoil => only(kind == BeX, recoil),
        XrbMark::BexSinceSupernova => only(kind == BeX, log_since),
        XrbMark::SupergiantPeriod => only(kind == Supergiant, log_days),
        XrbMark::SupergiantDonorMass => only(kind == Supergiant, Some(x.donor_mass.value())),
        XrbMark::SupergiantRate => only(kind == Supergiant, log_rate),
        XrbMark::SupergiantRecoil => only(matches!(kind, Supergiant | BlackHoleHighMass), recoil),
        XrbMark::SupergiantSinceSupernova => {
            only(matches!(kind, Supergiant | BlackHoleHighMass), log_since)
        }
        XrbMark::BlackHoleHighMassPeriod => only(kind == BlackHoleHighMass, log_days),
        XrbMark::BlackHoleHighMassDonorMass => {
            only(kind == BlackHoleHighMass, Some(x.donor_mass.value()))
        }
        XrbMark::BlackHoleHighMassRate => only(kind == BlackHoleHighMass, log_rate),
        XrbMark::UltracompactPeriod => only(kind == Ultracompact, log_minutes),
        XrbMark::UltracompactDonorInitialMass => {
            only(kind == Ultracompact, log(x.donor_initial_mass.value()))
        }
        XrbMark::SymbioticPeriod => only(kind == Symbiotic, log_days),
        XrbMark::SymbioticDonorMass => only(kind == Symbiotic, log(x.donor_mass.value())),
        XrbMark::NeutronStarProgenitor => only(
            own_compact && kind.accretor() == XrayAccretor::NeutronStar,
            log(x.accretor_initial_mass.value()),
        ),
        XrbMark::BlackHoleProgenitor => only(
            own_compact && kind.accretor() == XrayAccretor::BlackHole,
            log(x.accretor_initial_mass.value()),
        ),
        XrbMark::DonorInitialExcess => only(
            hydrogen_low || kind == Symbiotic,
            log(x.donor_initial_mass.value() / x.donor_mass.value()),
        ),
    };
    pair.filter(|(c, v)| c.is_finite() && v.is_finite())
}

/// `kind`'s place in [`XrbHostKind::ALL`].
fn xrb_index(kind: XrbHostKind) -> usize {
    XrbHostKind::ALL
        .iter()
        .position(|&k| k == kind)
        .expect("every kind is in `XrbHostKind::ALL`")
}

/// The fitted table's kind weights, in [`XrbHostKind::ALL`]'s order.
fn xrb_table_weights() -> [f64; 9] {
    let mut weights = [0.0; 9];
    let low = XRB_SAMPLER_27.pick(XrbPick::LowMassKind).weights();
    let high = XRB_SAMPLER_27.pick(XrbPick::HighMassKind).weights();
    for (kind, &w) in XrbHostKind::LOW_MASS.iter().zip(low) {
        weights[xrb_index(*kind)] = w;
    }
    for (kind, &w) in XrbHostKind::HIGH_MASS.iter().zip(high) {
        weights[xrb_index(*kind)] = w;
    }
    weights
}

/// Mark `mark`'s values of `pool`, weighted so that their kinds are in the table's weights.
fn xrb_weighted(pool: &[XrayMarks], mark: XrbMark, weights: &[f64; 9]) -> Vec<(f64, f64)> {
    let of_kind: Vec<(f64, usize)> = pool
        .iter()
        .filter_map(|x| Some((xrb_pair(x, mark)?.1, xrb_index(x.member_kind()?))))
        .collect();
    weighted_to(&of_kind, weights)
}

/// [`DRAWS`] values of `mark` drawn through the fitted sampler at evenly spread ranks, given
/// `conditions` drawn the same way for the low-mass donor's mass: hosts drawn through the table,
/// as the class's scan draws the mark.
fn table_draws(mark: XrbMark, conditions: &[f64]) -> Vec<(f64, f64)> {
    let sampler = ClassSampler::new(&XRB_SAMPLER_27);
    (0..DRAWS)
        .map(|i| {
            let rank = (f64::from(i) + 0.5) / f64::from(DRAWS);
            let condition = conditions.get(usize::try_from(i).expect("a draw's index"));
            // The condition's ranks are the golden ratio's multiples, so that they and the mark's
            // are spread independently.
            let mixed = (f64::from(i) * 0.618_033_988_749_894_8).fract();
            let value = match condition {
                Some(&c) => sampler.value_at(mark, c, mixed),
                None => sampler.value_at(mark, 0.0, rank),
            };
            (value, 1.0)
        })
        .collect()
}

/// The fitted X-ray binaries' sampler against the engine at 27 (P15.T10.b's acceptance; slow):
/// for each mark fitted from the engine, values drawn through the table against the engine's
/// members of the kinds the class draws it for, weighted so that their kinds are in the table's
/// weights, the slab's for the low-mass and history marks and the young thin disc's for the
/// high-mass ones, at the shape's resolution (module documentation). The class's hosts near the
/// Sun are too few for a sample of its entries, so the table is drawn directly, as its scan draws
/// it. A mark that misses is a finding, pinned at its measured distance in [`XRB_MISSES`]; a mark
/// the engine gives under [`MIN_MEMBERS`] members in either half, which the fit kept as the
/// scratch table's, is not compared.
#[test]
#[ignore = "slow: two censuses of 1.3 × 10⁷ records through the binary engine"]
fn the_x_ray_binary_sampler_against_the_engine() {
    let of_class = |members: Vec<ClassMember>| -> Vec<XrayMarks> {
        members
            .iter()
            .filter(|m| m.class() == CarvedClass::XrayBinary)
            .filter_map(|m| m.xray().copied())
            .collect()
    };
    let [slab, slab_fitted] = [Half::Held, Half::Fitted].map(|half| {
        of_class(census(
            CensusSite::SOLAR_CIRCLE,
            CensusRecords::All,
            CENSUS_RECORDS,
            half,
        ))
    });
    let [young, young_fitted] = [Half::Held, Half::Fitted].map(|half| {
        of_class(census(
            YOUNG_SITE,
            CensusRecords::Of(Population::YoungThinDisc),
            YOUNG_RECORDS,
            half,
        ))
    });
    let kinds = |pool: &[XrayMarks]| {
        XrbHostKind::ALL
            .iter()
            .map(|&kind| {
                let n = pool
                    .iter()
                    .filter(|x| x.member_kind() == Some(kind))
                    .count();
                format!("{kind:?} {n}")
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    println!("held-out slab members by kind: {}", kinds(&slab));
    println!("held-out young members by kind: {}", kinds(&young));
    let weights = xrb_table_weights();
    let periods: Vec<f64> = table_draws(XrbMark::LowMassPeriod, &[])
        .into_iter()
        .map(|d| d.0)
        .collect();
    let mut misses = Vec::new();
    for (k, mark) in XrbMark::ALL.into_iter().enumerate() {
        let high_mass = XRB_SAMPLER_27.mark(mark).name().contains("bex")
            || XRB_SAMPLER_27.mark(mark).name().contains("supergiant")
            || XRB_SAMPLER_27.mark(mark).name().contains("high_mass");
        let (pool, pool_fitted) = if high_mass {
            (&young, &young_fitted)
        } else {
            (&slab, &slab_fitted)
        };
        let held_out = xrb_weighted(pool, mark, &weights);
        let fitted_half = xrb_weighted(pool_fitted, mark, &weights);
        let name = XRB_SAMPLER_27.mark(mark).name();
        if held_out.len() < MIN_MEMBERS || fitted_half.len() < MIN_MEMBERS {
            println!(
                "  {name}: {} and {} engine members, not compared",
                held_out.len(),
                fitted_half.len()
            );
            continue;
        }
        let conditions: &[f64] = if mark == XrbMark::LowMassDonorMass {
            &periods
        } else {
            &[]
        };
        let r = Resolved::of(&table_draws(mark, conditions), &held_out, &fitted_half);
        println!(
            "  {name}: {} engine members, D {} (p {:.3e}) against {:.6} + in-sample {:.6}",
            held_out.len(),
            r.held_out.statistic,
            r.held_out.p_value,
            r.critical,
            r.in_sample
        );
        let pinned = XRB_MISSES.iter().find(|m| m.0 == k);
        let held_as_pinned = pinned.is_some_and(|m| (r.held_out.statistic - m.1).abs() < 1e-6);
        if (pinned.is_none() && !r.passes()) || (pinned.is_some() && !held_as_pinned) {
            misses.push(format!(
                "{name}: D {} against {} + {}",
                r.held_out.statistic, r.critical, r.in_sample
            ));
        }
    }
    assert!(misses.is_empty(), "{misses:#?}");
}

/// The X-ray binaries' marks whose Kolmogorov–Smirnov test misses at the shape's resolution, each
/// a finding: the mark's place in [`XrbMark`]'s order and its held-out distance, as measured to a
/// millionth.
const XRB_MISSES: [(usize, f64); 0] = [];
