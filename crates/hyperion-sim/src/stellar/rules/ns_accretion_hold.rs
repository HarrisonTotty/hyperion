//! The tests of P11.T4's neutron-star accretion fix, [`NS_ACCRETION_VERSION`]: a neutron star fed
//! by stable Roche-lobe overflow keeps [`NS_ACCRETION_EFFICIENCY`] of it only under the rules that
//! turn it on, read at one choke point, `Engine::accretion`, through the engine's input's rules.
//! Each fast test turns this hold's arm on alone ([`efficiency_rules`]), so that the other 27 holds
//! move nothing it holds equal.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - A pinned pair, a 1.5 M☉ main-sequence donor at its lobe beside a 1.35 M☉ neutron star: under
//!   the held arm the star takes the whole transfer and becomes a black hole; under the arm alone
//!   and under every 27 hold it keeps a tenth and stays a neutron star.
//! - The same pair twice under the arm gives the same timeline, and a cell's systems whose pairs
//!   feed no neutron star by stable transfer move under neither arm.
//! - Slow: a census of 10⁷ slab records under the version's rules and under every 27 hold.
//!
//! The movers under the arm, found by turning it on at the version (and back off), over the sim's,
//! the server's and the fitting crate's tests (the sim's with its `testing` feature too) and the
//! binaries' slow censuses (2026-11-01): two goldens, `stellar/binary_timelines`
//! (`a_thousand_timelines_are_pinned`) and `sky/census_near_sun` (layer D's stars without
//! photometry, 916 to 912); two fitted tables' fingerprints, `binary_pair_light_d` and
//! `sky_binary_light_d`, stale under the arm until the bump refits them; and the pinned chi-squares
//! of the merger channels' sampler under every 26 hold
//! (`the_channel_sampler_against_the_engine_under_every_26_hold`), which already differ from their
//! pin at this hold's base. No census of the binaries moves out of its bounds (`binary_system`,
//! `binary_carve`, `binary_classes`, `binary_xrb`, `binary_awd`, `binary_events`,
//! `binary_mergers`, the pair-light and reach tables' bounds, the engine's invariants).

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
use crate::stellar::binary::ns_accretion::NS_ACCRETION_EFFICIENCY;
use crate::stellar::binary::testing::census::{Census, CensusMember, slab_census};
use crate::stellar::binary::testing::remnant_pair::{ONSET_AGE, donor_onto_remnant};
use crate::stellar::binary::{BinaryClass, BinaryTimeline, CarvedClass, Component, SegmentKind};
use crate::stellar::multiplicity::{MultiplicityContext, RedrawAttempt};
use crate::stellar::remnant::RemnantKind;
use crate::stellar::system::{StarModel, SystemStars, draw_metallicity, primary_draws};
use crate::time::{ClockWindow, UniverseTime};
use crate::units::Years;

/// The census's Milky Way seed: the binary fits' (P15.T10.b's manifests).
const CENSUS_SEED: u64 = 0x0b17_15a0_0000_0001;

/// The records of the slow census: the fit's slab's 10⁷, since 10⁶ hold about one black hole in
/// an X-ray binary that was born one under every 27 hold.
const CENSUS_RECORDS: u64 = 10_000_000;

/// The pinned pair: a solar 1.5 M☉ main-sequence donor at τ = 0.3 filling its lobe beside a
/// 1.35 M☉ neutron star.
const PINNED: (f64, f64, f64) = (1.5, 0.3, 1.35);

/// The most a neutron star in a low-mass X-ray binary has gained under the arm, M☉: 0.2, the top of
/// the 0.1–0.2 M☉ by which the millisecond pulsars' masses exceed the slow pulsars' (Antoniadis et
/// al. 2016, arXiv:1605.01665; recalled, not read at source; [`NS_ACCRETION_EFFICIENCY`]).
const MOST_GAINED: f64 = 0.2;

/// The version's rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// The version's rules with this hold's arm on alone.
fn efficiency_rules() -> StellarRules {
    StellarRules {
        ns_accretion: NsAccretionRules::Efficiency,
        ..version()
    }
}

/// The pinned pair's timeline under `rules`, to 12 Gyr after its onset.
fn pinned(rules: StellarRules) -> BinaryTimeline {
    let (donor, tau, star) = PINNED;
    donor_onto_remnant(donor, tau, Phase::NeutronStar, star, rules, 1.2e10).0
}

/// The version's rules keep the held arm, every version below 27 does and 27 keeps a share; the
/// fitted tables hold only under the version's arm. The tripwire: the bump to 27 deletes this hold.
#[test]
fn the_neutron_star_accretion_is_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < NS_ACCRETION_VERSION,
        "the bump to 27 deletes P11.T4's neutron-star accretion hold: `NS_ACCRETION_VERSION`, \
         `NsAccretionRules` with its `Eddington` arm, `StellarRules::ns_accretion`, \
         `Engine::accretion`'s read of it, the brief's read of \
         `NsAccretionRules::fitted_tables_hold`, this module and its line in `stellar/rules.rs`, \
         and refits the pair-light and reach tables"
    );
    assert_eq!(version().ns_accretion, NsAccretionRules::Eddington);
    let at = |v: u32| StellarRules::of_version(crate::GeneratorVersion::new(v)).ns_accretion;
    for v in [GENERATOR_VERSION.get(), 25, 26] {
        assert_eq!(at(v), NsAccretionRules::Eddington, "version {v}");
    }
    assert_eq!(at(27), NsAccretionRules::Efficiency);
    assert_eq!(
        StellarRules::every_26_hold().ns_accretion,
        NsAccretionRules::Eddington
    );
    assert_eq!(
        StellarRules::every_27_hold().ns_accretion,
        NsAccretionRules::Efficiency
    );
    assert_eq!(
        GalaxyRules::every_27_hold().stellar,
        StellarRules::every_27_hold()
    );
    assert!(version().ns_accretion.fitted_tables_hold());
    assert!(!NsAccretionRules::Efficiency.fitted_tables_hold());
}

/// The pinned pair's neutron star at the end of the donor's first transfer, and its compact star's
/// mass 12 Gyr on, M☉, under the held arm, the arm alone and every 27 hold, as measured
/// (2026-11-01, at generator version 25).
const PINNED_FIGURES: [(f64, f64); 3] = [
    (1.811_174_601_094_979, 2.608_411_194_021_577_3),
    (1.387_548_786_842_689_2, 1.483_859_458_716_522_6),
    (1.387_548_786_842_689_2, 1.483_859_458_716_522_6),
];

/// Under the held arm the pinned pair's neutron star takes the donor's first transfer whole and
/// passes `MAX_NEUTRON_STAR_MASS` into a black hole; under the arm alone it keeps
/// [`NS_ACCRETION_EFFICIENCY`] of what the donor gives (its wind gives none on the main sequence),
/// to a part in 10⁶, and under every 27 hold less, the Eddington rate cutting the donor's thermal
/// rate; under both it is a neutron star 12 Gyr on, having gained less than [`MOST_GAINED`].
#[test]
fn the_pinned_neutron_star_keeps_a_tenth_under_the_arm() {
    let (donor, _, star) = PINNED;
    let mut measured = Vec::new();
    for (k, rules) in [version(), efficiency_rules(), StellarRules::every_27_hold()]
        .into_iter()
        .enumerate()
    {
        let timeline = pinned(rules);
        let first = timeline
            .segments()
            .iter()
            .find(|s| s.start().value() >= ONSET_AGE)
            .expect("the transfer from the onset");
        assert_eq!(
            first.kind(),
            SegmentKind::StableTransfer {
                donor: Component::Primary
            }
        );
        let end = timeline.state_at(Years::new(first.end().value() * (1.0 - 1e-12)));
        let [d, a] = end.stars();
        let (given, gained) = (donor - d.mass().value(), a.mass().value() - star);
        let last = timeline.state_at(Years::new(ONSET_AGE + 1.2e10));
        let compact = last.stars()[1];
        println!(
            "{:?}: the donor gave {given:.6} M_sun by {:.4e} yr, the star gained {gained:.6} \
             ({:.6} of it); 12 Gyr on a {:?} of {:.6} M_sun",
            rules.ns_accretion,
            first.end().value(),
            gained / given,
            compact.phase(),
            compact.mass().value()
        );
        if k == 0 {
            assert!((gained / given - 1.0).abs() < 1e-6, "{gained} of {given}");
            assert_eq!(compact.phase(), Phase::BlackHole);
        } else {
            // Under every 27 hold the donor, the heavier, transfers at its thermal rate at first
            // (`heavy_donor`), over the star's Eddington rate, which then cuts its share.
            let share = gained / given / NS_ACCRETION_EFFICIENCY;
            if k == 1 {
                assert!((share - 1.0).abs() < 1e-6, "{gained} of {given}");
            } else {
                assert!(share < 1.0, "{gained} of {given}");
            }
            assert_eq!(compact.phase(), Phase::NeutronStar);
            assert!(compact.mass().value() - star < MOST_GAINED);
        }
        measured.push((a.mass().value(), compact.mass().value()));
    }
    for (m, pinned) in measured.iter().zip(PINNED_FIGURES) {
        assert!(
            (m.0 - pinned.0).abs() < 1e-12 && (m.1 - pinned.1).abs() < 1e-12,
            "{measured:?}"
        );
    }
}

/// The same pair twice under the arm gives the same timeline.
#[test]
fn the_arm_is_deterministic() {
    assert_eq!(pinned(efficiency_rules()), pinned(efficiency_rules()));
}

/// Whether `timeline` feeds a neutron star by stable transfer in any segment.
fn feeds_a_neutron_star(timeline: &BinaryTimeline) -> bool {
    timeline.segments().iter().any(|s| {
        let SegmentKind::StableTransfer { donor } = s.kind() else {
            return false;
        };
        let at = Years::new(s.start().value());
        timeline.state_at(at).stars()[1 - donor.index()].phase() == Phase::NeutronStar
    })
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

/// A cell's systems of layer D whose pairs feed no neutron star by stable transfer under the held
/// arm are in the same states across the clock window under both arms.
#[test]
fn systems_that_feed_no_neutron_star_move_under_neither_arm() {
    let held = Galaxy::from_params(Seed::new(CENSUS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let arm = held.clone().with_rules(GalaxyRules {
        stellar: efficiency_rules(),
        ..GalaxyRules::of_version(GENERATOR_VERSION)
    });
    let size = i32::try_from(Layer::D.cell_size_ly()).expect("a small cell size");
    let mut records = Vec::new();
    let mut cell = Vec::new();
    for x in 0..4_096 {
        let key = CellKey::new(Layer::D, [x, 26_000 / size, 0]).expect("a cell of the grid");
        generate_cell(&held, key, &mut cell);
        records.extend_from_slice(&cell);
        if records.len() >= 200 {
            break;
        }
    }
    let (mut compared, mut fed) = (0, 0);
    for record in records.iter().take(200) {
        let stars = first_attempt(&held, record);
        if stars
            .pairs()
            .iter()
            .any(|pair| feeds_a_neutron_star(pair.timeline()))
        {
            fed += 1;
            continue;
        }
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
    println!("{compared} compared, {fed} feed a neutron star");
    assert!(compared > 150, "{compared} of {}", records.len());
}

/// What a census says of its X-ray binaries' compact stars.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CompactFigures {
    /// The X-ray binaries.
    members: usize,
    /// Those whose compact star is a black hole.
    black_holes: usize,
    /// Of those, the black holes born neutron stars.
    grown: usize,
    /// Of those, the ones fed by a white dwarf or a substellar object: the relics of a donor
    /// ground down.
    grown_by_dwarfs: usize,
    /// The median mass of the black holes in low-mass X-ray binaries, M☉.
    median_black_hole: f64,
    /// The neutron stars in low-mass X-ray binaries.
    neutron_stars: usize,
    /// The most any of them has gained since its birth, M☉.
    most_gained: f64,
}

/// Whether `member`'s compact star is a black hole born a neutron star.
fn grown(member: &CensusMember) -> bool {
    matches!(member.compact_birth(), Some((RemnantKind::NeutronStar, _)))
}

/// `census`'s figures.
fn figures(census: &Census) -> CompactFigures {
    let marks = |m: &CensusMember| m.xray.expect("an X-ray binary's marks");
    let xray: Vec<&CensusMember> = census.members.iter().filter(|m| m.xray.is_some()).collect();
    let low_mass = |m: &CensusMember| matches!(marks(m).class, BinaryClass::LowMassXrayBinary(_));
    let by_dwarf = |m: &CensusMember| {
        matches!(
            marks(m).donor_phase,
            Phase::HeliumWhiteDwarf
                | Phase::CarbonOxygenWhiteDwarf
                | Phase::OxygenNeonWhiteDwarf
                | Phase::Substellar
        )
    };
    let black_holes: Vec<&CensusMember> = xray
        .iter()
        .copied()
        .filter(|m| marks(m).accretor_phase == Phase::BlackHole)
        .collect();
    let mut masses: Vec<f64> = black_holes
        .iter()
        .filter(|m| low_mass(m))
        .map(|m| marks(m).accretor_mass.value())
        .collect();
    masses.sort_by(f64::total_cmp);
    let gains: Vec<f64> = xray
        .iter()
        .filter(|m| low_mass(m) && marks(m).accretor_phase == Phase::NeutronStar)
        .filter_map(|m| {
            let (_, birth) = m.compact_birth()?;
            Some(marks(m).accretor_mass.value() - birth.value())
        })
        .collect();
    CompactFigures {
        members: xray.len(),
        black_holes: black_holes.len(),
        grown: black_holes.iter().filter(|m| grown(m)).count(),
        grown_by_dwarfs: black_holes
            .iter()
            .filter(|m| grown(m) && by_dwarf(m))
            .count(),
        median_black_hole: masses.get(masses.len() / 2).copied().unwrap_or(f64::NAN),
        neutron_stars: gains.len(),
        most_gained: gains.iter().copied().fold(0.0, f64::max),
    }
}

/// The census's figures under the version's rules and under every 27 hold, as measured
/// (2026-11-01, at generator version 25).
const CENSUS_FIGURES: [CompactFigures; 2] = [
    CompactFigures {
        members: 134,
        black_holes: 77,
        grown: 59,
        grown_by_dwarfs: 5,
        median_black_hole: 2.673_097_250_365_381,
        neutron_stars: 55,
        most_gained: 0.786_283_605_608_198_8,
    },
    CompactFigures {
        members: 31,
        black_holes: 3,
        grown: 1,
        grown_by_dwarfs: 1,
        median_black_hole: 3.384_460_342_173_085_2,
        neutron_stars: 28,
        most_gained: 0.118_139_154_635_990_67,
    },
];

/// A census of 10⁷ slab records' X-ray binaries ([`slab_census`]), under the version's rules and
/// under every 27 hold. Under the version's most of the black holes were born neutron stars that
/// stable transfer grew past `MAX_NEUTRON_STAR_MASS`. Under every 27 hold:
///
/// - none of the black holes not fed by a white dwarf or a substellar object was born a neutron
///   star, against the brief's 5%; those fed by one, the relics of a donor ground down, which the
///   carving's ultracompact floor is to remove (`fix-p11-t8b-xrb-carving`), are pinned as measured
///   (a finding). At generator version 25 the one is a neutron star born at 1.861 M☉ that a
///   1.46 M☉ main-sequence donor fed for 7 Gyr, grinding itself down to 0.068 M☉: 0.139 M☉
///   gained at the arm's tenth carries it past `MAX_NEUTRON_STAR_MASS` (a lean of land-pod's,
///   2026-11-01, for research);
/// - no neutron star in a low-mass X-ray binary has gained more than [`MOST_GAINED`];
/// - the black holes' median mass in the low-mass X-ray binaries is pinned as measured, under the
///   brief's 5 M☉ (a finding): most are fallback black holes of 2–4 M☉ (plan 06's Mandel and
///   Müller law, 0.8 `M_CO` ± 0.5 M☉ held above 2 M☉), not neutron stars grown.
#[test]
#[ignore = "slow: 2 × 10⁷ grid records' pairs run through the binary engine"]
fn census_of_the_neutron_star_accretion() {
    let census = |rules: GalaxyRules| {
        let galaxy = Galaxy::from_params_with_rules(
            Seed::new(CENSUS_SEED),
            GalaxyParams::milky_way_like(),
            rules,
        )
        .expect("the Milky Way fixture's gas is mostly neutral");
        let census = slab_census(&galaxy, CENSUS_RECORDS, &[CarvedClass::XrayBinary]);
        (census.scanned, figures(&census))
    };
    let before = census(GalaxyRules::of_version(GENERATOR_VERSION));
    let after = with_every_27_hold(|| census(GalaxyRules::every_27_hold()));
    for (name, (scanned, f)) in [("version's rules", before), ("every 27 hold", after)] {
        println!("{name}: {scanned:?} records by band, {f:?}");
    }
    let (before, after) = (before.1, after.1);
    assert!(2 * before.grown > before.black_holes, "{before:?}");
    assert_eq!(after.grown, after.grown_by_dwarfs, "{after:?}");
    assert!(after.most_gained <= MOST_GAINED, "{after:?}");
    for (f, pinned) in [before, after].iter().zip(CENSUS_FIGURES) {
        assert_eq!(
            (
                f.members,
                f.black_holes,
                f.grown,
                f.grown_by_dwarfs,
                f.neutron_stars
            ),
            (
                pinned.members,
                pinned.black_holes,
                pinned.grown,
                pinned.grown_by_dwarfs,
                pinned.neutron_stars
            ),
            "{f:?}"
        );
        assert!(
            (f.median_black_hole - pinned.median_black_hole).abs() < 1e-12
                && (f.most_gained - pinned.most_gained).abs() < 1e-12,
            "{f:?}"
        );
    }
}
