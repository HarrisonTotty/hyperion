//! The tests of P06.T24.b's hold ([`PECULIAR_CLASSES_VERSION`]), on `rules/tests.rs`' pattern:
//! its tripwire, and every reader of the young stars' and the S and carbon stars' classes taking
//! them from its galaxy's rules, through the one `match` of `system::peculiar_extras`.
//!
//! - **D3.** Under the version's rules a young star and a carbon star of the thermally pulsing
//!   AGB are classed as at 24: their extras are the same with and without their pulses' streams,
//!   and a default galaxy is one built under those rules explicitly.
//! - **D2.** Under the full arm each reader shows a mover: a star's own summary
//!   (`SystemStars::summary_at`), the system's brief (`SystemStars::brief_at`, which observed mode
//!   reports) and the range brief's exact route (`BriefModel`). The paired path is tested in
//!   `system.rs` (`a_paired_star_takes_a_young_class_and_no_carbon_class`): no grid star its pair
//!   has changed is before the main sequence, so no grid system can show it moving.
//! - **D4.** The same seed twice under the full arm gives the same output.
//! - **D1** is the goldens as they stand: no version takes the full arm yet, so nothing on the
//!   record moves.
//!
//! Readers that cannot move: `StarModel::variability_at` and the summary's variability, since no
//! variability reads a young, S or carbon class (checked below); the range brief's main-sequence
//! route, whose primary is on the main sequence through the whole clock window; and its fate-table
//! and remnant routes, whose primaries are dead.

use std::sync::OnceLock;

use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::GENERATOR_VERSION;
use crate::coords::GalacticPosition;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, SystemOrigin, SystemRecord};
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::{Galaxy, Population};
use crate::id::Layer;
use crate::rng::Seed;
use crate::stellar::brief::{BriefModel, BriefRoute};
use crate::stellar::classify::PeculiarClass;
use crate::stellar::events::thermal_pulse::ThermalPulseClock;
use crate::stellar::system::{Pulses, StarModel, SystemStars, draw_metallicity, primary_draws};
use crate::time::UniverseTime;
use crate::units::{SolarMasses, Years};

/// The fixture's universe.
const SEED: u64 = 0x0600_0024_b000_5eed;

/// The fixture under the generator version's rules.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    })
}

/// The version's galaxy rules with P06.T24.b's classes in force, and nothing else.
fn full_rules() -> GalaxyRules {
    let version = GalaxyRules::of_version(GENERATOR_VERSION);
    GalaxyRules {
        stellar: StellarRules {
            peculiar_classes: PeculiarClassRules::Full,
            ..version.stellar
        },
        ..version
    }
}

/// [`held`]'s galaxy with P06.T24.b's classes in force.
fn full() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| held().clone().with_rules(full_rules()))
}

/// A grid record of the young thin disc at the solar circle, candidate `index` of one layer-C
/// cell, of `mass` M☉ and `age` years at the epoch.
fn record(index: u32, mass: f64, age: f64) -> SystemRecord {
    let key = CellKey::new(Layer::C, [0, 812, 0]).expect("a cell of the grid");
    SystemRecord::from_parts(
        key.candidate_id(index).expect("a candidate of the cell"),
        GalacticPosition::from_light_years([0.0, 26_000.0, 30.0]).expect("in range"),
        SystemOrigin::Grid(
            held()
                .fields()
                .component_id(0)
                .expect("the young thin disc"),
        ),
        Population::YoungThinDisc,
        SolarMasses::new(mass),
        Years::new(age),
    )
}

/// The first record from candidate `from` whose brief takes the exact route and whose primary's
/// class at the epoch under the full arm is `wanted`, with the age `age_of` gives it from its
/// candidate.
fn exact_record(
    mass: f64,
    age_of: impl Fn(u32) -> f64,
    wanted: impl Fn(Option<PeculiarClass>) -> bool,
) -> SystemRecord {
    (0..64)
        .map(|index| record(index, mass, age_of(index)))
        .find(|record| {
            BriefModel::new(full(), record).route() == BriefRoute::Exact
                && wanted(primary_class(full(), record))
        })
        .expect("a candidate of 64 is single, or certified, and of the class")
}

/// The peculiar class of `record`'s primary at the epoch in `galaxy`, from its summary.
fn primary_class(galaxy: &Galaxy, record: &SystemRecord) -> Option<PeculiarClass> {
    let summary = SystemStars::generate(galaxy, record).summary_at(UniverseTime::EPOCH);
    summary.stars()[0].classification().peculiar_class()
}

/// A single 1 M☉ star 2 Myr old: a T Tauri star under the full arm.
fn t_tauri() -> &'static SystemRecord {
    static RECORD: OnceLock<SystemRecord> = OnceLock::new();
    RECORD.get_or_init(|| {
        exact_record(
            1.0,
            |_| 2e6,
            |class| matches!(class, Some(PeculiarClass::TTauri { .. })),
        )
    })
}

/// The age at the epoch of candidate `index` at 3 M☉ that puts its primary 95% of the way
/// through its thermally pulsing AGB: its composition is read at its age, so the two are
/// settled together.
fn late_agb_age(index: u32) -> f64 {
    let mut age = 4e8;
    for _ in 0..4 {
        let probe = record(index, 3.0, age);
        let dead = StarModel::new(
            SolarMasses::new(3.0),
            draw_metallicity(held(), &probe),
            primary_draws(held(), &probe),
            Years::new(1.2e10),
        )
        .expect("in range");
        let clock = ThermalPulseClock::of(&dead).expect("a 3 M☉ star pulses");
        let (start, end) = (clock.start_age().value(), clock.end_age().value());
        age = start + 0.95 * (end - start);
    }
    age
}

/// A single 3 M☉ star late on its thermally pulsing AGB: a carbon star under the full arm.
fn carbon_star() -> &'static SystemRecord {
    static RECORD: OnceLock<SystemRecord> = OnceLock::new();
    RECORD.get_or_init(|| {
        exact_record(3.0, late_agb_age, |class| {
            class == Some(PeculiarClass::Carbon)
        })
    })
}

/// P06.T24.b is held until the 25 → 26 bump: the rules in force are version 25's and before,
/// under which no star is a T Tauri, Herbig Ae/Be, S or carbon star.
#[test]
fn the_young_and_carbon_classes_are_held_only_until_the_26_bump() {
    assert!(
        GENERATOR_VERSION < PECULIAR_CLASSES_VERSION,
        "the 25 → 26 bump puts P06.T24.b on the record: the young stars' ` CTTS`, ` WTTS` and \
         ` HAeBe` and the AGB's `MS`, `S`, `SC` and `C-N` classes. Delete \
         `PECULIAR_CLASSES_VERSION`, `PeculiarClassRules` with its `Held` arm, \
         `StellarRules::peculiar_classes`, the `match` in `system::peculiar_extras` and this \
         test, in the bump's own checkpoint commit"
    );
    assert_eq!(
        held().rules().stellar.peculiar_classes,
        PeculiarClassRules::Held
    );
    assert_eq!(
        StellarRules::of_version(GENERATOR_VERSION).peculiar_classes,
        PeculiarClassRules::Held
    );
    assert_eq!(
        StellarRules::every_25_hold().peculiar_classes,
        PeculiarClassRules::Held
    );
    assert_eq!(
        StellarRules::every_26_hold().peculiar_classes,
        PeculiarClassRules::Full
    );
    assert_eq!(
        GalaxyRules::every_26_hold().stellar,
        StellarRules::every_26_hold()
    );
}

/// D3: under the version's rules the pinned T Tauri star and carbon star take no class of
/// P06.T24.b's, their extras are the same whether their pulses' streams are opened or not, and a
/// galaxy built under those rules explicitly gives the same stars.
#[test]
fn the_version_s_rules_class_young_and_agb_stars_as_at_24() {
    let explicit = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::of_version(GENERATOR_VERSION),
    )
    .expect("the Milky Way fixture builds");
    for record in [t_tauri(), carbon_star()] {
        let stars = SystemStars::generate(held(), record);
        assert_eq!(stars, SystemStars::generate(&explicit, record));
        assert_eq!(primary_class(held(), record), None, "{record:?}");
        let t = UniverseTime::EPOCH;
        let primary = stars.primary();
        let state = primary.state_at(t).expect("formed");
        let streams = stars.streams(crate::id::BodyId::new(record.id(), 0));
        assert_eq!(
            primary.class_extras_at(&state, t, Pulses::Opened(streams)),
            primary.class_extras_at(&state, t, Pulses::Unopened)
        );
    }
}

/// D2: under the full arm the T Tauri star's and the carbon star's own summaries, their systems'
/// briefs and the range brief's exact route all take the class, with the same state, kind and
/// variability; under either arm the three agree.
#[test]
fn each_reader_takes_the_classes_from_its_galaxy_s_rules() {
    for (record, class) in [(t_tauri(), "T Tauri"), (carbon_star(), "carbon")] {
        let t = UniverseTime::EPOCH;
        let read = |galaxy: &Galaxy| {
            let stars = SystemStars::generate(galaxy, record);
            let summary = stars.summary_at(t);
            let primary = summary.stars()[0].clone();
            let brief = stars.brief_at(t).expect("formed");
            let model = BriefModel::new(galaxy, record);
            assert_eq!(model.route(), BriefRoute::Exact, "{class}");
            assert_eq!(model.brief_at(t), Some(brief), "{class}");
            assert_eq!(
                (brief.kind(), brief.class().to_string()),
                (primary.kind(), primary.classification().to_string())
            );
            primary
        };
        let (before, after) = (read(held()), read(full()));
        assert_eq!(before.state(), after.state(), "{class}");
        assert_eq!(before.kind(), after.kind(), "{class}");
        assert_eq!(before.variability(), after.variability(), "{class}");
        assert_eq!(before.classification().peculiar_class(), None, "{class}");
        assert_ne!(
            before.classification().to_string(),
            after.classification().to_string(),
            "{class}"
        );
        let star = SystemStars::generate(full(), record);
        assert_eq!(
            star.primary().variability_at(t),
            after.variability(),
            "{class}"
        );
    }
}

/// D4: a galaxy built under the full arm, and the pinned stars' summaries in it, are the same
/// twice, and the same as [`held`]'s galaxy put under it with `Galaxy::with_rules`.
#[test]
fn the_full_arm_gives_the_same_output_twice() {
    let build = || {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            full_rules(),
        )
        .expect("the Milky Way fixture builds")
    };
    let (first, second) = (build(), build());
    assert_eq!(first, second);
    assert_eq!(&first, full());
    for record in [t_tauri(), carbon_star()] {
        let once = SystemStars::generate(&first, record).summary_at(UniverseTime::EPOCH);
        assert_eq!(
            once,
            SystemStars::generate(&second, record).summary_at(UniverseTime::EPOCH)
        );
        assert_eq!(
            once,
            SystemStars::generate(full(), record).summary_at(UniverseTime::EPOCH)
        );
    }
}

/// Order independence under the full arm (sim-determinism's order rule; the server caches the
/// stars): the pinned stars' summaries, briefs and range briefs at five times across the clock
/// window are the same asked forwards and backwards, from one model of each.
#[test]
fn the_full_arm_s_classes_are_the_same_in_any_order() {
    let built: Vec<(SystemStars, BriefModel)> = [t_tauri(), carbon_star()]
        .into_iter()
        .map(|record| {
            (
                SystemStars::generate(full(), record),
                BriefModel::new(full(), record),
            )
        })
        .collect();
    let keys: Vec<(usize, i64)> = (0..built.len())
        .flat_map(|i| [-1_000, -10, 0, 10, 1_000].map(|y| (i, y)))
        .collect();
    assert_order_independent(&keys, |&(i, years)| {
        let t = UniverseTime::from_julian_years(years).expect("inside the clock");
        let (stars, model) = &built[i];
        (stars.summary_at(t), stars.brief_at(t), model.brief_at(t))
    });
}

/// Both 26 holds on, as version 26 will have them: the carbon star's summary reads its class from
/// the pulses its events built (`system::Pulses::Built`), which must be the class its own series
/// gives (`Pulses::Opened`), as the system's brief and the range brief read it, at the epoch and at
/// times across the clock window.
#[test]
fn with_its_events_composed_the_carbon_star_s_class_is_its_brief_s() {
    let both = held().clone().with_rules(GalaxyRules::every_26_hold());
    assert_eq!(both.rules().stellar, StellarRules::every_26_hold());
    let record = carbon_star();
    let stars = SystemStars::generate(&both, record);
    let model = BriefModel::new(&both, record);
    let streams = stars.streams(crate::id::BodyId::new(record.id(), 0));
    for years in [-1_000, -300, 0, 450, 1_000] {
        let t = UniverseTime::from_julian_years(years).expect("inside the clock");
        let summary = stars.summary_at(t);
        let primary = &summary.stars()[0];
        assert!(primary.active_events().is_some(), "composed at 26");
        let brief = stars.brief_at(t).expect("formed");
        assert_eq!(
            primary.classification().to_string(),
            brief.class().to_string()
        );
        assert_eq!(model.brief_at(t), Some(brief));
        let state = stars.primary().state_at(t).expect("formed");
        let opened = stars
            .primary()
            .class_extras_at(&state, t, Pulses::Opened(streams));
        let events =
            crate::stellar::events::StarEvents::of(stars.primary(), both.seed(), streams.body());
        assert_eq!(
            stars
                .primary()
                .class_extras_at(&state, t, Pulses::Built(events.pulses())),
            opened
        );
    }
    assert_eq!(
        stars.summary_at(UniverseTime::EPOCH).stars()[0]
            .classification()
            .peculiar_class(),
        Some(PeculiarClass::Carbon)
    );
}
