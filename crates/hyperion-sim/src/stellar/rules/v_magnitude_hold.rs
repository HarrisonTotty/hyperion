//! The tests of P06.T43.b's hold, [`V_MAGNITUDE_VERSION`] (decision-stellar-holds §1.2, with
//! decision-galaxy-25-train §1.5–§1.6's D2–D4): white dwarfs take the Montreal grid's V and evolved
//! stars the giants' bolometric correction only under the rules that turn A4's arm on, chosen at
//! one choke point, `photometry::absolute_magnitude_v_under`. Each test turns this hold's arm on
//! alone ([`a4_rules`]), so that the other 26 holds move nothing it holds equal, except the plan's
//! figures, which are asserted under `every_26_hold`.
//!
//! - **D3.** The version's rules keep A4's interim: no white dwarf has a V, and every living star
//!   reads the dwarf table.
//! - **D2.** Under A4 these readers show a mover: a star's summary (a white dwarf gains a V, a cool
//!   evolved star dims) and the host stars' discs here; the sky census's stars, its tally of white
//!   dwarfs without photometry and its restated bounds in `sky::census::cell`'s
//!   `the_census_lists_white_dwarfs_under_the_a4_arm` and
//!   `the_census_bounds_hold_white_dwarfs_under_the_a4_arm`. A main-sequence star moves under
//!   none.
//! - **D4.** The same seed twice under A4 gives the same stars.
//! - **D1** is the goldens as they stand: no version takes A4's arm. The determinism audit's golden
//!   of the two corrections off their nodes is `photometry`'s
//!   `the_a4_corrections_are_pinned_off_their_nodes`, at the version's rules.
//! - The census's white-dwarf ceiling, `WHITE_DWARF_BRIGHTEST_V`, is measured by the slow
//!   [`no_white_dwarf_is_brighter_than_the_census_bound`].
//!
//! Readers that take the version's rules, and why: the range rows' brief reads no photometry; the
//! tables fitted offline through `absolute_v_of_state` or `absolute_magnitude_v`, which
//! `hyperion-fit` builds under the version's rules and the bump refits (`sky_envelope`,
//! `sky_phase_envelope`, `sky_binary_light_*`, `binary_pair_light_*`), the pair light's held
//! floor, the sky's luminosity tables (`sky::luminosity`, built once per seed) and the server's
//! converters. `binary_cooling_floor` reads only stars below 0.1 M☉, which A4 does not move.

use std::sync::OnceLock;

use hyperion_testkit::float::bits;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::features::centre::testing::milky_way_galaxy;
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::math;
use crate::sky::disc::{HostDisc, host_discs};
use crate::sky::envelope::FE_H_NODES;
use crate::sky::phase::ETA_NODES;
use crate::stellar::binary::evolve;
use crate::stellar::binary::pair_light::{LAYERS, PairGrid, sample_input};
use crate::stellar::classify::tests::{star, star_g};
use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use crate::stellar::photometry::{
    WHITE_DWARF_BRIGHTEST_V, absolute_bolometric_magnitude, absolute_magnitude_v_under,
    absolute_v_of_state_under, bolometric_correction_v_under, white_dwarf_absolute_magnitude_v,
};
use crate::stellar::sse::{Track, TrackOptions};
use crate::stellar::system::{StarSummary, SystemStars};
use crate::stellar::{Composition, Phase, StarState};
use crate::time::UniverseTime;
use crate::units::{Dex, HeliumExcess, Magnitudes, SolarMasses, Years};

/// The version's stellar rules: A4's interim.
const VERSION: StellarRules = StellarRules::of_version(GENERATOR_VERSION);

/// The version's stellar rules with this hold's arm alone turned on.
const fn a4_rules() -> StellarRules {
    StellarRules {
        v_magnitude: VMagnitudeRules::A4,
        ..VERSION
    }
}

/// Every 26 hold with this one's arm turned back off: what `every_26_hold` is without A4.
const fn every_26_but_a4() -> StellarRules {
    StellarRules {
        v_magnitude: VMagnitudeRules::Interim,
        ..StellarRules::every_26_hold()
    }
}

/// A magnitude's bits, or `None`.
fn v_bits(m: Option<Magnitudes>) -> Option<u64> {
    m.map(|m| bits(m.value()))
}

/// The sky fixture's galaxy under the version's rules.
fn held() -> &'static Galaxy {
    milky_way_galaxy()
}

/// [`held`]'s galaxy under [`a4_rules`].
fn a4_galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        held().clone().with_rules(GalaxyRules {
            stellar: a4_rules(),
            ..GalaxyRules::of_version(GENERATOR_VERSION)
        })
    })
}

/// [`held`]'s galaxy under every 26 hold.
fn every_26_galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| held().clone().with_rules(GalaxyRules::every_26_hold()))
}

/// Twenty records of each of three cells of every layer by the Sun.
fn records() -> &'static [SystemRecord] {
    static RECORDS: OnceLock<Vec<SystemRecord>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        let mut all = Vec::new();
        let mut cell = Vec::new();
        for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
            let size = i32::try_from(layer.cell_size_ly()).expect("small");
            for step in 0..3 {
                let key = CellKey::new(layer, [step, 26_000 / size, 0]).expect("a cell");
                generate_cell(held(), key, &mut cell);
                all.extend(cell.drain(..).take(20));
            }
        }
        all
    })
}

/// Whether `phase` is a white dwarf's.
const fn is_white_dwarf(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf
    )
}

/// P06.T43.b is held until the 25 → 26 bump: the rules in force keep A4's interim.
#[test]
fn the_a4_magnitudes_are_held_only_until_the_26_bump() {
    assert!(
        GENERATOR_VERSION < V_MAGNITUDE_VERSION,
        "the 25 → 26 bump puts P06.T43.b's A4 photometry on the record (every white dwarf gains a \
         V from the Montreal grid, and every living star past the hydrogen main sequence reads the \
         giants' bolometric correction): delete `V_MAGNITUDE_VERSION`, `VMagnitudeRules` with its \
         `Interim` arm, `StellarRules::v_magnitude`, the `match` in \
         `photometry::absolute_magnitude_v_under`, the disc's dwarf-table stand-in for white \
         dwarfs and this module; keep the census's `BoundStar::white_dwarfs` ceiling for the \
         stars a pair changes, whose white dwarfs the refitted tables do not hold, and for the \
         rest take the refitted phase envelope's white-dwarf rows; restate on the new \
         photometry `photometry::tests::protostars_and_white_dwarfs_are_dark_and_main_sequence_\
         stars_are_not` and `nothing_without_a_calibration_has_a_magnitude`, the doctest of \
         `white_dwarf_absolute_magnitude_v`, and the census tests that rest on dark white dwarfs \
         (`a_white_dwarf_primary_beside_a_bright_companion_lists_the_companion`, \
         `each_verdict_leaves_the_stars_the_light_the_ruling_gives`, \
         `the_primary_is_bounded_once_by_the_least_light_that_bounds_it_at_every_attempt`, \
         `the_observers_own_system_is_absent`); refit the tables `just fit-check` flags \
         (sky_envelope, sky_phase_envelope, sky_binary_light_c/d/e, binary_pair_light_c/d/e); \
         then re-bless by test name the goldens that move (stellar/classify, \
         stellar_system's summaries, sky's real_boundary, census_near_sun, eye_cut and \
         near_sun) and luminosity's `SERIAL_FINGERPRINT`"
    );
    assert_eq!(VERSION.v_magnitude, VMagnitudeRules::Interim);
    assert_eq!(VERSION.v_magnitude.white_dwarfs_brightest_v(), None);
    assert_eq!(held().rules().stellar.v_magnitude, VMagnitudeRules::Interim);
}

/// A4's arm turns on at 26 and leaves the other 26 holds' arms there as they are.
#[test]
fn the_a4_arm_turns_on_at_26() {
    for v in 1..=64 {
        let expected = if v >= 26 {
            VMagnitudeRules::A4
        } else {
            VMagnitudeRules::Interim
        };
        assert_eq!(
            StellarRules::of_version(GeneratorVersion::new(v)).v_magnitude,
            expected,
            "version {v}"
        );
    }
    assert_eq!(
        StellarRules::every_25_hold().v_magnitude,
        VMagnitudeRules::Interim
    );
    let at_26 = StellarRules::every_26_hold();
    assert_eq!(at_26.v_magnitude, VMagnitudeRules::A4);
    assert_eq!(
        at_26.v_magnitude.white_dwarfs_brightest_v(),
        Some(WHITE_DWARF_BRIGHTEST_V)
    );
    assert_eq!(at_26.kick_rank, KickRankRules::Committed);
    assert_eq!(at_26.dwarf_sequence, DwarfSequenceRules::Reduced);
    assert_eq!(at_26.helium, HeliumRules::Fitted);
    assert_eq!(at_26.hd_limit, HdLimitRules::Enforced);
    assert_eq!(GalaxyRules::every_26_hold().stellar, at_26);
}

/// D3: under the version's rules a white dwarf has no V and a giant reads the dwarf table's
/// correction at its temperature, bit for bit, as before the hold.
#[test]
fn the_version_s_rules_keep_the_interim() {
    let dwarf = star_g(Phase::CarbonOxygenWhiteDwarf, 0.6, 8.0, 10_000.0);
    assert_eq!(absolute_magnitude_v_under(&dwarf, VERSION), None);
    assert_eq!(absolute_v_of_state_under(&dwarf, VERSION), None);
    let giant = star_g(Phase::FirstGiantBranch, 1.0, 0.52, math::exp10(3.512));
    let dwarf_table = absolute_bolometric_magnitude(giant.luminosity()).unwrap()
        - bolometric_correction_v_under(giant.effective_temperature(), VERSION).unwrap();
    assert_eq!(
        v_bits(absolute_magnitude_v_under(&giant, VERSION)),
        v_bits(Some(dwarf_table))
    );
    assert_ne!(
        v_bits(absolute_magnitude_v_under(&giant, a4_rules())),
        v_bits(Some(dwarf_table))
    );
}

/// The plan's first figure, under every 26 hold: a generated white dwarf has a V, the Montreal
/// grid's at its state, and is not dark; under the version's rules it has none.
#[test]
fn a_generated_white_dwarf_has_its_montreal_v() {
    let rules = StellarRules::every_26_hold();
    let mut seen = 0;
    for record in records() {
        let summary =
            SystemStars::generate(every_26_galaxy(), record).summary_at(UniverseTime::EPOCH);
        for star in summary.stars() {
            let state = star.state();
            if !is_white_dwarf(state.phase()) {
                continue;
            }
            let Some(montreal) = white_dwarf_absolute_magnitude_v(state) else {
                continue;
            };
            seen += 1;
            assert_eq!(
                v_bits(star.absolute_magnitude_v()),
                v_bits(Some(montreal)),
                "{:?}",
                record.id()
            );
            assert_eq!(
                v_bits(absolute_v_of_state_under(state, rules)),
                v_bits(Some(montreal))
            );
            assert_eq!(absolute_v_of_state_under(state, VERSION), None);
            assert!(
                montreal.value() > WHITE_DWARF_BRIGHTEST_V.value(),
                "{montreal:?}"
            );
        }
    }
    eprintln!("{seen} white dwarfs of {} records", records().len());
    assert!(seen >= 3, "{seen} white dwarfs");
}

/// The plan's second figure, under every 26 hold: an M6 III giant (Straižys and Kuriliene's 3,251
/// K and log g 0.52, their Tables III and VII) is 1.339 mag fainter in V than the version's rules
/// make it, read as a record's summary and the sky's census read it, on every evolved branch it
/// can be on.
#[test]
fn an_m6_giant_is_1_339_mag_fainter() {
    let rules = StellarRules::every_26_hold();
    for phase in [
        Phase::FirstGiantBranch,
        Phase::EarlyAgb,
        Phase::ThermallyPulsingAgb,
    ] {
        let giant = star_g(phase, 1.0, 0.52, math::exp10(3.512));
        for read in [absolute_magnitude_v_under, absolute_v_of_state_under] {
            let today = read(&giant, VERSION).unwrap().value();
            let after = read(&giant, rules).unwrap().value();
            assert!(
                (after - today - 1.339).abs() < 5e-4,
                "{phase:?}: {today} → {after}"
            );
        }
    }
}

/// The plan's third figure: A4 moves no main-sequence star, bit for bit, under every 26 hold and
/// under the version's other rules alike; nor a protostar, a pre-main-sequence star or a brown
/// dwarf, nor any evolved star from 6,000 K, where the giants' correction is the dwarf table's.
#[test]
fn a_main_sequence_star_s_v_does_not_change() {
    let mut compared = 0;
    let mut t = 1_800.0_f64;
    while t < 60_000.0 {
        for (phase, m, log_g) in [
            (Phase::MainSequence, 1.0, 4.4),
            (Phase::MainSequence, 0.2, 5.1),
            (Phase::MainSequence, 20.0, 4.0),
            (Phase::PreMainSequence, 0.8, 3.8),
            (Phase::Protostar, 0.5, 3.5),
            (Phase::Substellar, 0.05, 5.2),
        ] {
            let state = star_g(phase, m, log_g, t);
            for (with, without) in [
                (StellarRules::every_26_hold(), every_26_but_a4()),
                (a4_rules(), VERSION),
            ] {
                assert_eq!(
                    v_bits(absolute_magnitude_v_under(&state, with)),
                    v_bits(absolute_magnitude_v_under(&state, without)),
                    "{phase:?} at {t} K"
                );
            }
            compared += 1;
        }
        if t >= 6_000.0 {
            for phase in [
                Phase::HertzsprungGap,
                Phase::HeliumMainSequence,
                Phase::PostAgb,
            ] {
                let state = star_g(phase, 1.0, 3.0, t);
                assert_eq!(
                    v_bits(absolute_magnitude_v_under(&state, a4_rules())),
                    v_bits(absolute_magnitude_v_under(&state, VERSION)),
                    "{phase:?} at {t} K"
                );
            }
        }
        t *= 1.003;
    }
    assert!(compared > 6_000);
    // A star with no luminosity has no magnitude under either arm.
    let dark = star(Phase::MainSequence, 1.0, 0.0, 5_772.0);
    assert_eq!(absolute_magnitude_v_under(&dark, a4_rules()), None);
}

/// D2, a galaxy's stars: under A4 the summaries' V moves for white dwarfs, which gain one, and for
/// cool evolved stars, and for no star on or before its main sequence, and nothing else of any
/// star moves.
#[test]
fn a_galaxy_s_stars_take_its_v_magnitudes() {
    let (mut white_dwarfs, mut evolved) = (0, 0);
    for record in records() {
        let (was, now) = (
            SystemStars::generate(held(), record).summary_at(UniverseTime::EPOCH),
            SystemStars::generate(a4_galaxy(), record).summary_at(UniverseTime::EPOCH),
        );
        assert_eq!(was.stars().len(), now.stars().len());
        let v_of = |s: &StarSummary| v_bits(s.absolute_magnitude_v());
        for (old, new) in was.stars().iter().zip(now.stars()) {
            assert_eq!(old.state(), new.state(), "{:?}", record.id());
            assert_eq!(old.classification(), new.classification());
            assert_eq!(v_bits(old.colour_b_v()), v_bits(new.colour_b_v()));
            let phase = old.state().phase();
            match phase {
                Phase::HeliumWhiteDwarf
                | Phase::CarbonOxygenWhiteDwarf
                | Phase::OxygenNeonWhiteDwarf => {
                    assert_eq!(old.absolute_magnitude_v(), None);
                    white_dwarfs += usize::from(new.absolute_magnitude_v().is_some());
                }
                Phase::Protostar
                | Phase::PreMainSequence
                | Phase::MainSequence
                | Phase::Substellar
                | Phase::NeutronStar
                | Phase::BlackHole
                | Phase::NoRemnant => assert_eq!(v_of(old), v_of(new), "{phase:?}"),
                Phase::HertzsprungGap
                | Phase::FirstGiantBranch
                | Phase::CoreHeliumBurning
                | Phase::EarlyAgb
                | Phase::ThermallyPulsingAgb
                | Phase::HeliumMainSequence
                | Phase::HeliumHertzsprungGap
                | Phase::HeliumGiantBranch
                | Phase::PostAgb => evolved += usize::from(v_of(old) != v_of(new)),
            }
        }
    }
    eprintln!(
        "{} records: {white_dwarfs} white dwarfs gain a V, {evolved} evolved stars move",
        records().len()
    );
    assert!(white_dwarfs > 0 && evolved > 0, "{white_dwarfs} {evolved}");
}

/// D2, the host stars' discs: under A4 a white dwarf's disc is lit by the Montreal grid's V, not
/// the dwarf table's stand-in, and only its luminance moves.
#[test]
fn a_white_dwarf_s_disc_takes_its_galaxy_s_v() {
    let moved = records().iter().find_map(|record| {
        let stars = SystemStars::generate(held(), record);
        let state = stars.state_at(UniverseTime::EPOCH)?;
        if !state.stars().iter().any(|s| is_white_dwarf(s.phase())) {
            return None;
        }
        let (was, now) = (
            host_discs(held(), &stars, UniverseTime::EPOCH),
            host_discs(a4_galaxy(), &stars, UniverseTime::EPOCH),
        );
        assert_eq!(was.len(), now.len());
        let luminance = |discs: &[HostDisc]| -> Vec<u64> {
            discs
                .iter()
                .flat_map(|d| d.mean_luminance().map(|l| bits(l.value())))
                .collect()
        };
        let radii = |discs: &[HostDisc]| -> Vec<u64> {
            discs.iter().map(|d| bits(d.radius().value())).collect()
        };
        assert_eq!(radii(&was), radii(&now));
        (luminance(&was) != luminance(&now)).then_some(was.len())
    });
    assert!(moved.is_some(), "no white dwarf's disc by the Sun moved");
}

/// D4: A4's galaxy, built twice, gives the same stars.
#[test]
fn the_a4_arm_gives_the_same_stars_twice() {
    let again = held().clone().with_rules(a4_galaxy().rules());
    assert_eq!(&again, a4_galaxy());
    for record in records().iter().take(60) {
        assert_eq!(
            SystemStars::generate(&again, record).summary_at(UniverseTime::EPOCH),
            SystemStars::generate(a4_galaxy(), record).summary_at(UniverseTime::EPOCH),
            "{:?}",
            record.id()
        );
    }
}

/// The census's white-dwarf ceiling holds: no white dwarf the generator makes is brighter in V
/// under A4 than `WHITE_DWARF_BRIGHTEST_V`. A white dwarf is brightest as it forms, so the test
/// reads each one from its first instant on, at ages even in log: of every single star's track
/// from 0.7 to 12 M☉ over the phase envelope's \[Fe/H\] nodes and η nodes, with and without the
/// companion-stripped mark; and of the binary engine's pairs, two samples of every cell of the
/// pair-light tables' grids, which reach the lightest helium dwarfs.
#[test]
#[ignore = "slow: 10,648 tracks and two pairs a pair-light cell, from each white dwarf's birth"]
fn no_white_dwarf_is_brighter_than_the_census_bound() {
    /// The brightest white dwarf read so far, what it was, and how many were read.
    #[derive(Default)]
    struct Brightest {
        v: Option<f64>,
        what: String,
        seen: u64,
    }
    impl Brightest {
        fn look(&mut self, state: &StarState, what: impl Fn() -> String) {
            if !is_white_dwarf(state.phase()) {
                return;
            }
            if let Some(v) = absolute_magnitude_v_under(state, a4_rules()) {
                self.seen += 1;
                if self.v.is_none_or(|b| v.value() < b) {
                    self.v = Some(v.value());
                    self.what = format!("{} {state:?}", what());
                }
            }
        }
    }
    let rules = a4_rules();
    let mut brightest = Brightest::default();
    let offsets: Vec<f64> = (0..=130)
        .map(|k| math::exp10(f64::from(k) * 0.1 - 3.0))
        .collect();
    let masses = 120;
    for i in 0..=masses {
        let m = 0.7 * math::powf(12.0 / 0.7, f64::from(i) / f64::from(masses));
        for &fe_h in &FE_H_NODES {
            let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
            for &z in &ETA_NODES {
                for stripped in [
                    StarDrawsParts::MEDIAN.stripped,
                    crate::rng::Mark::from_word(0),
                ] {
                    let draws = StarDraws::from_parts(StarDrawsParts {
                        eta: StandardNormal::new(z).expect("finite"),
                        stripped,
                        ..StarDrawsParts::MEDIAN
                    });
                    let track = Track::full_with(
                        SolarMasses::new(m),
                        &composition,
                        &draws,
                        TrackOptions::default().with_rules(rules),
                    );
                    let Some(life) = track.lifetime() else {
                        continue;
                    };
                    for &dt in &offsets {
                        let age = life.value() + dt;
                        brightest.look(&track.state_at(Years::new(age)), || {
                            format!("{m:.3} M☉ [Fe/H] {fe_h} η {z} at {dt:.3e} yr")
                        });
                    }
                }
            }
        }
    }
    let singles = brightest.seen;
    let until = Years::new(1.5e10);
    for layer in LAYERS {
        let grid = PairGrid::of(layer).expect("a pair-light layer");
        for cell in 0..grid.cells() {
            for index in 0..2 {
                let input = sample_input(&grid, cell, index, 0x0a4_0a4).with_rules(rules);
                let timeline = evolve(&input, until);
                for segment in timeline.segments() {
                    let (start, end) = (segment.start().value(), segment.end().value());
                    for &dt in &offsets {
                        let age = start + dt;
                        if age > end {
                            break;
                        }
                        let state = timeline.state_at(Years::new(age));
                        for star in state.stars() {
                            brightest
                                .look(star, || format!("{layer:?} cell {cell} sample {index}"));
                        }
                    }
                }
            }
        }
    }
    let v = brightest.v.expect("white dwarfs were read");
    eprintln!(
        "white dwarfs read: {singles} of single tracks, {} of pairs; the brightest M_V {v:.3}: {}",
        brightest.seen - singles,
        brightest.what
    );
    assert!(
        v >= WHITE_DWARF_BRIGHTEST_V.value(),
        "{v} ({}) is brighter than the census's ceiling",
        brightest.what
    );
}
