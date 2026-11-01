//! The tests of P06.T28.g's hold ([`STAR_EVENTS_VERSION`]), on `rules/tests.rs`' pattern: its
//! tripwire, and the summary composing a star's events from its galaxy's rules, through the one
//! `match` of `system::star_summary`.
//!
//! - **D3.** Under the version's rules a summary holds no events and no event light, as at 24.
//! - **D2.** Under the composed arm the summary's reader shows a mover: a star's own summary
//!   (`SystemStars::summary_at`), which observed mode and the server's answer read, holds its
//!   active events and their light, its state and the system's brief unchanged. The server's wire
//!   is tested in `hyperion-server` (`convert::stellar`). A star its pair has changed takes no
//!   events (`star_summary_with`): no reader of it can move.
//! - **D4.** The same seed twice under the composed arm gives the same summaries, and a star
//!   evaluated cold at t = +900 years equals one evaluated after 100 earlier times.
//! - **D1** is the goldens as they stand: no version takes the composed arm yet.
//!
//! Readers that cannot move: the system's brief and the range brief, which read the track's state
//! (design note 14), and `StarModel::variability_at`.

use std::sync::OnceLock;

use super::*;
use crate::GENERATOR_VERSION;
use crate::coords::GalacticPosition;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, SystemOrigin, SystemRecord};
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::{Galaxy, Population};
use crate::id::Layer;
use crate::rng::Seed;
use crate::stellar::brief::BriefModel;
use crate::stellar::events::{STAR_EVENTS_VERSION, StarEventRules};
use crate::stellar::system::{StarSummary, SystemStars};
use crate::time::UniverseTime;
use crate::units::{SolarMasses, Years};

/// The fixture's universe.
const SEED: u64 = 0x0600_0028_9000_5eed;

/// The fixture under the generator version's rules.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    })
}

/// The version's galaxy rules with P06.T28.g's events composed, and nothing else.
fn composed_rules() -> GalaxyRules {
    let version = GalaxyRules::of_version(GENERATOR_VERSION);
    GalaxyRules {
        stellar: StellarRules {
            star_events: StarEventRules::Composed,
            ..version.stellar
        },
        ..version
    }
}

/// [`held`]'s galaxy with the events composed.
fn composed() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| held().clone().with_rules(composed_rules()))
}

/// Candidate `index` of one layer-C cell of the young thin disc at the solar circle, of `mass` M☉
/// and `age` years at the epoch.
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

fn years(y: i64) -> UniverseTime {
    UniverseTime::from_julian_years(y).expect("inside the clock")
}

/// The primary's summary of `stars` at `t`.
fn primary(stars: &SystemStars, t: UniverseTime) -> StarSummary {
    stars.summary_at(t).stars()[0].clone()
}

/// A saturated dwarf of 0.2 M☉ at 300 Myr and its first time, on whole hours of the epoch's day,
/// at which a flare runs under the composed arm.
fn flaring() -> &'static (SystemRecord, UniverseTime) {
    static FOUND: OnceLock<(SystemRecord, UniverseTime)> = OnceLock::new();
    FOUND.get_or_init(|| {
        let record = record(3, 0.2, 3e8);
        let stars = SystemStars::generate(composed(), &record);
        let hour = |h: i64| {
            UniverseTime::EPOCH
                .checked_add(crate::time::Span::from_seconds(3_600 * h))
                .expect("inside the clock")
        };
        let t = (0..24 * 30)
            .map(hour)
            .find(|&t| {
                primary(&stars, t)
                    .active_events()
                    .is_some_and(|e| !e.is_empty())
            })
            .expect("a saturated dwarf flares within a month");
        (record, t)
    })
}

/// P06.T28.g is held until the 25 → 26 bump: the rules in force are version 25's and before,
/// under which a summary holds no events.
#[test]
fn the_star_events_are_held_only_until_the_26_bump() {
    assert!(
        GENERATOR_VERSION < STAR_EVENTS_VERSION,
        "the 25 → 26 bump puts P06.T28.g on the record: each star's active events and the light \
         they make in its summary, and `active_events` on the wire. Delete `STAR_EVENTS_VERSION`, \
         `StarEventRules` with its `Held` arm, `StellarRules::star_events`, the `match` in \
         `system::star_summary` and this test, in the bump's own checkpoint commit; restate \
         `StarSummaryDto::active_events`' doc, and write the events in `stellar/summaries`"
    );
    assert_eq!(held().rules().stellar.star_events, StarEventRules::Held);
    assert_eq!(
        StellarRules::of_version(GENERATOR_VERSION).star_events,
        StarEventRules::Held
    );
    assert_eq!(
        StellarRules::every_26_hold().star_events,
        StarEventRules::Composed
    );
}

/// D3: under the version's rules the flaring dwarf's summary holds no events and no event light
/// at its flare's time, nor at any other.
#[test]
fn the_version_s_rules_compose_no_events() {
    let (record, t) = flaring();
    let stars = SystemStars::generate(held(), record);
    for when in [*t, UniverseTime::EPOCH, years(900)] {
        let summary = primary(&stars, when);
        assert_eq!(summary.active_events(), None);
        assert_eq!(summary.event_effects(), None);
    }
}

/// D2: under the composed arm the flaring dwarf's summary holds its running flare and the light
/// it makes, above the track's; its state, class and kind are the held arm's, and the system's
/// brief is the same bit for bit under either arm.
#[test]
fn the_summary_takes_its_events_from_its_galaxy_s_rules() {
    let (record, t) = flaring();
    let (held, composed) = (
        SystemStars::generate(held(), record),
        SystemStars::generate(composed(), record),
    );
    let (before, after) = (primary(&held, *t), primary(&composed, *t));
    assert_eq!(before.state(), after.state());
    assert_eq!(before.kind(), after.kind());
    assert_eq!(
        before.classification().to_string(),
        after.classification().to_string()
    );
    let events = after.active_events().expect("composed");
    assert!(events.iter().all(|e| e.is_active_at(*t)), "{events:?}");
    let effects = after.event_effects().expect("a flare's light");
    assert!(
        effects.luminosity() > after.state().luminosity(),
        "{effects:?}"
    );
    assert_eq!(held.brief_at(*t), composed.brief_at(*t));
}

/// D4: a galaxy built under the composed arm gives the flaring dwarf's summaries the same twice,
/// and the same as [`held`]'s galaxy put under it; and its summary at +900 years evaluated cold
/// equals one evaluated after 100 earlier times (the plan's test that no function in `stellar`
/// takes a list of past events).
#[test]
fn the_composed_arm_gives_the_same_output_twice_and_cold() {
    let build = || {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            composed_rules(),
        )
        .expect("the Milky Way fixture builds")
    };
    let (first, second) = (build(), build());
    assert_eq!(&first, composed());
    let (record, t) = flaring();
    let once = SystemStars::generate(&first, record);
    assert_eq!(
        once.summary_at(*t),
        SystemStars::generate(&second, record).summary_at(*t)
    );
    assert_eq!(
        once.summary_at(*t),
        SystemStars::generate(composed(), record).summary_at(*t)
    );
    let late = years(900);
    let cold = SystemStars::generate(composed(), record).summary_at(late);
    let warm = SystemStars::generate(composed(), record);
    for k in 0..100 {
        let _ = warm.summary_at(years(-1_000 + 19 * k));
    }
    assert_eq!(cold, warm.summary_at(late));
}

/// Under the composed arm `M_V` follows the event light at the track's bolometric correction
/// (research-leans-12 §3): in a 3 M☉ star's thermal pulse of factor F it moves by −2.5 log₁₀ F to
/// 10⁻¹² mag, the composed L and R keep the track's `T_eff`, and so does the summary's state; a
/// flare brightens the dwarf's by its own light. The summary's own `M_V` is the track's to the bit
/// with an event in progress or none, and with none the summary carries no event light; and the
/// system's brief is the primary's track values to the bit, and the range brief the held arm's.
#[test]
fn the_composed_m_v_follows_the_event_light_and_the_brief_keeps_the_track_s() {
    use crate::events::{EventSeries, tags};
    use crate::id::BodyId;
    use crate::math;
    use crate::stellar::events::thermal_pulse::ThermalPulseModel;
    use hyperion_testkit::float::bits as float_bits;
    let bits = |m: Option<crate::units::Magnitudes>| m.map(|m| float_bits(m.value()));
    let check_brief = |stars: &SystemStars, summary: &StarSummary, t: UniverseTime| {
        let brief = stars.brief_at(t).expect("formed");
        // The range brief is the held galaxy's, as the system's is.
        let record = stars.record();
        assert_eq!(
            BriefModel::new(composed(), record).brief_at(t),
            BriefModel::new(held(), record).brief_at(t)
        );
        let luminosity = summary.state().luminosity().value();
        assert_eq!(
            brief.log_luminosity().map(|l| float_bits(l.value())),
            Some(float_bits(math::log10(luminosity)))
        );
        assert_eq!(
            float_bits(brief.effective_temperature().value()),
            float_bits(summary.state().effective_temperature().value())
        );
    };
    // The thermal pulse: the record's age steps back from the star's death, at the epoch, inside
    // the clock window, to a time in a pulse's transient.
    let death = SystemStars::generate(composed(), &record(5, 3.0, 1e8))
        .primary()
        .lifetime()
        .expect("a 3 M☉ star dies")
        .value();
    let t = UniverseTime::EPOCH;
    let (pulsing, record) = (1..200)
        .map(|k| record(5, 3.0, death - 7_000.0 * f64::from(k)))
        .map(|record| (SystemStars::generate(composed(), &record), record))
        .find(|(stars, _)| {
            let summary = primary(stars, t);
            summary.event_effects().is_some_and(|effects| {
                float_bits(effects.radius().value()) != float_bits(summary.state().radius().value())
            })
        })
        .expect("a pulse's transient in the last 1.4 Myr");
    let summary = primary(&pulsing, t);
    let effects = summary.event_effects().expect("a pulse's light");
    let series = EventSeries::new(
        Seed::new(SEED),
        tags::STAR_THERMAL_PULSE,
        BodyId::new(record.id(), 0).into(),
    );
    let factor = ThermalPulseModel::of(pulsing.primary(), &series)
        .expect("the star pulses")
        .luminosity_factor_at(t);
    let track = summary.absolute_magnitude_v().expect("a giant's M_V");
    let composed_v = effects.absolute_magnitude_v().expect("the pulse's M_V");
    let shift = composed_v.value() - track.value();
    assert!(
        (shift + 2.5 * math::log10(factor)).abs() < 1e-12,
        "F {factor}: M_V moves by {shift}"
    );
    let (state, held_summary) = (
        summary.state(),
        primary(&SystemStars::generate(held(), &record), t),
    );
    let teff = |l: f64, r: f64| math::powf_positive(l / (r * r), 0.25);
    let ratio = teff(effects.luminosity().value(), effects.radius().value())
        / teff(state.luminosity().value(), state.radius().value());
    assert!((ratio - 1.0).abs() < 1e-12, "T_eff moves by {ratio}");
    assert_eq!(summary.state(), held_summary.state());
    assert_eq!(bits(Some(track)), bits(held_summary.absolute_magnitude_v()));
    check_brief(&pulsing, &summary, t);
    // The flare and a quiet hour of the flaring dwarf.
    let (dwarf, t) = flaring();
    let stars = SystemStars::generate(composed(), dwarf);
    let held_stars = SystemStars::generate(held(), dwarf);
    let summary = primary(&stars, *t);
    let effects = summary.event_effects().expect("a flare's light");
    let track = summary.absolute_magnitude_v().expect("a dwarf's M_V");
    let ratio = effects.luminosity().value() / summary.state().luminosity().value();
    let flared = effects.absolute_magnitude_v().expect("the flare's M_V");
    assert!(flared < track, "{flared:?} against {track:?}");
    assert!((flared.value() - track.value() + 2.5 * math::log10(ratio)).abs() < 1e-12);
    assert_eq!(
        bits(summary.absolute_magnitude_v()),
        bits(primary(&held_stars, *t).absolute_magnitude_v())
    );
    check_brief(&stars, &summary, *t);
    let quiet = (1..24 * 30)
        .map(|h| {
            UniverseTime::EPOCH
                .checked_add(crate::time::Span::from_seconds(3_600 * h))
                .expect("inside the clock")
        })
        .find(|&q| primary(&stars, q).event_effects().is_none())
        .expect("a quiet hour in a month");
    let summary = primary(&stars, quiet);
    assert_eq!(
        bits(summary.absolute_magnitude_v()),
        bits(primary(&held_stars, quiet).absolute_magnitude_v())
    );
    check_brief(&stars, &summary, quiet);
}
