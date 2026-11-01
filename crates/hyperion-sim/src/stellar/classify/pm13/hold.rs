//! The tests of P06.T23.a's hold, [`DWARF_SEQUENCE_VERSION`]: its tripwire; the version's rules
//! reading the rows (D3); under the reduced sequence, the 26 arm, the fitted grids following the
//! rows to the table's acceptance, the plan's classification and photometry figures restated,
//! every reader on the record moving (D2), and the same galaxy built twice giving the same stars
//! (D4).
//!
//! The bit-identity of the version in force is held by the goldens (D1), which pin every reader
//! (`stellar/classify`, the systems' and the range rows' goldens among them), and by the rows' own
//! tests in `pm13`, `classify` and `photometry`, which read the held arm. The sky census's reader
//! is held by its own test, `the_census_reads_its_galaxy_s_dwarf_sequence`, and the host stars'
//! discs by [`a_host_disc_takes_its_galaxy_s_dwarf_sequence`].
//!
//! Readers that cannot be shown to move on the fixture: the binary engine's Be stars. A B star's
//! letter moves only within 0.05 of a subtype of B0 and A0, and a B star spins by its critical
//! fraction, which reads no colour; the pair's classification takes its input's rules
//! ([`a_pair_s_class_takes_its_input_s_rules`]).

use std::sync::OnceLock;

use hyperion_testkit::float::bits;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::features::centre::testing::milky_way_galaxy;
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::sky::disc::{HostDisc, host_discs};
use crate::stellar::binary::{BinaryInput, evolve};
use crate::stellar::brief::{BriefModel, BriefRoute};
use crate::stellar::classify::tests::{star, star_g};
use crate::stellar::classify::{
    ClassExtras, LuminosityClass, SpectralType, classify_under, subtype_from_teff_under,
};
use crate::stellar::draws::StarDraws;
use crate::stellar::photometry::{
    absolute_magnitude_v_under, absolute_v_of_state_under, bolometric_correction_v_under,
    colour_b_v_under,
};
use crate::stellar::rotation::rotation_under;
use crate::stellar::system::{StarSummary, SystemStars};
use crate::stellar::{Composition, Phase, StarState};
use crate::time::UniverseTime;
use crate::units::{
    GravitationalParameter, Kelvin, Magnitudes, Radians, Seconds, SolarMasses, Years,
};

/// The generator version's stellar rules: the rows.
const VERSION: StellarRules = StellarRules::of_version(GENERATOR_VERSION);

/// The version's stellar rules with this hold's arm alone turned on: the fitted grids.
const REDUCED: StellarRules = StellarRules {
    dwarf_sequence: DwarfSequenceRules::Reduced,
    ..VERSION
};

/// The table's acceptance bounds (`hyperion-fit`'s `dwarf_sequence` task): the code, BC<sub>V</sub>
/// and B − V.
const ACCEPTANCE: [f64; 3] = [0.05, 0.005, 0.005];

/// The sky fixture's galaxy under the version's rules.
fn held() -> &'static Galaxy {
    milky_way_galaxy()
}

/// [`held`]'s galaxy with the reduced sequence.
fn reduced() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        held().clone().with_rules(GalaxyRules {
            stellar: REDUCED,
            ..GalaxyRules::of_version(GENERATOR_VERSION)
        })
    })
}

/// Twelve records of each of three cells of every layer by the Sun.
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
                all.extend(cell.drain(..).take(12));
            }
        }
        all
    })
}

/// A star's spectral code in its summary, if it has one on the sequence.
fn code_of(summary: &StarSummary) -> Option<f64> {
    match summary.classification().spectral_type() {
        SpectralType::Sequence(code) => Some(code.value()),
        SpectralType::WhiteDwarf(_)
        | SpectralType::NeutronStar(_)
        | SpectralType::BlackHole
        | SpectralType::WolfRayet(_)
        | SpectralType::NoRemnant => None,
    }
}

/// P06.T23.a's licence fix is held until the 25 → 26 bump: the rules in force are version 24's,
/// under which the stage reads the source's rows; 25's do too, and 26's read the fitted grids.
#[test]
fn the_reduced_dwarf_sequence_is_held_only_until_the_26_bump() {
    assert!(
        GENERATOR_VERSION < DWARF_SEQUENCE_VERSION,
        "the 25 → 26 bump puts P06.T23.a's reduced dwarf sequence on the record (the fitted \
         `tables::dwarf_sequence`, in place of the source's rows, which the data rule does not \
         let the repository carry): delete `DWARF_SEQUENCE_VERSION`, `DwarfSequenceRules` with \
         its `Rows` arm, `StellarRules::dwarf_sequence`, the rows (`DWARF_SEQUENCE`, `DwarfRow`, \
         `place`, `rows_code_at`, `rows_column_at`) and the tests that read them, and this test, \
         in the bump's own commit; this module's tests of the reduced sequence become the \
         version's"
    );
    for v in 1..=64 {
        let expected = if v >= 26 {
            DwarfSequenceRules::Reduced
        } else {
            DwarfSequenceRules::Rows
        };
        assert_eq!(
            StellarRules::of_version(GeneratorVersion::new(v)).dwarf_sequence,
            expected,
            "version {v}"
        );
    }
    assert_eq!(VERSION.dwarf_sequence, DwarfSequenceRules::Rows);
    // Other holds turn on at 26 too (the helium table's, the Humphreys–Davidson limit's), so the
    // field alone is this hold's.
    assert_eq!(
        StellarRules::every_26_hold().dwarf_sequence,
        REDUCED.dwarf_sequence
    );
    assert_eq!(
        held().rules().stellar.dwarf_sequence,
        DwarfSequenceRules::Rows
    );
}

/// D3: the version's rules read the rows, bit for bit at every row, as `pm13` read them before
/// the hold; a default galaxy carries them.
#[test]
fn the_version_reads_the_rows() {
    let rows = DwarfSequence::of(VERSION);
    for row in DWARF_SEQUENCE {
        assert_eq!(bits(rows.code_at(row.teff_k)), bits(row.code()), "{row:?}");
        assert_eq!(
            rows.column_at(row.teff_k, Column::BcV).map(bits),
            row.bc_v.map(bits)
        );
        assert_eq!(
            rows.column_at(row.teff_k, Column::ColourBV).map(bits),
            row.colour_b_v.map(bits)
        );
    }
    assert_eq!(DwarfSequence::of(held().rules().stellar), rows);
    assert_ne!(DwarfSequence::of(REDUCED), rows);
}

/// Under the reduced sequence every quantity follows the rows' interpolation to the table's
/// acceptance from 250 to 44,900 K, gives a value exactly where the rows do, and keeps the rows'
/// values at the hottest point; above it both arms continue the same way.
#[test]
fn the_grids_follow_the_rows_to_the_tables_acceptance() {
    let (rows, grids) = (DwarfSequence::of(VERSION), DwarfSequence::of(REDUCED));
    let mut worst = [0.0_f64; 3];
    let mut t = 250.0_f64;
    while t <= 44_900.0 {
        let code = (grids.code_at(t) - rows.code_at(t)).abs();
        worst[0] = worst[0].max(code);
        for (k, column) in [(1, Column::BcV), (2, Column::ColourBV)] {
            match (grids.column_at(t, column), rows.column_at(t, column)) {
                (Some(g), Some(r)) => worst[k] = worst[k].max((g - r).abs()),
                (None, None) => {}
                (g, r) => panic!("{column:?} at {t} K: {g:?} against {r:?}"),
            }
        }
        t *= 1.000_2;
    }
    eprintln!("the largest change, code, BC_V and B - V: {worst:?}");
    for (w, bound) in worst.iter().zip(ACCEPTANCE) {
        assert!(*w <= bound, "{worst:?}");
    }
    assert_eq!(grids.hottest(), rows.hottest());
    for t in [44_900.0, 45_000.0, 1e5, 1e6] {
        for column in [Column::BcV, Column::ColourBV] {
            let k = Kelvin::new(t);
            let (g, r) = match column {
                Column::BcV => (
                    bolometric_correction_v_under(k, REDUCED),
                    bolometric_correction_v_under(k, VERSION),
                ),
                Column::ColourBV => (colour_b_v_under(k, REDUCED), colour_b_v_under(k, VERSION)),
            };
            assert_eq!(
                g.map(|m| bits(m.value())),
                r.map(|m| bits(m.value())),
                "{t} K"
            );
        }
    }
    assert_eq!(bits(grids.code_at(1e6)), bits(3.0));
    assert_eq!(bits(grids.code_at(100.0)), bits(94.0));
    assert_eq!(bits(grids.code_at(250.0)), bits(94.0));
    assert!(grids.column_at(1_710.0, Column::BcV).is_some());
    assert_eq!(grids.column_at(1_709.0, Column::BcV), None);
    assert!(grids.column_at(2_380.0, Column::ColourBV).is_some());
    assert_eq!(grids.column_at(2_379.0, Column::ColourBV), None);
}

/// The plan's figures under the reduced sequence (P06.T23's tests, restated): the Sun is G2V
/// with M<sub>V</sub> = 4.81 ± 0.05 and B − V 0.65; a Vega-like star (9,600 K, log g 4.0) is A0V;
/// a 4,300 K, log g 1.7 star is K III; 3,600 K and 10⁵ L☉ is M Iab or Ia; the dwarf subtype is
/// monotone in temperature; an L4 dwarf has a V magnitude, one below 1,710 K none.
#[test]
fn the_plans_figures_hold_under_the_reduced_sequence() {
    let sun = star(Phase::MainSequence, 1.0, 1.0, 5_772.0);
    let class = |state: &StarState| {
        classify_under(
            state,
            &Composition::SOLAR,
            &StarDraws::median(),
            ClassExtras::NONE,
            REDUCED,
        )
    };
    assert_eq!(class(&sun).to_string(), "G2V");
    let m_v = absolute_magnitude_v_under(&sun, REDUCED).unwrap().value();
    assert!((m_v - 4.81).abs() < 0.05, "{m_v}");
    let held_m_v = absolute_magnitude_v_under(&sun, VERSION).unwrap().value();
    assert!((m_v - held_m_v).abs() < ACCEPTANCE[1], "{m_v} {held_m_v}");
    let b_v = colour_b_v_under(Kelvin::new(5_772.0), REDUCED)
        .unwrap()
        .value();
    assert!((b_v - 0.65).abs() < 0.005, "{b_v}");
    let vega = star_g(Phase::MainSequence, 2.1, 4.0, 9_600.0);
    assert_eq!(class(&vega).to_string(), "A0V");
    let arcturus = star_g(Phase::FirstGiantBranch, 1.1, 1.7, 4_300.0);
    let written = class(&arcturus).to_string();
    assert!(
        written.starts_with('K') && written.ends_with("III"),
        "{written}"
    );
    let red = class(&star(Phase::CoreHeliumBurning, 15.0, 1e5, 3_600.0));
    assert!(red.to_string().starts_with('M'), "{red}");
    assert!(
        matches!(
            red.luminosity_class(),
            Some(LuminosityClass::Supergiant | LuminosityClass::LuminousSupergiant)
        ),
        "{red}"
    );
    let mut previous = 0.0;
    let mut t = 60_000.0_f64;
    while t > 100.0 {
        let code = subtype_from_teff_under(Kelvin::new(t), REDUCED)
            .unwrap()
            .value();
        assert!(code >= previous, "{t} K: {code} after {previous}");
        previous = code;
        t *= 0.9995;
    }
    let l4 = star(Phase::Substellar, 0.07, 2.0e-4, 1_870.0);
    assert!(absolute_magnitude_v_under(&l4, REDUCED).is_some());
    let cooler = star(Phase::Substellar, 0.06, 1e-4, 1_650.0);
    assert_eq!(absolute_magnitude_v_under(&cooler, REDUCED), None);
}

/// D2, the functions: every reader of the sequence moves under the reduced one, away from the
/// grids' ends: the dwarf subtype, a dwarf's class, BC<sub>V</sub>, B − V, M<sub>V</sub>, the
/// sky's V and a cool dwarf's braked rotation, through its zero-age colour.
#[test]
fn every_reader_moves_under_the_reduced_sequence() {
    let t = Kelvin::new(5_660.0);
    let as_bits = |m: Option<Magnitudes>| m.map(|m| bits(m.value()));
    assert_ne!(
        as_bits(bolometric_correction_v_under(t, REDUCED)),
        as_bits(bolometric_correction_v_under(t, VERSION))
    );
    assert_ne!(
        as_bits(colour_b_v_under(t, REDUCED)),
        as_bits(colour_b_v_under(t, VERSION))
    );
    let (a, b) = (
        subtype_from_teff_under(t, REDUCED).unwrap(),
        subtype_from_teff_under(t, VERSION).unwrap(),
    );
    assert_ne!(bits(a.value()), bits(b.value()));
    let dwarf = star(Phase::MainSequence, 0.95, 0.75, 5_660.0);
    assert_ne!(
        as_bits(absolute_magnitude_v_under(&dwarf, REDUCED)),
        as_bits(absolute_magnitude_v_under(&dwarf, VERSION))
    );
    assert_ne!(
        as_bits(absolute_v_of_state_under(&dwarf, REDUCED)),
        as_bits(absolute_v_of_state_under(&dwarf, VERSION))
    );
    let class = |rules| match classify_under(
        &dwarf,
        &Composition::SOLAR,
        &StarDraws::median(),
        ClassExtras::NONE,
        rules,
    )
    .spectral_type()
    {
        SpectralType::Sequence(code) => bits(code.value()),
        other => panic!("{other:?}"),
    };
    assert_ne!(class(REDUCED), class(VERSION));
    // A 0.6 M☉ dwarf at 1 Gyr, below the Kraft break: its braking reads its zero-age B − V.
    let cool = star(Phase::MainSequence, 0.6, 0.08, 4_000.0);
    let period = |rules| {
        rotation_under(
            &cool,
            &Composition::SOLAR,
            &StarDraws::median(),
            None,
            rules,
        )
        .unwrap()
        .period()
        .value()
    };
    assert_ne!(bits(period(REDUCED)), bits(period(VERSION)));
}

/// D2, a galaxy's stars: under the reduced sequence the summaries' types, V magnitudes, colours
/// and rotations move and nothing else of the stars does; the range brief still equals the
/// system's brief on every route but the table's, and some briefs move.
#[test]
fn a_galaxy_s_stars_take_its_dwarf_sequence() {
    let mut moved = [0_usize; 4];
    let mut briefs_moved = 0;
    for record in records() {
        let (was, now) = (
            SystemStars::generate(held(), record),
            SystemStars::generate(reduced(), record),
        );
        let (before, after) = (
            was.summary_at(UniverseTime::EPOCH),
            now.summary_at(UniverseTime::EPOCH),
        );
        assert_eq!(before.stars().len(), after.stars().len());
        let v_of = |s: &StarSummary| s.absolute_magnitude_v().map(|m| bits(m.value()));
        let colour_of = |s: &StarSummary| s.colour_b_v().map(|m| bits(m.value()));
        let period_of = |s: &StarSummary| s.rotation().map(|r| bits(r.period().value()));
        let type_of = |s: &StarSummary| code_of(s).map(bits);
        for (old, new) in before.stars().iter().zip(after.stars()) {
            assert_eq!(old.state(), new.state(), "{:?}", record.id());
            let changes = [
                v_of(old) != v_of(new),
                colour_of(old) != colour_of(new),
                period_of(old) != period_of(new),
                type_of(old) != type_of(new),
            ];
            for (count, changed) in moved.iter_mut().zip(changes) {
                *count += usize::from(changed);
            }
        }
        let model = BriefModel::new(reduced(), record);
        if model.route() != BriefRoute::Table {
            assert_eq!(
                model.brief_at(UniverseTime::EPOCH),
                now.brief_at(UniverseTime::EPOCH),
                "{:?}",
                record.id()
            );
        }
        briefs_moved += usize::from(
            BriefModel::new(held(), record).brief_at(UniverseTime::EPOCH)
                != model.brief_at(UniverseTime::EPOCH),
        );
    }
    eprintln!(
        "{} records: V moved {}, B - V {}, rotation {}, type {}; briefs {briefs_moved}",
        records().len(),
        moved[0],
        moved[1],
        moved[2],
        moved[3]
    );
    assert!(moved.iter().all(|&n| n > 0), "{moved:?}");
    assert!(briefs_moved > 0);
}

/// D2, the host stars' discs: under the reduced sequence a main-sequence star's disc is lit by its
/// galaxy's V, and only its luminance moves.
#[test]
fn a_host_disc_takes_its_galaxy_s_dwarf_sequence() {
    let moved = records()
        .iter()
        .filter(|record| record.layer() == Layer::C)
        .find_map(|record| {
            let stars = SystemStars::generate(held(), record);
            let (was, now) = (
                host_discs(held(), &stars, UniverseTime::EPOCH),
                host_discs(reduced(), &stars, UniverseTime::EPOCH),
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
            (!was.is_empty() && luminance(&was) != luminance(&now)).then_some(was.len())
        });
    assert!(moved.is_some(), "no disc of layer C by the Sun moved");
}

/// D2, a pair: the engine's classification takes its input's rules, which a pair in a galaxy
/// takes from it. An 8 and 6 M☉ pair on 30 days, 20 Myr old.
#[test]
fn a_pair_s_class_takes_its_input_s_rules() {
    let orbit = KeplerElements::from_period(
        Seconds::new(30.0 * 86_400.0),
        GravitationalParameter::from_solar_masses(SolarMasses::new(14.0)),
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::new(0.3), Radians::new(0.0), Radians::new(0.0))
            .expect("a valid orientation"),
        Radians::new(0.0),
    )
    .expect("a valid orbit");
    let until = Years::new(2.0e7);
    let input = BinaryInput::new(
        SolarMasses::new(8.0),
        SolarMasses::new(6.0),
        Composition::SOLAR,
        orbit,
        [StarDraws::median(), StarDraws::median()],
        until,
    )
    .expect("two stars in range with a finite age");
    for rules in [VERSION, REDUCED] {
        let timeline = evolve(&input.clone().with_rules(rules), until);
        assert_eq!(timeline.class_context(Years::new(1.0e7)).rules(), rules);
    }
}

/// D4: the reduced sequence's galaxy, built twice, gives the same stars.
#[test]
fn the_reduced_sequence_gives_the_same_stars_twice() {
    let again = held().clone().with_rules(reduced().rules());
    assert_eq!(&again, reduced());
    for record in records().iter().take(40) {
        assert_eq!(
            SystemStars::generate(&again, record),
            SystemStars::generate(reduced(), record),
            "{:?}",
            record.id()
        );
    }
}
