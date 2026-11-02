//! The tests of P11.T4's heavy-donor fix, [`HEAVY_DONOR_VERSION`]: a main-sequence donor heavier
//! than its compact accretor transfers at its thermal rate, and the transfer keeps its angular
//! momentum, only under the rules that turn it on, read by the transfer's step, its trials and its
//! corotation through the engine's input's rules. Each fast test turns this hold's arm on alone
//! ([`thermal_rules`]), so that the other 27 holds move nothing it holds equal.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - The brief's pair, a 1.4 M☉ main-sequence donor at its lobe beside a 0.6 M☉ white dwarf: under
//!   the held arm it transfers at about 10⁻¹⁰ M☉ yr⁻¹ for 3.4 Gyr while it is the heavier; under
//!   the arm alone and under every 27 hold never below a tenth of its thermal rate.
//! - The same pair twice under the arm gives the same timeline, and a cell's systems whose pairs
//!   never feed a white dwarf, a neutron star or a black hole by stable transfer move under neither
//!   arm.
//! - Slow: a census of 10⁷ slab records' accreting white dwarfs under the version's rules and
//!   under every 27 hold.
//!
//! The movers under the arm, found by turning it on at the version (and back off), over the sim's
//! tests (with its `testing` feature too), the server's and the fitting crate's (2026-11-01): three
//! goldens, `stellar/binary_timelines`, `sky/census_near_sun` and `sky/band_face_row`; the paper's
//! cataclysmic variable, whose 1.3 M☉ donor, heavier than its white dwarf, opens at its thermal
//! rate (4.2 × 10⁻⁷ M☉ yr⁻¹) with the dwarf burning steadily, not in novae
//! (`the_papers_cataclysmic_variable_reproduces_its_sequence`,
//! `the_papers_cataclysmic_variable_is_one`, which reads a Type Ia progenitor first, and
//! `a_timeline_from_marks_keeps_its_carved_class_and_state`); the fitted tables' fingerprints of
//! `binary_awd`, `binary_xrb`, `binary_pair_light_c`, `_d` and `_e`, and `sky_binary_light_c`, `_d`
//! and `_e`, stale under the arm until the bump refits them; and, of the binaries' slow tests, the
//! merger channels' pinned chi-squares (`channels_hold` and `engine_channels_hold`). Under every 27
//! hold the engine channels' pin and the neutron-star census's are re-pinned with this hold, and
//! the listed binaries' sampler test (`the_listed_samplers_against_the_engine_s_members`) draws
//! its fast accreting white dwarfs of layer C at one in 1.7 × 10⁵ records, against 17 times that
//! without the arm, so it runs for about an hour.

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::planetary::testing::with_every_27_hold;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::testing::census::{Census, slab_census};
use crate::stellar::binary::testing::remnant_pair::{ONSET_AGE, donor_onto_remnant, thermal_rate};
use crate::stellar::binary::{BinaryClass, BinaryTimeline, CarvedClass, CvKind, SegmentKind};
use crate::stellar::multiplicity::{MultiplicityContext, RedrawAttempt};
use crate::stellar::system::{StarModel, SystemStars, draw_metallicity, primary_draws};
use crate::time::{ClockWindow, UniverseTime};
use crate::units::Years;

/// The census's Milky Way seed: the binary fits' (P15.T10.b's manifests).
const CENSUS_SEED: u64 = 0x0b17_15a0_0000_0001;

/// The records of the slow census: the fit's slab's 10⁷.
const CENSUS_RECORDS: u64 = 10_000_000;

/// The brief's pair: a solar 1.4 M☉ main-sequence donor at τ = 0.3 filling its lobe beside a
/// 0.6 M☉ carbon–oxygen white dwarf.
const PINNED: (f64, f64, f64) = (1.4, 0.3, 0.6);

/// The short edge of the cataclysmic variables' period gap, hours: 2.15 h (Knigge 2006, MNRAS
/// 373, 484; Knigge, Baraffe and Patterson 2011, ApJS 194, 28; recalled, not read at source).
const PERIOD_GAP_LOWER_HOURS: f64 = 2.15;

/// The share of the cataclysmic variables within 150 pc that lie below the period gap, and its
/// error: about 30 of Pala et al.'s (2020, MNRAS 494, 3799) 42, 0.7, with a Poisson error of 0.07
/// (recalled, not read at source).
const BELOW_GAP_SHARE: (f64, f64) = (0.7, 0.07);

/// The most of the hydrogen-donor cataclysmic variables whose donor may be heavier than the white
/// dwarf under every 27 hold: the brief's 5%.
const MOST_HEAVY_DONORS: f64 = 0.05;

/// The version's rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// The version's rules with this hold's arm on alone.
fn thermal_rules() -> StellarRules {
    StellarRules {
        heavy_donor: HeavyDonorRules::Thermal,
        ..version()
    }
}

/// The pinned pair's timeline under `rules`, to 12 Gyr after its onset.
fn pinned(rules: StellarRules) -> BinaryTimeline {
    let (donor, tau, dwarf) = PINNED;
    donor_onto_remnant(
        donor,
        tau,
        Phase::CarbonOxygenWhiteDwarf,
        dwarf,
        rules,
        1.2e10,
    )
    .0
}

/// The version's rules keep the held arm, every version below 27 does and 27 transfers at the
/// thermal rate; the fitted tables hold only under the version's arm. The tripwire: the bump to 27
/// deletes this hold.
#[test]
fn the_heavy_donor_rules_are_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < HEAVY_DONOR_VERSION,
        "the bump to 27 deletes P11.T4's heavy-donor hold: `HEAVY_DONOR_VERSION`, \
         `HeavyDonorRules` with its `Nuclear` arm, `StellarRules::heavy_donor`, the transfer's \
         three reads of it, `Engine::corotate_donor`, the brief's read of \
         `HeavyDonorRules::fitted_tables_hold`, this module and its line in `stellar/rules.rs`, \
         and refits the pair-light and reach tables"
    );
    assert_eq!(version().heavy_donor, HeavyDonorRules::Nuclear);
    let at = |v: u32| StellarRules::of_version(crate::GeneratorVersion::new(v)).heavy_donor;
    for v in [GENERATOR_VERSION.get(), 25, 26] {
        assert_eq!(at(v), HeavyDonorRules::Nuclear, "version {v}");
    }
    assert_eq!(at(27), HeavyDonorRules::Thermal);
    assert_eq!(
        StellarRules::every_27_hold().heavy_donor,
        HeavyDonorRules::Thermal
    );
    assert!(version().heavy_donor.fitted_tables_hold());
    assert!(!HeavyDonorRules::Thermal.fitted_tables_hold());
}

/// While the pair's donor is the heavier, its least rate of transfer, M☉ yr⁻¹, and how long that
/// lasts, years: sampled at 6,000 ages spaced evenly in log from a year after the onset to 12 Gyr,
/// up to the first at which the donor is no heavier than its white dwarf or no longer living.
fn while_heavier(timeline: &BinaryTimeline) -> (f64, f64) {
    let mut least = f64::INFINITY;
    for k in 0..=6_000_u32 {
        let since = crate::math::exp10(f64::from(k) / 6_000.0 * 10.08);
        let state = timeline.state_at(Years::new(ONSET_AGE + since));
        let [donor, dwarf] = state.stars();
        if donor.mass() <= dwarf.mass() || !donor.phase().is_living() {
            return (least, since);
        }
        if let Some(rate) = state.transfer_rate() {
            least = least.min(rate.value());
        }
    }
    (least, f64::INFINITY)
}

/// The pinned pair's least rate while its donor is the heavier and how long that lasts, under the
/// held arm, the arm alone and every 27 hold, as measured (2026-11-01, at generator version 25).
const PINNED_FIGURES: [(f64, f64); 3] = [
    (4.405_438_444_327_729e-11, 3_380_337_007.177_530_3),
    (4.389_759_460_751_92e-7, 1_244_629.241_081_902),
    (4.389_759_460_751_92e-7, 1_244_629.241_081_902),
];

/// The brief's pair: under the held arm its donor, heavier than the white dwarf for 3.4 Gyr,
/// transfers far below a tenth of its thermal rate; under the arm alone and under every 27 hold
/// never below a tenth of it while it is the heavier, which then lasts about a million years.
#[test]
fn the_heavy_donor_transfers_at_its_thermal_rate_under_the_arm() {
    let (donor, tau, _) = PINNED;
    let thermal = thermal_rate(donor, tau);
    let mut measured = Vec::new();
    for (k, rules) in [version(), thermal_rules(), StellarRules::every_27_hold()]
        .into_iter()
        .enumerate()
    {
        let timeline = pinned(rules);
        let (least, lasts) = while_heavier(&timeline);
        println!(
            "{:?}: the least rate while the donor is the heavier {least:.4e} M_sun/yr \
             ({:.4} of the thermal {thermal:.4e}), for {lasts:.4e} yr",
            rules.heavy_donor,
            least / thermal
        );
        if k == 0 {
            assert!(least < 0.01 * thermal, "{least}");
            assert!(lasts > 1.0e9, "{lasts}");
        } else {
            assert!(least >= 0.1 * thermal, "{least} against {thermal}");
            assert!(lasts < 1.0e7, "{lasts}");
        }
        measured.push((least, lasts));
    }
    for (m, pinned) in measured.iter().zip(PINNED_FIGURES) {
        assert!(
            (m.0 / pinned.0 - 1.0).abs() < 1e-9 && (m.1 / pinned.1 - 1.0).abs() < 1e-9,
            "{measured:?}"
        );
    }
}

/// The same pair twice under the arm gives the same timeline.
#[test]
fn the_arm_is_deterministic() {
    assert_eq!(pinned(thermal_rules()), pinned(thermal_rules()));
}

/// The first attempt's stars of `record` in `galaxy`, as the engine's members run them.
fn first_attempt(galaxy: &Galaxy, record: &SystemRecord) -> SystemStars {
    let primary = StarModel::new_under(
        record.primary_initial_mass(),
        draw_metallicity(galaxy, record),
        primary_draws(galaxy, record),
        record.age_at_epoch(),
        galaxy.rules().stellar,
    )
    .expect("a grid primary is a star with a finite age");
    SystemStars::at_attempt(
        galaxy,
        record,
        &primary,
        MultiplicityContext::Free,
        RedrawAttempt::FIRST,
    )
}

/// Whether `timeline` feeds a white dwarf, a neutron star or a black hole by stable transfer in
/// any segment.
fn feeds_a_compact_star(timeline: &BinaryTimeline) -> bool {
    timeline.segments().iter().any(|s| {
        let SegmentKind::StableTransfer { donor } = s.kind() else {
            return false;
        };
        let at = Years::new(s.start().value());
        matches!(
            timeline.state_at(at).stars()[1 - donor.index()].phase(),
            Phase::HeliumWhiteDwarf
                | Phase::CarbonOxygenWhiteDwarf
                | Phase::OxygenNeonWhiteDwarf
                | Phase::NeutronStar
                | Phase::BlackHole
        )
    })
}

/// A cell's systems of layer C whose pairs feed no compact star by stable transfer under the held
/// arm are in the same states across the clock window under both arms, those that transfer onto a
/// main-sequence star or a giant among them.
#[test]
fn systems_that_feed_no_compact_star_move_under_neither_arm() {
    let held = Galaxy::from_params(Seed::new(CENSUS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let arm = held.clone().with_rules(GalaxyRules {
        stellar: thermal_rules(),
        ..GalaxyRules::of_version(GENERATOR_VERSION)
    });
    let size = i32::try_from(Layer::C.cell_size_ly()).expect("a small cell size");
    let mut records = Vec::new();
    let mut cell = Vec::new();
    for x in 0..4_096 {
        let key = CellKey::new(Layer::C, [x, 26_000 / size, 0]).expect("a cell of the grid");
        generate_cell(&held, key, &mut cell);
        records.extend_from_slice(&cell);
        if records.len() >= 300 {
            break;
        }
    }
    let (mut compared, mut fed, mut transferring) = (0, 0, 0);
    for record in records.iter().take(300) {
        let stars = first_attempt(&held, record);
        if stars
            .pairs()
            .iter()
            .any(|pair| feeds_a_compact_star(pair.timeline()))
        {
            fed += 1;
            continue;
        }
        transferring += usize::from(stars.pairs().iter().any(|pair| {
            pair.timeline()
                .segments()
                .iter()
                .any(|s| matches!(s.kind(), SegmentKind::StableTransfer { .. }))
        }));
        let other = first_attempt(&arm, record);
        for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
            assert_eq!(
                stars.state_at(t),
                other.state_at(t),
                "{:?} at {t}",
                record.id()
            );
        }
        compared += 1;
    }
    println!(
        "{compared} compared, {transferring} of them in stable transfer; {fed} feed a compact star"
    );
    assert!(compared > 200, "{compared} of {}", records.len());
}

/// What a census says of its hydrogen-donor cataclysmic variables.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CvFigures {
    /// The accreting white dwarfs.
    members: usize,
    /// Of those, the cataclysmic variables with a hydrogen donor (all but the helium-donor ones).
    variables: usize,
    /// Of those, the ones whose donor is heavier than the white dwarf.
    heavy: usize,
    /// Of those, the ones below the period gap.
    below_gap: usize,
    /// Of those, the ones above 6 h.
    above_six_hours: usize,
}

impl CvFigures {
    /// The share of the variables whose donor is heavier than the white dwarf.
    fn heavy_share(&self) -> f64 {
        share(self.heavy, self.variables)
    }

    /// The share of the variables below the period gap.
    fn below_gap_share(&self) -> f64 {
        share(self.below_gap, self.variables)
    }
}

/// `part` of `whole`, as a share.
fn share(part: usize, whole: usize) -> f64 {
    let [part, whole] = [part, whole].map(|n| f64::from(u32::try_from(n).expect("a count")));
    part / whole.max(1.0)
}

/// `census`'s figures.
fn figures(census: &Census) -> CvFigures {
    let variables: Vec<_> = census
        .members
        .iter()
        .filter_map(|m| {
            let marks = m.accreting?;
            matches!(
                m.timeline.class_at(marks.age),
                BinaryClass::CataclysmicVariable(kind) if kind != CvKind::AmCvn
            )
            .then_some(marks)
        })
        .collect();
    let hours = |p: Option<crate::units::Seconds>| p.map_or(f64::NAN, |p| p.value() / 3_600.0);
    CvFigures {
        members: census
            .members
            .iter()
            .filter(|m| m.accreting.is_some())
            .count(),
        variables: variables.len(),
        heavy: variables
            .iter()
            .filter(|a| a.donor_mass > a.white_dwarf_mass)
            .count(),
        below_gap: variables
            .iter()
            .filter(|a| hours(a.period) < PERIOD_GAP_LOWER_HOURS)
            .count(),
        above_six_hours: variables.iter().filter(|a| hours(a.period) > 6.0).count(),
    }
}

/// The census's figures under the version's rules and under every 27 hold, as measured
/// (2026-11-01, at generator version 25).
const CENSUS_FIGURES: [CvFigures; 2] = [
    CvFigures {
        members: 3_642,
        variables: 1_404,
        heavy: 832,
        below_gap: 436,
        above_six_hours: 848,
    },
    CvFigures {
        members: 4_120,
        variables: 2_656,
        heavy: 47,
        below_gap: 2_298,
        above_six_hours: 80,
    },
];

/// A census of 10⁷ slab records' accreting white dwarfs ([`slab_census`]), under the version's
/// rules and under every 27 hold. Under the version's most hydrogen-donor cataclysmic variables
/// have a donor heavier than the white dwarf; under every 27 hold at most [`MOST_HEAVY_DONORS`] do,
/// and the share below the period gap is at least Pala et al.'s less its error
/// ([`BELOW_GAP_SHARE`]).
#[test]
#[ignore = "slow: 2 × 10⁷ grid records' pairs run through the binary engine"]
fn census_of_the_heavy_donors() {
    let census = |rules: GalaxyRules| {
        let galaxy = Galaxy::from_params_with_rules(
            Seed::new(CENSUS_SEED),
            GalaxyParams::milky_way_like(),
            rules,
        )
        .expect("the Milky Way fixture's gas is mostly neutral");
        let census = slab_census(
            &galaxy,
            CENSUS_RECORDS,
            &[CarvedClass::AccretingWdFast, CarvedClass::AccretingWdSlow],
        );
        (census.scanned, figures(&census))
    };
    let before = census(GalaxyRules::of_version(GENERATOR_VERSION));
    let after = with_every_27_hold(|| census(GalaxyRules::every_27_hold()));
    for (name, (scanned, f)) in [("version's rules", before), ("every 27 hold", after)] {
        println!(
            "{name}: {scanned:?} records by band, {f:?}: heavier donors {:.4}, below the gap {:.4}",
            f.heavy_share(),
            f.below_gap_share()
        );
    }
    let (before, after) = (before.1, after.1);
    assert!(before.heavy_share() > 0.5, "{before:?}");
    assert!(after.heavy_share() <= MOST_HEAVY_DONORS, "{after:?}");
    assert!(
        after.below_gap_share() >= BELOW_GAP_SHARE.0 - BELOW_GAP_SHARE.1,
        "{after:?}"
    );
    for (f, pinned) in [before, after].iter().zip(CENSUS_FIGURES) {
        assert_eq!(*f, pinned);
    }
}
