//! The state of a free-floating brown dwarf and what a rogue planet carries (plan 13, P13.T5.a and
//! T5.d).
//!
//! A brown dwarf takes plan 06's stellar stage as a star does: its metallicity draw, P06.T13's
//! cooling fits at its age plus the clock time, and its classification, with no companion. The
//! tests check the classes (none earlier than M6 over 10⁴ objects), the fits' monotony in age and
//! mass, the state's continuity across the clock window and across 0.08 M☉ against a layer-A star,
//! the brief a range query carries, and pin a histogram of classes for the old thin disc and the
//! halo. A rogue planet's record carries its mass, age, population and metallicity, and nothing
//! derived.

#[expect(
    dead_code,
    reason = "the substellar state tests use the Sun-like point and the block of cells alone"
)]
mod common;

use std::collections::BTreeMap;
use std::sync::OnceLock;

use common::{objects_in_block, sunlike_point};
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{SystemKind, SystemRecord};
use hyperion_sim::galaxy::{Galaxy, PointLy, Population};
use hyperion_sim::id::Layer;
use hyperion_sim::stellar::brief::BriefModel;
use hyperion_sim::stellar::classify::{SpectralLetter, SpectralType};
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::system::{StarModel, SystemExistence, SystemStars, draw_metallicity};
use hyperion_sim::stellar::{Composition, ObjectKind, Phase, StarState};
use hyperion_sim::time::{CLOCK_WINDOW_H, UniverseTime};
use hyperion_sim::units::consts::{EARTH_MASS_KG, SOLAR_MASS_KG};
use hyperion_sim::units::{Dex, HeliumExcess, LightYears, SolarMasses, Years};
use hyperion_sim::{GENERATOR_VERSION, Seed, math};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The seed of the galaxy these objects are placed in.
const SEED: u64 = 0x0d13_05a0_0000_0000;

fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    })
}

/// Some 10⁴ brown dwarfs about the Sun-like point, a cube of 18 16-ly cells a side.
fn local_brown_dwarfs() -> &'static [SystemRecord] {
    static RECORDS: OnceLock<Vec<SystemRecord>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        let sun = sunlike_point(galaxy()).to_light_years_f64();
        let at = [sun[0] - 144.0, sun[1] - 144.0, sun[2] - 144.0];
        objects_in_block(galaxy(), Layer::BrownDwarf, at, 18)
    })
}

/// The primary's state at `t` of a born system.
fn state_at(stars: &SystemStars, t: UniverseTime) -> Option<StarState> {
    stars.primary().state_at(t)
}

#[test]
fn brown_dwarfs_take_the_stellar_stage_alone() {
    let galaxy = galaxy();
    let records = local_brown_dwarfs();
    assert!(records.len() >= 10_000, "{} brown dwarfs", records.len());
    for record in records.iter().take(500) {
        assert_eq!(record.kind(), SystemKind::BrownDwarf);
        let stars = SystemStars::generate(galaxy, record);
        assert_eq!(stars.star_count(), 1);
        assert_eq!(stars.stars().len(), 1);
        let primary = stars.primary();
        assert_eq!(primary.initial_mass(), record.primary_initial_mass());
        assert_eq!(*primary.composition(), draw_metallicity(galaxy, record));
        assert_eq!(primary.age_at_epoch(), record.age_at_epoch());
        assert_eq!(primary.death(), None, "a brown dwarf never dies");
        let summary = stars.summary_at(UniverseTime::EPOCH);
        if summary.existence() == SystemExistence::Exists {
            let star = &summary.stars()[0];
            assert_eq!(star.state().phase(), Phase::Substellar);
            assert!(
                matches!(star.kind(), ObjectKind::Substellar | ObjectKind::Dwarf),
                "{:?}",
                star.kind()
            );
            // The brief a range query carries is the system's own, of one object.
            let brief = BriefModel::new(galaxy, record)
                .brief_at(UniverseTime::EPOCH)
                .expect("born");
            assert_eq!(Some(brief), stars.brief_at(UniverseTime::EPOCH));
            assert_eq!(brief.star_count(), 1);
        }
    }
}

/// The warm edge of M5.5 and of M6 on the dwarf scale the classifier reads (Pecaut and Mamajek
/// 2013, Mamajek's table v2022.04.16): a code of 65.5 is written from 2,994 K down, 66 (M6V, 2,810
/// K at its centre) from 2,869 K down (the orchestrator's ruling 135.1).
const M5_5_WARM_EDGE_K: f64 = 2_994.0;
const M6_WARM_EDGE_K: f64 = 2_869.0;

/// The age from which every layer-F object is M6 or later: 0.2 Gyr, by when BHAC15's 0.08 M☉ has
/// fallen below M6's warm edge (at 0.12 Gyr; ruling 135.4).
const M6_FROM_AGE_YR: f64 = 2.0e8;

/// Plan 13, P13.T5.a, as ruling 135.4 restates it: over 10⁴ layer-F objects, every one is M5.5 or
/// later, and every one older than 0.2 Gyr is M6 or later.
///
/// The plan's "none earlier than M6" read the young scale's M6 (Luhman et al. 2003, 2,990 K)
/// through the dwarf scale the classifier keeps: a young object near the hydrogen-burning limit,
/// still contracting at ruling 42.1's Hayashi temperature, is M5.5 on it (ruling 135.1–2). The layer
/// is the mass band [13 `M_Jup`, 0.08 M☉) and holds stars above the model's hydrogen-burning limit
/// too (ruling 135.3), so the test prints the counts of each kind and asserts no share of either.
#[test]
fn every_layer_f_object_is_m5_5_or_later_and_m6_or_later_from_0_2_gyr() {
    let galaxy = galaxy();
    let mut classed = 0_u32;
    let mut kinds: BTreeMap<String, u32> = BTreeMap::new();
    let mut coolest = f64::NEG_INFINITY;
    for record in local_brown_dwarfs() {
        let summary = SystemStars::generate(galaxy, record).summary_at(UniverseTime::EPOCH);
        let Some(star) = summary.stars().first() else {
            continue;
        };
        let SpectralType::Sequence(code) = star.classification().spectral_type() else {
            panic!(
                "a layer-F object is typed on the sequence: {}",
                star.classification()
            );
        };
        let written = code.to_half_subtype().value();
        coolest = coolest.max(written);
        let age = record.age_at_epoch().value();
        let describe = || {
            format!(
                "{:#x} of {} M☉ at {age} yr, [Fe/H] {}, is {}",
                record.id().raw(),
                record.primary_initial_mass().value(),
                summary.composition().fe_h().value(),
                star.classification()
            )
        };
        assert!(written >= 65.5, "(a) {}", describe());
        if age > M6_FROM_AGE_YR {
            assert!(written >= 66.0, "(b) {}", describe());
        }
        *kinds.entry(format!("{:?}", star.kind())).or_insert(0) += 1;
        classed += 1;
    }
    eprintln!("layer-F objects classed: {classed}, by kind {kinds:?}");
    assert!(classed >= 10_000, "{classed} layer-F objects classed");
    assert!(
        coolest >= 90.0,
        "the coolest is {coolest}: old brown dwarfs reach Y"
    );
}

/// Ruling 135.4 (c): the fits' effective temperature stays below M5.5's warm edge everywhere over
/// the layer's band, 1 Myr–13 Gyr and every metal fraction the formulae take (Z from 10⁻⁴ to 0.03),
/// and below M6's from 0.2 Gyr. A deterministic check of the model, independent of the placed
/// sample.
#[test]
fn the_fit_stays_below_m5_5_and_below_m6_from_0_2_gyr() {
    let masses: Vec<f64> = (0..=60)
        .map(|k| 0.0124 * math::powf(0.08 / 0.0124, f64::from(k) / 60.0))
        .map(|m: f64| m.min(0.08 * (1.0 - 1e-9)))
        .collect();
    let ages: Vec<f64> = (0..=82)
        .map(|k| 1.0e6 * math::powf(10.0, f64::from(k) / 20.0))
        .filter(|&age| age <= 1.3e10)
        .collect();
    // [Fe/H] −2.5 lies below the formulae's floor, Z = 10⁻⁴, and +0.18 reaches their top, 0.03.
    let fe_hs = [-2.5, -2.0, -1.5, -1.0, -0.5, -0.25, 0.0, 0.1, 0.18];
    let mut hottest = (0.0, 0.0, 0.0, 0.0);
    let mut hottest_old = (0.0, 0.0, 0.0, 0.0);
    for fe_h in fe_hs {
        let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        for &m in &masses {
            for &age in &ages {
                let teff = StarModel::new(
                    SolarMasses::new(m),
                    composition,
                    StarDraws::median(),
                    Years::new(age),
                )
                .expect("a layer-F mass")
                .state_at(UniverseTime::EPOCH)
                .expect("formed")
                .effective_temperature()
                .value();
                if teff > hottest.0 {
                    hottest = (teff, m, age, fe_h);
                }
                if age >= M6_FROM_AGE_YR && teff > hottest_old.0 {
                    hottest_old = (teff, m, age, fe_h);
                }
            }
        }
    }
    eprintln!(
        "hottest {:.0} K ({} M☉, {:e} yr, [Fe/H] {}); from 0.2 Gyr {:.0} K ({} M☉, {:e} yr, [Fe/H] {})",
        hottest.0,
        hottest.1,
        hottest.2,
        hottest.3,
        hottest_old.0,
        hottest_old.1,
        hottest_old.2,
        hottest_old.3
    );
    assert!(hottest.0 < M5_5_WARM_EDGE_K, "{hottest:?}");
    assert!(hottest_old.0 < M6_WARM_EDGE_K, "{hottest_old:?}");
}

/// Effective temperature falls monotonically with age at fixed mass, and rises with mass at fixed
/// age, over the band and every metallicity the galaxy draws.
#[test]
fn temperature_falls_with_age_and_rises_with_mass() {
    let masses: Vec<f64> = (0..=40)
        .map(|k| 0.0124 * math::powf(0.08 / 0.0124, f64::from(k) / 40.0))
        .collect();
    let ages: Vec<f64> = (0..=48)
        .map(|k| 1.0e6 * math::powf(10.0, f64::from(k) / 12.0))
        .filter(|&age| age <= 1.4e10)
        .collect();
    for fe_h in [-2.5, -1.5, -0.5, 0.0, 0.4] {
        let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        let teff = |m: f64, age: f64| {
            let model = StarModel::new(
                SolarMasses::new(m),
                composition,
                StarDraws::median(),
                Years::new(age),
            )
            .expect("a brown dwarf's mass");
            model
                .state_at(UniverseTime::EPOCH)
                .expect("born")
                .effective_temperature()
                .value()
        };
        for &m in &masses {
            for pair in ages.windows(2) {
                assert!(
                    teff(m, pair[1]) <= teff(m, pair[0]),
                    "[Fe/H] {fe_h}, {m} M☉: {} K at {} yr above {} K at {} yr",
                    teff(m, pair[1]),
                    pair[1],
                    teff(m, pair[0]),
                    pair[0]
                );
            }
        }
        for &age in &ages {
            for pair in masses.windows(2) {
                assert!(
                    teff(pair[1], age) >= teff(pair[0], age),
                    "[Fe/H] {fe_h} at {age} yr: {} M☉ cooler than {} M☉",
                    pair[1],
                    pair[0]
                );
            }
        }
    }
}

/// A brown dwarf's state moves across the clock window only as its age does: over ±1,000 yr the
/// luminosity, radius and temperature change by at most their fits' slopes in age allow, and in
/// the same direction on both sides.
#[test]
fn state_is_continuous_across_the_clock_window() {
    let galaxy = galaxy();
    let early = UniverseTime::EPOCH
        .checked_sub(CLOCK_WINDOW_H)
        .expect("the window's start");
    let late = UniverseTime::EPOCH
        .checked_add(CLOCK_WINDOW_H)
        .expect("the window's end");
    let mut checked = 0;
    for record in local_brown_dwarfs().iter().take(2_000) {
        let stars = SystemStars::generate(galaxy, record);
        let (Some(before), Some(now), Some(after)) = (
            state_at(&stars, early),
            state_at(&stars, UniverseTime::EPOCH),
            state_at(&stars, late),
        ) else {
            continue;
        };
        // d ln L ÷ d ln t is at most 1.3 in size (Burrows et al.'s t^−1.3), so over H at age t the
        // luminosity moves by at most 1.3 H ÷ t, and the radius and temperature by less; ages
        // under the fits' 1 Myr hold not at all.
        let age = record.age_at_epoch().value() - 1_000.0;
        let bound = 1.5 * 1_000.0 / age.max(1.0e6) + 1e-12;
        let rel = |a: f64, b: f64| (a / b - 1.0).abs();
        let l = |s: &StarState| s.luminosity().value();
        let r = |s: &StarState| s.radius().value();
        let t = |s: &StarState| s.effective_temperature().value();
        for (name, f) in [
            ("L", &l as &dyn Fn(&StarState) -> f64),
            ("R", &r),
            ("T", &t),
        ] {
            assert!(
                rel(f(&before), f(&now)) <= bound && rel(f(&after), f(&now)) <= bound,
                "{:#x}: {name} {} / {} / {} over ±H at {age} yr",
                record.id().raw(),
                f(&before),
                f(&now),
                f(&after)
            );
            // Cooling only: nothing rises with age.
            assert!(
                f(&after) <= f(&now) && f(&now) <= f(&before),
                "{name} rises"
            );
        }
        checked += 1;
    }
    assert!(checked > 1_500, "{checked} brown dwarfs checked");
}

/// A brown dwarf at the layer's upper edge and a layer-A star at its lower edge, 0.08 M☉, of the
/// same age and metallicity, agree within 5% in luminosity, radius and temperature, for the ages
/// and compositions of the placed brown dwarfs.
///
/// The two sides are taken 10⁻⁶ of the mass apart: near the hydrogen-burning limit the luminosity
/// rises as steeply as m^7.6 (a 0.0795 M☉ object at 9.5 Gyr is 5.5% fainter than one of 0.0801),
/// so a wider gap would measure the slope, not a step.
#[test]
fn state_is_continuous_in_mass_across_the_layers() {
    let galaxy = galaxy();
    let edge = MassBand::A.lo();
    let mut checked = 0;
    for record in local_brown_dwarfs().iter().step_by(10) {
        let composition = draw_metallicity(galaxy, record);
        let at = |m: f64| {
            StarModel::new(
                SolarMasses::new(m),
                composition,
                StarDraws::median(),
                record.age_at_epoch(),
            )
            .expect("a mass the fits cover")
            .state_at(UniverseTime::EPOCH)
        };
        let (Some(dwarf), Some(star)) = (at(edge * (1.0 - 1e-6)), at(edge)) else {
            continue;
        };
        for (name, a, b) in [
            ("L", dwarf.luminosity().value(), star.luminosity().value()),
            ("R", dwarf.radius().value(), star.radius().value()),
            (
                "T",
                dwarf.effective_temperature().value(),
                star.effective_temperature().value(),
            ),
        ] {
            assert!(
                (a / b - 1.0).abs() < 0.05,
                "at {} yr, [Fe/H] {}: {name} {a} against {b}",
                record.age_at_epoch().value(),
                composition.fe_h().value()
            );
        }
        checked += 1;
    }
    assert!(checked >= 1_000, "{checked} ages and compositions");
}

/// An Earth-mass object at 26,000 ly has a tidal radius of 0.04–0.08 ly, so plan 02's
/// `tidal_radius` serves a rogue planet as it does a star.
#[test]
fn an_earth_mass_object_has_a_tidal_radius_of_hundredths_of_a_light_year() {
    let galaxy = galaxy();
    // Plan 02's own tests take the Sun's point on the x axis.
    let at = PointLy::new(26_000.0, 0.0, 0.0);
    let tidal = |m: f64| {
        LightYears::from(galaxy.potential().tidal_radius(SolarMasses::new(m), &at)).value()
    };
    let earth = tidal(EARTH_MASS_KG / SOLAR_MASS_KG);
    eprintln!("tidal radius of an Earth mass at 26,000 ly: {earth:.4} ly");
    assert!((0.04..=0.08).contains(&earth), "{earth} ly");
    // And a brown dwarf's lies between it and a star's.
    let brown = tidal(0.04);
    assert!(brown > earth && brown < tidal(1.0), "{brown} ly");
}

/// Pins the classes of born brown dwarfs, by letter and half subtype, for the old thin disc about
/// the Sun and the halo above it, with their mean age.
///
/// Checked by eye against the expectation that old brown dwarfs are mostly T and Y: the table
/// prints the shares, which are recorded in plan 13's as-built notes.
#[test]
fn brown_dwarf_classes_are_pinned() {
    let galaxy = galaxy();
    // The halo dominates the brown dwarfs 8,000 ly above the plane inside the solar circle.
    let halo_block = objects_in_block(galaxy, Layer::BrownDwarf, [0.0, 16_000.0, 8_000.0], 48);
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (name, population, records) in [
        (
            "old_thin_disc",
            Population::OldThinDisc,
            local_brown_dwarfs(),
        ),
        ("halo", Population::Halo, halo_block.as_slice()),
    ] {
        let mut letters: BTreeMap<SpectralLetter, u32> = BTreeMap::new();
        let mut subtypes: BTreeMap<u64, u64> = BTreeMap::new();
        let mut count = 0_u32;
        let mut age_sum = 0.0;
        for record in records.iter().filter(|r| r.population() == population) {
            let summary = SystemStars::generate(galaxy, record).summary_at(UniverseTime::EPOCH);
            let Some(star) = summary.stars().first() else {
                continue;
            };
            let SpectralType::Sequence(code) = star.classification().spectral_type() else {
                panic!("a brown dwarf is typed on the sequence");
            };
            let half = code.to_half_subtype();
            *letters.entry(half.letter()).or_insert(0) += 1;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "twice a code of 3–94 is a whole number of 6–188"
            )]
            let twice = (2.0 * half.value()) as u64;
            *subtypes.entry(twice).or_insert(0) += 1;
            count += 1;
            age_sum += record.age_at_epoch().value();
        }
        assert!(count >= 300, "{count} {name} brown dwarfs");
        w.line(&format!("{name}.count = {count}"));
        w.f64(&format!("{name}.mean_age_yr"), age_sum / f64::from(count));
        for (letter, n) in &letters {
            w.line(&format!("{name}.letter.{letter:?} = {n}"));
        }
        for (twice, n) in &subtypes {
            w.line(&format!("{name}.half_code.{twice} = {n}"));
        }
        let share =
            |letter| f64::from(letters.get(&letter).copied().unwrap_or(0)) / f64::from(count);
        eprintln!(
            "{name}: {count}, M {:.3} L {:.3} T {:.3} Y {:.3}",
            share(SpectralLetter::M),
            share(SpectralLetter::L),
            share(SpectralLetter::T),
            share(SpectralLetter::Y)
        );
    }
    golden!("stellar/brown_dwarf_classes", w.as_str());
}

/// Plan 13, P13.T5.d: a rogue planet's record is the whole of what the sim returns for it: its
/// mass, age, population and metallicity, and nothing derived. It has no stars, no brief and no
/// classification; plan 14 derives it as body `0x0000`.
#[test]
fn a_rogue_planet_carries_its_record_and_metallicity_alone() {
    let galaxy = galaxy();
    let sun = sunlike_point(galaxy).to_light_years_f64();
    let records = objects_in_block(galaxy, Layer::RoguePlanet, sun, 3);
    assert!(records.len() > 20, "{} rogue planets", records.len());
    for record in &records {
        assert_eq!(record.kind(), SystemKind::RoguePlanet);
        let mass = record.primary_initial_mass().value();
        assert!((1.0e-6..=0.0125).contains(&mass), "{mass} M☉");
        assert!(record.age_at_epoch().value().is_finite());
        // Its metallicity is drawn as a star's, from its component at its position and age.
        let composition = draw_metallicity(galaxy, record);
        assert!(composition.fe_h().value().abs() < 3.0);
        assert_eq!(composition, draw_metallicity(galaxy, record));
    }
}

/// A brown dwarf's stars, summary and brief do not depend on what was generated before them, as
/// the server's system and brief caches need (the sim-determinism rule for anything behind a
/// cache).
#[test]
fn brown_dwarfs_are_the_same_whatever_the_order() {
    let galaxy = galaxy();
    let records: Vec<SystemRecord> = local_brown_dwarfs().iter().take(40).copied().collect();
    let late = UniverseTime::EPOCH
        .checked_add(CLOCK_WINDOW_H)
        .expect("the window's end");
    hyperion_testkit::order::assert_order_independent(&records, |record| {
        let stars = SystemStars::generate(galaxy, record);
        (
            stars.summary_at(late),
            stars.brief_at(UniverseTime::EPOCH),
            BriefModel::new(galaxy, record).brief_at(UniverseTime::EPOCH),
        )
    });
}
