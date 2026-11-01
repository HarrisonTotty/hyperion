//! The tests of P06.T24.b's young stars' and S and carbon stars' classes, which every reader takes
//! through `system::peculiar_extras`:
//!
//! - **D4.** The same seed twice gives the same output.
//! - The pinned stars' summaries, briefs and range briefs are the same in any order, and the carbon
//!   star's class from its built pulses is its brief's.
//!
//! Readers that cannot move: `StarModel::variability_at` and the summary's variability, since no
//! variability reads a young, S or carbon class; the range brief's main-sequence route, whose
//! primary is on the main sequence through the whole clock window; and its fate-table and remnant
//! routes, whose primaries are dead.

use std::sync::OnceLock;

use hyperion_testkit::order::assert_order_independent;

use super::*;
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

/// The fixture.
fn fixture() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    })
}

/// A grid record of the young thin disc at the solar circle, candidate `index` of one layer-C
/// cell, of `mass` M☉ and `age` years at the epoch.
fn record(index: u32, mass: f64, age: f64) -> SystemRecord {
    let key = CellKey::new(Layer::C, [0, 812, 0]).expect("a cell of the grid");
    SystemRecord::from_parts(
        key.candidate_id(index).expect("a candidate of the cell"),
        GalacticPosition::from_light_years([0.0, 26_000.0, 30.0]).expect("in range"),
        SystemOrigin::Grid(
            fixture()
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
/// class at the epoch is `wanted`, with the age `age_of` gives it from its
/// candidate.
fn exact_record(
    mass: f64,
    age_of: impl Fn(u32) -> f64,
    wanted: impl Fn(Option<PeculiarClass>) -> bool,
) -> SystemRecord {
    (0..64)
        .map(|index| record(index, mass, age_of(index)))
        .find(|record| {
            BriefModel::new(fixture(), record).route() == BriefRoute::Exact
                && wanted(primary_class(fixture(), record))
        })
        .expect("a candidate of 64 is single, or certified, and of the class")
}

/// The peculiar class of `record`'s primary at the epoch in `galaxy`, from its summary.
fn primary_class(galaxy: &Galaxy, record: &SystemRecord) -> Option<PeculiarClass> {
    let summary = SystemStars::generate(galaxy, record).summary_at(UniverseTime::EPOCH);
    summary.stars()[0].classification().peculiar_class()
}

/// A single 1 M☉ star 2 Myr old: a T Tauri star.
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
            draw_metallicity(fixture(), &probe),
            primary_draws(fixture(), &probe),
            Years::new(1.2e10),
        )
        .expect("in range");
        let clock = ThermalPulseClock::of(&dead).expect("a 3 M☉ star pulses");
        let (start, end) = (clock.start_age().value(), clock.end_age().value());
        age = start + 0.95 * (end - start);
    }
    age
}

/// A single 3 M☉ star late on its thermally pulsing AGB: a carbon star.
fn carbon_star() -> &'static SystemRecord {
    static RECORD: OnceLock<SystemRecord> = OnceLock::new();
    RECORD.get_or_init(|| {
        exact_record(3.0, late_agb_age, |class| {
            class == Some(PeculiarClass::Carbon)
        })
    })
}

/// D4: a galaxy, and the pinned stars' summaries in it, are the same twice, and the same as
/// [`fixture`]'s.
#[test]
fn the_classes_give_the_same_output_twice() {
    let build = || {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    };
    let (first, second) = (build(), build());
    assert_eq!(first, second);
    assert_eq!(&first, fixture());
    for record in [t_tauri(), carbon_star()] {
        let once = SystemStars::generate(&first, record).summary_at(UniverseTime::EPOCH);
        assert_eq!(
            once,
            SystemStars::generate(&second, record).summary_at(UniverseTime::EPOCH)
        );
        assert_eq!(
            once,
            SystemStars::generate(fixture(), record).summary_at(UniverseTime::EPOCH)
        );
    }
}

/// Order independence (sim-determinism's order rule; the server caches the stars): the pinned
/// stars' summaries, briefs and range briefs at five times across the clock window are the same
/// asked forwards and backwards, from one model of each.
#[test]
fn the_classes_are_the_same_in_any_order() {
    let built: Vec<(SystemStars, BriefModel)> = [t_tauri(), carbon_star()]
        .into_iter()
        .map(|record| {
            (
                SystemStars::generate(fixture(), record),
                BriefModel::new(fixture(), record),
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
/// times across the clock window. Every stellar 26 hold is on; the galaxy's stay the version's,
/// since the field share and the carve-out are read when the galaxy is built.
#[test]
fn with_its_events_composed_the_carbon_star_s_class_is_its_brief_s() {
    // Built under every 26 hold, since the field share and the carve-out are read when a galaxy is
    // built.
    let both = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::of_version(crate::GENERATOR_VERSION),
    )
    .expect("the Milky Way fixture's gas is mostly neutral");
    assert_eq!(
        both.rules().stellar,
        StellarRules::of_version(crate::GENERATOR_VERSION)
    );
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
