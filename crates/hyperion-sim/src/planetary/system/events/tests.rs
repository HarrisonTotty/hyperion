//! Tests of a system's events on the record (P14.T31.c): the merge's order, its partition
//! and order independence, the IDs, each body's existence, the storms' clock and water deck, no
//! mark on any record, and the golden systems' century of events; and the census
//! decision-galaxy-25-train §5.2 asks for, run by name.

use std::fmt::Write as _;

use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, f64_digest};
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::events::testing::assert_partition_independent;
use crate::orbit::Eccentricity;
use crate::planetary::events::STORM_PERIOD_FLOOR;
use crate::planetary::events::storms::MIN_DURATION_SHARE;
use crate::planetary::events::{dust_storms as dust, storms as storm_kind};
use crate::planetary::fate::DestructionCause;
use crate::planetary::generate;
use crate::planetary::record::{BodyKind, SectionState};
use crate::planetary::system::tests::{
    GOLDEN_SYSTEMS_SEED, SEED, golden_contexts, golden_systems, sample,
};
use crate::time::ClockWindow;
use crate::units::Seconds;
use crate::{GENERATOR_VERSION, Seed};

/// The instant `years` Julian years from the epoch.
fn year(years: i64) -> UniverseTime {
    UniverseTime::from_julian_years(years).unwrap()
}

/// The century about the epoch, [−50, 50) Julian years: the window the golden pins.
fn century() -> TimeWindow {
    TimeWindow::new(year(-50), year(50)).unwrap()
}

/// One golden system at 25: its name, context, system and event models.
type At25 = (&'static str, SystemContext, PlanetarySystem, SystemEvents);

/// The golden systems generated afresh, each with its event models.
fn golden_at_25() -> Vec<At25> {
    golden_contexts()
        .into_iter()
        .map(|(name, ctx)| {
            let system = generate(Seed::new(GOLDEN_SYSTEMS_SEED), &ctx);
            let models = system.event_models(&ctx);
            (name, ctx, system, models)
        })
        .collect()
}

/// The golden system `name` of `systems`.
fn named<'a>(systems: &'a [At25], name: &str) -> &'a At25 {
    systems.iter().find(|(at, ..)| *at == name).unwrap()
}

/// The models of body `index` among `models`.
fn models_of(models: &SystemEvents, index: u16) -> &BodyModels {
    models
        .bodies
        .iter()
        .find(|body| body.index.get() == index)
        .unwrap()
}

/// Test (c), the list: a system's events are its models' over the window, sorted strictly by time
/// and then by ID, the same for the same seed generated twice; a reversed or empty window has none.
#[test]
fn a_system_s_events_are_its_models_in_time_then_id_order() {
    let systems = golden_at_25();
    let again = golden_at_25();
    let mut kinds = [0_usize; 5];
    for ((name, ctx, system, models), (.., twin)) in systems.iter().zip(&again) {
        let events = models.between(century());
        assert!(
            events
                .windows(2)
                .all(|pair| (pair[0].time(), pair[0].id()) < (pair[1].time(), pair[1].id())),
            "{name}: not in strict time-then-ID order"
        );
        assert!(events.iter().all(|event| century().contains(event.time())));
        assert_eq!(models, twin, "{name}: the same seed twice");
        assert_eq!(
            system.events_between(ctx, year(1), year(-1)),
            Vec::<BodyEvent>::new()
        );
        assert_eq!(
            system.events_between(ctx, year(1), year(1)),
            Vec::<BodyEvent>::new()
        );
        let asked = system.events_between(ctx, century().start(), century().end());
        assert_eq!(asked, events, "{name}");
        for event in &events {
            let kind = match event {
                BodyEvent::Impact { .. } => 0,
                BodyEvent::Eruption { .. } => 1,
                BodyEvent::Storm { .. } => 2,
                BodyEvent::DustStorm { .. } => 3,
                BodyEvent::Comet { .. } => 4,
            };
            kinds[kind] += 1;
        }
    }
    eprintln!("impacts, eruptions, storms, dust storms, comets: {kinds:?}");
    assert!(
        kinds.iter().all(|&n| n > 0),
        "every kind is listed: {kinds:?}"
    );
}

/// Test (c), the partition: every golden system's events of a century come back the same asked
/// whole, in halves and cut at uneven points, a nanosecond from a whole year among them (plan 06's
/// `assert_partition_independent`).
#[test]
fn the_events_come_back_whole_and_in_halves() {
    let nanosecond = Span::new(0, 1).unwrap();
    let uneven = [
        year(-37).checked_add(nanosecond).unwrap(),
        year(-37)
            .checked_add(nanosecond.checked_mul(2).unwrap())
            .unwrap(),
        UniverseTime::EPOCH,
        year(12)
            .checked_add(Span::from_seconds(86_400 * 100))
            .unwrap(),
        year(49),
    ];
    for (name, .., models) in &golden_at_25() {
        let list = |window: TimeWindow| models.between(window);
        assert_partition_independent(century(), &[UniverseTime::EPOCH], list);
        assert_partition_independent(century(), &uneven, list);
        eprintln!("{name}: {} events", models.between(century()).len());
    }
}

/// Test (c), the order of asking: a system's decades in any order give the same events, and so do
/// the systems themselves, each generated and asked afresh (plan 01's `assert_order_independent`).
#[test]
fn the_events_are_the_same_in_any_order_of_asking() {
    let systems = golden_at_25();
    let decades: Vec<TimeWindow> = (-5..5)
        .map(|k| TimeWindow::new(year(10 * k), year(10 * k + 10)).unwrap())
        .collect();
    for name in ["solar_like", "eccentric_giant", "close_binary"] {
        let (.., models) = named(&systems, name);
        assert_order_independent(&decades, |window| models.between(*window));
    }
    let contexts: Vec<(&str, SystemContext)> = golden_contexts()
        .into_iter()
        .filter(|(name, _)| ["solar_like", "hot_jupiter", "eccentric_giant"].contains(name))
        .collect();
    assert_order_independent(&contexts, |(_, ctx)| {
        generate(Seed::new(GOLDEN_SYSTEMS_SEED), ctx).events_between(ctx, year(-10), year(10))
    });
}

/// Test (c), the IDs: every event's ID round-trips through plan 01's text form and regenerates the
/// event alone; an ID of another system, of a kind the body has no model for or of a tag of no
/// kind gives none.
#[test]
fn event_ids_round_trip_through_their_text_and_regenerate() {
    let systems = golden_at_25();
    let mut checked = 0_usize;
    for (name, .., models) in &systems {
        for event in models.between(century()) {
            let id = event.id();
            let text = id.to_string();
            assert_eq!(text.parse::<EventId>(), Ok(id), "{name}: {text}");
            assert_eq!(models.event(id), Some(event), "{name}: {text}");
            checked += 1;
        }
    }
    assert!(checked > 100_000, "{checked}");
    let (.., solar) = named(&systems, "solar_like");
    let (.., hot) = named(&systems, "hot_jupiter");
    let storm = solar
        .between(century())
        .into_iter()
        .find(|event| matches!(event, BodyEvent::Storm { .. }))
        .unwrap();
    let comet = solar
        .between(century())
        .into_iter()
        .find(|event| matches!(event, BodyEvent::Comet { .. }))
        .unwrap();
    assert_eq!(hot.event(storm.id()), None, "another system's storm");
    assert_eq!(hot.event(comet.id()), None, "another system's comet");
    // The rocky innermost planet has no storms: a storm word on its subject names nothing.
    let rocky = solar.bodies[0].index.body_id(solar.system());
    let foreign = EventId::new(EventSubject::Body(rocky), storm.id().word());
    assert_eq!(solar.event(foreign), None);
    // A comet's word on the system's subject is the comet; an impact's word there is nothing.
    let impact = solar
        .between(century())
        .into_iter()
        .find(|event| matches!(event, BodyEvent::Impact { .. }))
        .unwrap();
    let on_system = EventId::new(EventSubject::System(solar.system()), impact.id().word());
    assert_eq!(solar.event(on_system), None);
    assert_eq!(solar.event(comet.id()), Some(comet));
}

/// No event leaves a mark (design note 15): every body's record at its events' times is the same
/// whether asked before the events are listed or after.
#[test]
fn no_event_changes_any_body_at_result() {
    let systems = golden_at_25();
    for name in ["solar_like", "eccentric_giant"] {
        let (_, ctx, system, models) = named(&systems, name);
        // The first event of each kind on each body, and the halo's first comet.
        let mut times: Vec<(BodyIndex, UniverseTime)> = Vec::new();
        let events = models.between(century());
        for event in &events {
            let kind = core::mem::discriminant(event);
            if !events
                .iter()
                .take_while(|other| *other != event)
                .any(|other| other.body() == event.body() && core::mem::discriminant(other) == kind)
            {
                times.push((event.body(), event.time()));
            }
        }
        assert!(times.len() >= 3, "{name}: {times:?}");
        let records = |system: &PlanetarySystem| -> Vec<BodyRecord> {
            times
                .iter()
                .map(|&(index, t)| system.body_at(ctx, index, t).unwrap())
                .collect()
        };
        let before = records(system);
        let listed = system.events_between(ctx, year(-50), year(50));
        assert_eq!(listed, events);
        let after = records(system);
        assert_eq!(before, after, "{name}");
    }
}

/// Each event falls while its body is present: its position at the event's time is stated, as a
/// present body's is, for the first events of each kind on every body (`position_at` reads the
/// fate transform apart from the models' lifetimes).
#[test]
fn each_event_falls_while_its_body_is_present() {
    let systems = golden_at_25();
    let mut checked = 0;
    for (name, ctx, system, models) in &systems {
        let mut seen: Vec<(BodyIndex, core::mem::Discriminant<BodyEvent>, usize)> = Vec::new();
        for event in models.between(century()) {
            let key = (event.body(), core::mem::discriminant(&event));
            let count = if let Some((.., n)) = seen.iter_mut().find(|(b, k, _)| (*b, *k) == key) {
                *n += 1;
                *n
            } else {
                seen.push((key.0, key.1, 1));
                1
            };
            if count > 3 || matches!(event, BodyEvent::Comet { .. }) {
                continue;
            }
            let position = system.position_at(ctx, event.body(), event.time()).unwrap();
            assert!(position.is_some(), "{name}: {event:?}");
            checked += 1;
        }
    }
    assert!(checked > 300, "{checked}");
}

/// A body's events stop at its ending: a giant's impacts listed with a lifetime that ends at the
/// epoch are exactly those before it, and with one formed at the epoch exactly those after.
#[test]
fn a_body_s_events_are_cut_at_its_formation_and_its_ending() {
    let systems = golden_at_25();
    let (.., models) = named(&systems, "solar_like");
    let giant = models_of(models, 0x0500);
    let mut all = Vec::new();
    giant.list(century(), &mut Found::default(), &mut all);
    assert!(all.len() > 1_000, "{}", all.len());
    let ended = BodyModels {
        presence: Presence::Primary(Lifetime {
            formed_at: Some(ClockWindow::START),
            ending: Some(BodyState::Destroyed {
                cause: DestructionCause::Engulfed,
                at: UniverseTime::EPOCH,
            }),
        }),
        ..*giant
    };
    let born = BodyModels {
        presence: Presence::Primary(Lifetime {
            formed_at: Some(UniverseTime::EPOCH),
            ending: None,
        }),
        ..*giant
    };
    let never = BodyModels {
        presence: Presence::Primary(Lifetime {
            formed_at: None,
            ending: None,
        }),
        ..*giant
    };
    let (mut before, mut after, mut none) = (Vec::new(), Vec::new(), Vec::new());
    let mut found = Found::default();
    ended.list(century(), &mut found, &mut before);
    born.list(century(), &mut found, &mut after);
    never.list(century(), &mut found, &mut none);
    // A body's list runs kind by kind, each in time order.
    let (early, late): (Vec<BodyEvent>, Vec<BodyEvent>) = all
        .into_iter()
        .partition(|event| event.time() < UniverseTime::EPOCH);
    assert!(!early.is_empty() && !late.is_empty());
    assert_eq!(before, early);
    assert_eq!(after, late);
    assert_eq!(none, Vec::<BodyEvent>::new());
    let late = late[0].clone();
    assert_eq!(ended.event(late.id()), None);
    assert_eq!(born.event(late.id()), Some(late));
}

/// A halo's comets end where it is unbound: listed with an unbinding at the epoch, the solar-like
/// halo's comets of the century are exactly those before it, and a later one's ID names none.
#[test]
fn a_halo_s_comets_end_at_its_unbinding() {
    let systems = golden_at_25();
    let (.., models) = named(&systems, "solar_like");
    let comets = models.comets.unwrap();
    assert_eq!(
        comets.until, None,
        "the solar-like halo outlives the window"
    );
    let cut = SystemEvents {
        comets: Some(HaloComets {
            until: Some(UniverseTime::EPOCH),
            ..comets
        }),
        ..models.clone()
    };
    let of_comets = |events: Vec<BodyEvent>| -> Vec<BodyEvent> {
        events
            .into_iter()
            .filter(|event| matches!(event, BodyEvent::Comet { .. }))
            .collect()
    };
    let all = of_comets(models.between(century()));
    let kept = of_comets(cut.between(century()));
    let early: Vec<BodyEvent> = all
        .iter()
        .filter(|event| event.time() < UniverseTime::EPOCH)
        .cloned()
        .collect();
    assert!(!early.is_empty() && early.len() < all.len());
    assert_eq!(kept, early);
    let late = all
        .iter()
        .find(|event| event.time() >= UniverseTime::EPOCH)
        .unwrap();
    assert_eq!(cut.event(late.id()), None);
    assert_eq!(models.event(late.id()).as_ref(), Some(late));
}

/// A moon's presence is its record's: its planet's lifetime with its own loss agrees with
/// `position_at`, which reads the fate transform and the moon's state apart from the models, at
/// the window's ends, the epoch, and a nanosecond either side of each moon's loss and its planet's
/// formation and ending inside the clock's reach.
#[test]
fn a_moon_s_presence_is_its_record_s() {
    let nanosecond = Span::new(0, 1).unwrap();
    let systems: Vec<(&SystemContext, &PlanetarySystem)> = golden_systems()
        .iter()
        .map(|(_, ctx, system)| (ctx, system))
        .chain(
            crate::planetary::system::tests::whole_fast()[..40]
                .iter()
                .map(|(ctx, system)| (ctx, system)),
        )
        .collect();
    let (mut moons, mut losses, mut checked) = (0, 0, 0);
    for (ctx, system) in systems {
        for (body, presence) in system.presences(ctx) {
            let Presence::Moon {
                planet,
                satellite,
                age_at_epoch,
            } = presence
            else {
                continue;
            };
            moons += 1;
            let lost = super::super::moon_lost_at(&satellite, age_at_epoch)
                .filter(|&at| at <= ClockWindow::END);
            losses += usize::from(lost.is_some());
            let mut times = vec![ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END];
            for at in [
                lost,
                planet.formed_at,
                planet.ending.and_then(|ending| ending.ended_at()),
            ]
            .into_iter()
            .flatten()
            .filter(|&at| at <= ClockWindow::END)
            {
                times.extend(at.checked_sub(nanosecond));
                times.push(at);
                times.extend(
                    at.checked_add(nanosecond)
                        .filter(|&t| t <= ClockWindow::END),
                );
            }
            for t in times {
                let position = system.position_at(ctx, body.index, t).unwrap();
                assert_eq!(
                    presence.holds_at(t),
                    position.is_some(),
                    "{:?} at {t}",
                    body.index
                );
                checked += 1;
            }
        }
    }
    eprintln!("{moons} moons, {losses} lost by the window's end, {checked} checked");
    assert!(moons > 50 && losses > 0, "{moons} moons, {losses} losses");
}

/// P14.T36.b's bound: a window is listed whole where it holds no more than asked, and otherwise
/// the widest span about its middle that holds no more, every event of it, as far either side,
/// one grain wider holding more; the busiest golden system's century, at four bounds.
#[test]
fn a_listing_holds_no_more_than_asked_about_the_window_s_middle() {
    let systems = golden_at_25();
    let (.., models) = named(&systems, "close_binary");
    let all = models.between(century());
    assert!(all.len() > 100_000, "{}", all.len());
    let whole = models.listed_about_middle(century(), all.len());
    assert_eq!((whole.span(), whole.events()), (century(), all.as_slice()));
    let grain = Span::new(0, 0)
        .unwrap()
        .checked_add(
            Span::from_seconds_f64(50.0 * 31_557_600.0 / f64::from(LISTING_GRAINS_U32)).unwrap(),
        )
        .unwrap();
    for most in [0, 1, 1_000, 25_000] {
        let listed = models.listed_about_middle(century(), most);
        let span = listed.span();
        assert!(listed.events().len() <= most, "{most}");
        assert_eq!(listed.events(), models.between(span), "{most}");
        assert!(century().start() <= span.start() && span.end() <= century().end());
        let (before, after) = (
            UniverseTime::EPOCH.checked_since(span.start()).unwrap(),
            span.end().checked_since(UniverseTime::EPOCH).unwrap(),
        );
        assert_eq!(before, after, "{most}: as far either side of the middle");
        // A grain wider, to the rounding of the grain here, holds more.
        let wider = TimeWindow::new(
            span.start().checked_sub(grain).unwrap(),
            span.end().checked_add(grain).unwrap(),
        )
        .unwrap();
        assert!(
            models.between(wider).len() > most,
            "{most}: {span:?} is the widest"
        );
    }
}

/// A listing of the clock's whole window overflows nothing: with no events, it is the window.
#[test]
fn a_listing_of_the_clock_s_whole_window_is_the_window() {
    let clock = TimeWindow::new(
        UniverseTime::new(i64::MIN, 0).unwrap(),
        UniverseTime::new(i64::MAX, 999_999_999).unwrap(),
    )
    .unwrap();
    let none = SystemEvents::none(SystemId::from_raw(0x0200_0800_2000_0000).unwrap());
    let listed = none.listed_about_middle(clock, 0);
    assert_eq!((listed.span(), listed.events()), (clock, &[][..]));
}

/// The models' charge is one entry a body with models, each holding nothing of its own on the heap.
#[test]
fn the_models_charge_one_entry_a_body() {
    let systems = golden_at_25();
    let (.., models) = named(&systems, "solar_like");
    assert_eq!(
        models.heap_bytes(),
        models.bodies.capacity() * size_of::<BodyModels>()
    );
    assert!(models.heap_bytes() >= 22 * size_of::<BodyModels>());
    assert_eq!(SystemEvents::none(models.system()).heap_bytes(), 0);
}

/// [`Lifetime::state_at`] is [`BodyFate::at`]'s state for every planet and member of the golden
/// systems and the sample, at the window's ends, the epoch, and a nanosecond either side of each
/// formation and ending inside the clock's reach, endings among them.
#[test]
fn a_lifetime_gives_the_fate_s_state() {
    let nanosecond = Span::new(0, 1).unwrap();
    let systems: Vec<(&SystemContext, &PlanetarySystem)> = golden_systems()
        .iter()
        .map(|(_, ctx, system)| (ctx, system))
        .chain(
            crate::planetary::system::tests::whole_fast()[..40]
                .iter()
                .map(|(ctx, system)| (ctx, system)),
        )
        .collect();
    let (mut checked, mut endings) = (0, 0);
    for (ctx, system) in systems {
        let hosts: Vec<_> = system
            .zones
            .iter()
            .map(|zone| fate_host(ctx, zone))
            .collect();
        for fate in system.fates(&hosts).iter().flatten() {
            let lifetime = Lifetime::of(fate);
            let mut times = vec![ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END];
            for at in [
                fate.formed_at(),
                fate.ending().and_then(|ending| ending.ended_at()),
            ]
            .into_iter()
            .flatten()
            .filter(|&at| at <= ClockWindow::END)
            {
                times.extend(at.checked_sub(nanosecond));
                times.push(at);
                times.extend(
                    at.checked_add(nanosecond)
                        .filter(|&t| t <= ClockWindow::END),
                );
            }
            endings += usize::from(fate.ending().is_some());
            for t in times {
                assert_eq!(lifetime.state_at(t), fate.at(t).state(), "{t}");
                checked += 1;
            }
        }
    }
    assert!(
        endings > 0 && checked > 1_000,
        "{endings} endings, {checked} checked"
    );
}

/// Decision-galaxy-25-train §5.2, points 1 and 2: a giant has great storms exactly where its
/// envelope lists a water deck, each on its orbital period floored at 10 Julian years from a
/// perihelion passage; no other body has any. The cold giants keep them, the hot Jupiter and the
/// warm ice giant lose them.
#[test]
fn great_storms_take_a_water_deck_and_a_ten_year_clock() {
    let systems = golden_at_25();
    let (mut kept, mut removed) = (Vec::new(), Vec::new());
    for (name, ctx, system, models) in &systems {
        let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
        for body in &models.bodies {
            let record = snapshot.body(body.index).unwrap();
            let bulk = record.bulk().ok().unwrap();
            let giant = has_great_storms(bulk.class());
            let water = has_water_deck(record.envelope());
            let label = format!("{name} {:04x}", body.index.get());
            assert_eq!(body.storms.is_some(), giant && water, "{label}");
            if giant {
                if water { &mut kept } else { &mut removed }.push(label.clone());
            }
            let Some(model) = &body.storms else {
                continue;
            };
            let elements = record.orbit().ok().unwrap().elements();
            let (period, perihelion) = clock_of(elements).unwrap();
            assert_eq!(model.clock().period(), storm_period(period), "{label}");
            assert!(model.clock().period() >= STORM_PERIOD_FLOOR, "{label}");
            assert_eq!(model.inputs().origin, perihelion, "{label}");
            // Every storm lasts at least the shortest share of 10 years, 8.8 days.
            let mut listed = Vec::new();
            storm_kind::storms(model, century(), &mut listed);
            assert!(!listed.is_empty(), "{label}");
            for storm in listed {
                assert!(
                    storm.duration() >= Span::from_seconds(8 * 86_400),
                    "{label}"
                );
            }
        }
    }
    eprintln!("kept: {kept:?}\nremoved: {removed:?}");
    assert!(kept.contains(&"solar_like 0500".to_owned()), "{kept:?}");
    assert!(
        kept.contains(&"eccentric_giant 0100".to_owned()),
        "{kept:?}"
    );
    assert!(
        removed.contains(&"hot_jupiter 0100".to_owned()),
        "{removed:?}"
    );
    assert!(
        removed.contains(&"wide_binary 0100".to_owned()),
        "{removed:?}"
    );
    let floor_share = MIN_DURATION_SHARE * STORM_PERIOD_FLOOR.as_seconds_f64() / 86_400.0;
    assert!((floor_share - 8.766).abs() < 1e-3, "{floor_share}");
}

/// The water deck is read off an `Ok` envelope alone: a cold giant's lists one, a hot Jupiter's
/// none, and every other state of the section gives none.
#[test]
fn a_water_deck_is_read_off_an_ok_envelope_alone() {
    let systems = golden_at_25();
    let envelope_of = |name: &str, index: u16| {
        let (_, ctx, system, _) = named(&systems, name);
        let index = BodyIndex::try_from(index).unwrap();
        system
            .body_at(ctx, index, UniverseTime::EPOCH)
            .unwrap()
            .envelope()
            .clone()
    };
    let cold = envelope_of("solar_like", 0x0500);
    let hot = envelope_of("hot_jupiter", 0x0100);
    assert_eq!(cold.state(), SectionState::Ok);
    assert_eq!(hot.state(), SectionState::Ok);
    assert!(has_water_deck(&cold));
    assert!(!has_water_deck(&hot));
    for state in [
        Section::NotModelled,
        Section::NotApplicable,
        Section::NotResolved,
    ] {
        assert!(!has_water_deck(&state));
    }
}

/// The storm clock's floor is 10 Julian years, and the clock its period or the floor, the larger.
#[test]
fn the_storm_period_is_its_orbit_s_floored_at_ten_julian_years() {
    assert_eq!(Some(STORM_PERIOD_FLOOR), Span::from_julian_years(10));
    let day = |days: i64| Span::from_seconds(days * 86_400);
    assert_eq!(storm_period(day(3)), STORM_PERIOD_FLOOR);
    assert_eq!(storm_period(STORM_PERIOD_FLOOR), STORM_PERIOD_FLOOR);
    assert_eq!(storm_period(day(10_756)), day(10_756));
    assert_eq!(storm_period(day(4_333)), day(4_333));
}

/// An orbit's clock starts at a perihelion passage at or before the epoch, less than a period
/// before it, where the elements' mean anomaly is 0 to rounding.
#[test]
fn an_orbit_s_clock_starts_at_its_last_perihelion_passage() {
    let mut checked = 0;
    for (_, ctx, system) in golden_systems() {
        let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
        for record in snapshot.bodies() {
            let Some(orbit) = record.orbit().ok() else {
                continue;
            };
            let elements = orbit.elements();
            let (period, perihelion) = clock_of(elements).unwrap();
            assert!(perihelion <= UniverseTime::EPOCH);
            assert!(UniverseTime::EPOCH.checked_sub(period).unwrap() < perihelion);
            let m = elements.mean_anomaly_at(perihelion).value();
            assert!(m.min(TAU - m) < 1e-6, "{m}");
            checked += 1;
        }
    }
    assert!(checked > 100, "{checked}");
}

/// A body has dust storms exactly where its air holds dust on a year of at least 301 days, each on
/// its orbital period from a perihelion passage with the orbit's eccentricity, and every kind's
/// model is the record's: no eruptions without a surface, and impacts on every present planet,
/// member and moon with figures, and every giant.
#[test]
fn each_kind_reads_its_record() {
    let systems = golden_at_25();
    let (mut dusty, mut moons) = (0, 0);
    for (name, ctx, system, models) in &systems {
        let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
        for body in &models.bodies {
            let record = snapshot.body(body.index).unwrap();
            let label = format!("{name} {:04x}", body.index.get());
            let tau = dust_optical_depth_of(record);
            assert_eq!(
                body.eruptions.is_some(),
                record.surface().ok().is_some(),
                "{label}"
            );
            let giant = record
                .bulk()
                .ok()
                .is_some_and(|bulk| bulk.class().is_giant());
            if record.surface().ok().is_some() || giant {
                assert!(body.impacts.is_some(), "{label}");
            }
            if !has_dust_storms(tau) {
                assert!(body.dust_storms.is_none(), "{label}");
                continue;
            }
            // A moon's seasons, and its year's gate, are its planet's.
            let moon = matches!(record.identity().kind(), BodyKind::Moon(_));
            let primary = match record.identity().kind() {
                BodyKind::Moon(_) => {
                    let Some(OrbitHost::Body(planet)) = record.identity().parent() else {
                        panic!("{label}: a moon's parent is a body");
                    };
                    snapshot.body(planet).unwrap()
                }
                BodyKind::Planet
                | BodyKind::DwarfPlanet
                | BodyKind::Ring
                | BodyKind::Belt(_)
                | BodyKind::CometaryHalo
                | BodyKind::ProtoplanetaryDisc
                | BodyKind::DebrisDisc
                | BodyKind::Unresolved => record,
            };
            let elements = primary.orbit().ok().unwrap().elements();
            let (period, perihelion) = clock_of(elements).unwrap();
            assert_eq!(
                body.dust_storms.is_some(),
                year_holds_dust_storms(period),
                "{label}"
            );
            let Some(model) = &body.dust_storms else {
                continue;
            };
            dusty += 1;
            moons += usize::from(moon);
            assert_eq!(model.clock().period(), period, "{label}");
            assert_eq!(model.inputs().perihelion, perihelion, "{label}");
            assert_eq!(
                model.inputs().eccentricity,
                elements.eccentricity(),
                "{label}"
            );
        }
        // Every present body but a ring has a body's models; nothing is listed for a ring, a belt.
        for record in snapshot.bodies() {
            let listed = models
                .bodies
                .iter()
                .any(|body| body.index == record.index());
            let present = record.identity().state() == BodyState::Present;
            let has_bulk = record.bulk().ok().is_some();
            assert_eq!(
                listed,
                present && has_bulk,
                "{name} {:04x}",
                record.index().get()
            );
        }
    }
    assert!(
        dusty > moons && moons > 0,
        "{dusty} dusty bodies, {moons} moons among them"
    );
}

/// A dusty body has no dust storms on a year under 301 days, the census's short years among them,
/// and takes the least year itself: `filler_c`'s 0400 on its own orbit with its period changed, the
/// gate read before the model is built, so that no refusal reaches `kept`.
#[test]
fn a_dusty_body_on_a_year_under_301_days_has_no_dust_storms() {
    let systems = golden_at_25();
    let (_, ctx, system, models) = named(&systems, "filler_c");
    assert!(models_of(models, 0x0400).dust_storms.is_some());
    let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
    let body = system
        .bodies
        .iter()
        .find(|body| body.index.get() == 0x0400)
        .unwrap();
    let record = snapshot.body(body.index).unwrap();
    assert!(has_dust_storms(dust_optical_depth_of(record)));
    let own = *record.orbit().ok().unwrap().elements();
    let on = |days: f64| {
        KeplerElements::from_period(
            Seconds::new(days * 86_400.0),
            own.gravitational_parameter(),
            own.eccentricity(),
            *own.orientation(),
            own.mean_anomaly_at_epoch(),
        )
        .unwrap()
    };
    for days in [10.6, 26.1, 79.4, 143.0, 300.0] {
        assert_eq!(
            system.dust_storm_model(body, record, Some(&on(days))),
            None,
            "{days} d"
        );
    }
    let least = system
        .dust_storm_model(body, record, Some(&on(301.0)))
        .unwrap();
    assert_eq!(least.clock().period(), dust::LEAST_PERIOD);
    assert_eq!(least.inputs().eccentricity, own.eccentricity());
}

/// The golden systems' events of the century about the epoch (P14.T31.c; decision-galaxy-25-train
/// §5.2's "goldens over generated bodies"): for each system its count by kind and a digest of every
/// event; for each body and kind its count, its digest and its first event; every storm and dust
/// storm; and the first three comets.
#[test]
fn the_golden_systems_events_of_a_century_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line(
        "# P14.T31.c: each golden system's events in [-50, 50) Julian years about the epoch, under \
         every 25 hold",
    );
    for (name, ctx, _, models) in &golden_at_25() {
        let events = models.between(century());
        let mut counts = [0_usize; 5];
        for event in &events {
            counts[kind_of(event)] += 1;
        }
        w.line("");
        w.line(&format!(
            "{name} {}: {} events: {} impacts, {} eruptions, {} storms, {} dust storms, {} comets",
            ctx.id(),
            events.len(),
            counts[0],
            counts[1],
            counts[2],
            counts[3],
            counts[4]
        ));
        w.u64_hex("  digest", digest(events.iter()));
        let mut bodies: Vec<BodyIndex> = events.iter().map(BodyEvent::body).collect();
        bodies.sort_unstable();
        bodies.dedup();
        for body in bodies {
            for (kind, kind_name) in KINDS.iter().enumerate() {
                let of: Vec<&BodyEvent> = events
                    .iter()
                    .filter(|event| event.body() == body && kind_of(event) == kind)
                    .collect();
                let Some(first) = of.first() else {
                    continue;
                };
                let all = matches!(first, BodyEvent::Storm { .. } | BodyEvent::DustStorm { .. });
                let shown = match first {
                    BodyEvent::Storm { .. } | BodyEvent::DustStorm { .. } => of.len(),
                    BodyEvent::Comet { .. } => 3,
                    BodyEvent::Impact { .. } | BodyEvent::Eruption { .. } => 1,
                };
                w.line(&format!(
                    "  {:04x} {}: {}{}",
                    body.get(),
                    kind_name,
                    of.len(),
                    if all {
                        String::new()
                    } else {
                        format!(", digest 0x{:016x}", digest(of.iter().copied()))
                    }
                ));
                for event in of.iter().take(shown) {
                    pin_event(&mut w, event);
                }
            }
        }
    }
    golden!("planetary/system_events", w.as_str());
}

/// The kinds' names, in [`kind_of`]'s order.
const KINDS: [&str; 5] = ["impacts", "eruptions", "storms", "dust storms", "comets"];

/// The index of `event`'s kind in [`KINDS`].
fn kind_of(event: &BodyEvent) -> usize {
    match event {
        BodyEvent::Impact { .. } => 0,
        BodyEvent::Eruption { .. } => 1,
        BodyEvent::Storm { .. } => 2,
        BodyEvent::DustStorm { .. } => 3,
        BodyEvent::Comet { .. } => 4,
    }
}

/// `event`'s values: a line of its ID, its time and its kind's whole values (spans, cycle, band),
/// and its floats with their labels, in order, which the golden pins by their bits.
fn values_of(event: &BodyEvent) -> (String, Vec<(&'static str, f64)>) {
    let mut line = format!("{} at {}", event.id(), event.time());
    let floats = match event {
        BodyEvent::Impact { impact, .. } => vec![
            ("diameter (m)", impact.diameter().value()),
            ("energy (J)", impact.energy_joules()),
            ("velocity (m/s)", impact.velocity().value()),
            ("latitude (rad)", impact.site().latitude().value()),
            ("longitude (rad)", impact.site().longitude().value()),
        ],
        BodyEvent::Eruption { eruption, .. } => {
            write!(line, " for {}", eruption.duration()).unwrap();
            Vec::new()
        }
        BodyEvent::Storm { storm, .. } => {
            write!(
                line,
                " cycle {} for {}, {:?}",
                storm.cycle(),
                storm.duration(),
                storm.band()
            )
            .unwrap();
            Vec::new()
        }
        BodyEvent::DustStorm { dust_storm, .. } => {
            write!(
                line,
                " cycle {} for {}",
                dust_storm.cycle(),
                dust_storm.duration()
            )
            .unwrap();
            vec![(
                "true anomaly at onset (rad)",
                dust_storm.onset_true_anomaly().value(),
            )]
        }
        BodyEvent::Comet { comet, .. } => {
            write!(
                line,
                " listed {} to {}{}",
                comet.listed().start(),
                comet.listed().end(),
                if comet.plunges() { ", plunges" } else { "" }
            )
            .unwrap();
            vec![
                ("perihelion (m)", comet.perihelion().value()),
                ("eccentricity", comet.eccentricity()),
                ("nucleus (m)", comet.nucleus_diameter().value()),
            ]
        }
    };
    (line, floats)
}

/// Pins `event`: its line ([`values_of`]), then each of its floats by its bits.
fn pin_event(w: &mut GoldenWriter, event: &BodyEvent) {
    let (line, floats) = values_of(event);
    w.line(&format!("    {line}"));
    for (label, value) in floats {
        w.f64(&format!("      {label}"), value);
    }
}

/// FNV-1a 64 over each of `events`' line ([`values_of`]) and its floats' digest (the testkit's
/// [`f64_digest`], over their bits), each followed by a line break.
fn digest<'a>(events: impl Iterator<Item = &'a BodyEvent>) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for event in events {
        let (line, floats) = values_of(event);
        let values: Vec<f64> = floats.iter().map(|&(_, value)| value).collect();
        let text = format!("{line} {:016x}\n", f64_digest(&values));
        for byte in text.bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// Decision-galaxy-25-train §5.2's census over the 400 systems nearest the solar-circle point: the
/// giants present at the epoch that keep great storms and those the water deck's gate removes, by
/// why and by their equilibrium temperatures; and the dusty bodies by their clock's period against
/// the least year, 301 days, with the century's dust storms of those kept, every one of which lasts
/// Mars's 109.9–158.0 days (P14.T31.c's fix).
#[test]
#[ignore = "a measurement for decision-galaxy-25-train §5.2, run by name"]
fn the_storm_census_is_stated() {
    let year_s = 31_557_600.0;
    // (label, T_eq, P in years, envelope state, water deck)
    let mut giants: Vec<Giant> = Vec::new();
    let mut dusty: Vec<Dusty> = Vec::new();
    for ctx in sample() {
        let system = generate(SEED, ctx);
        let models = system.event_models(ctx);
        let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
        for body in &models.bodies {
            let record = snapshot.body(body.index).unwrap();
            let bulk = record.bulk().ok().unwrap();
            let label = format!("{}.{:04x}", ctx.id(), body.index.get());
            let period = record
                .orbit()
                .ok()
                .map_or(f64::NAN, |orbit| orbit.elements().period().value());
            if has_great_storms(bulk.class()) {
                giants.push((
                    label.clone(),
                    bulk.equilibrium_temperature().value(),
                    period / year_s,
                    record.envelope().state(),
                    has_water_deck(record.envelope()),
                ));
            }
            let tau = dust_optical_depth_of(record);
            if !has_dust_storms(tau) {
                continue;
            }
            // A moon's seasons, and its year's gate, are its planet's.
            let primary = match record.identity().kind() {
                BodyKind::Moon(_) => {
                    let Some(OrbitHost::Body(planet)) = record.identity().parent() else {
                        panic!("{label}: a moon's parent is a body");
                    };
                    snapshot.body(planet).unwrap()
                }
                BodyKind::Planet
                | BodyKind::DwarfPlanet
                | BodyKind::Ring
                | BodyKind::Belt(_)
                | BodyKind::CometaryHalo
                | BodyKind::ProtoplanetaryDisc
                | BodyKind::DebrisDisc
                | BodyKind::Unresolved => record,
            };
            let elements = primary.orbit().ok().unwrap().elements();
            let (period, _) = clock_of(elements).unwrap();
            assert_eq!(
                body.dust_storms.is_some(),
                year_holds_dust_storms(period),
                "{label}"
            );
            let mut listed = Vec::new();
            if let Some(model) = &body.dust_storms {
                dust::dust_storms(model, century(), &mut listed);
            }
            dusty.push(Dusty {
                label,
                kind: format!("{:?} {:?}", record.identity().kind(), bulk.class()),
                spin: record.rotation().ok().map_or_else(
                    || "no rotation".to_owned(),
                    |frame| format!("{:?}", frame.state_at(UniverseTime::EPOCH)),
                ),
                t_eq: bulk.equilibrium_temperature().value(),
                period_days: period.as_seconds_f64() / 86_400.0,
                eccentricity: elements.eccentricity(),
                tau,
                kept: body.dust_storms.is_some(),
                durations: listed
                    .iter()
                    .map(|storm| storm.duration().as_seconds_f64() / 86_400.0)
                    .collect(),
            });
        }
    }
    report_giants(&giants);
    report_dusty(&mut dusty);
}

/// A dusty body of the census: its label, its kind and class, its spin at the epoch, `T_eq` (K),
/// its clock's period (days), that orbit's eccentricity, its background τ(550), whether it keeps
/// its dust storms and the durations (days) of the century's.
struct Dusty {
    label: String,
    kind: String,
    spin: String,
    t_eq: f64,
    period_days: f64,
    eccentricity: Eccentricity,
    tau: f64,
    kept: bool,
    durations: Vec<f64>,
}

/// Prints the census's dusty bodies by their clock's period, kept at or above the least year and
/// removed below it, and checks that every kept storm lasts Mars's 109.9–158.0 days.
fn report_dusty(dusty: &mut [Dusty]) {
    let year_days = dust::DURATION_YEAR.as_seconds_f64() / 86_400.0;
    let least_days = dust::LEAST_PERIOD.as_seconds_f64() / 86_400.0;
    let (shortest, longest) = (
        dust::MIN_DURATION_SHARE * year_days,
        dust::MAX_DURATION_SHARE * year_days,
    );
    dusty.sort_by(|a, b| a.period_days.total_cmp(&b.period_days));
    let kept = dusty.iter().filter(|body| body.kept).count();
    eprintln!(
        "dusty bodies: {}; with dust storms (P at least {least_days:.0} d): {kept}; without (P \
         under it): {}",
        dusty.len(),
        dusty.len() - kept
    );
    let (mut lo, mut hi, mut storms) = (f64::INFINITY, f64::NEG_INFINITY, 0);
    for body in dusty.iter() {
        let at =
            |pick: fn(f64, f64) -> f64, from: f64| body.durations.iter().copied().fold(from, pick);
        let (body_lo, body_hi) = (at(f64::min, f64::INFINITY), at(f64::max, f64::NEG_INFINITY));
        for &duration in &body.durations {
            assert!(
                shortest - 1e-9 <= duration && duration < longest,
                "{}: a storm of {duration} d",
                body.label
            );
        }
        lo = lo.min(body_lo);
        hi = hi.max(body_hi);
        storms += body.durations.len();
        eprintln!(
            "| {} | {} | {} | T_eq {:.0} K | P {:.1} d | e {:.3} (contrast {:.0}%) | τ {:.3} | {} | \
             {} storms a century | {} |",
            body.label,
            body.kind,
            body.spin,
            body.t_eq,
            body.period_days,
            body.eccentricity.value(),
            100.0 * dust::perihelion_contrast(body.eccentricity),
            body.tau,
            if body.kept { "kept" } else { "removed" },
            body.durations.len(),
            if body.durations.is_empty() {
                "–".to_owned()
            } else {
                format!("{body_lo:.1}–{body_hi:.1} d")
            }
        );
    }
    eprintln!(
        "kept bodies' storms a century: {storms}, lasting {lo:.1}–{hi:.1} d, within \
         [{shortest:.1}, {longest:.1}] d"
    );
}

/// A giant of the census: its label, `T_eq` (K), orbital period (years), envelope section's state and
/// whether that lists a water deck.
type Giant = (String, f64, f64, SectionState, bool);

/// Prints the census's giants: those that keep great storms and those the gate removes, by why,
/// with their `T_eq` and periods, and the long-period end of the kept.
fn report_giants(giants: &[Giant]) {
    let kept: Vec<_> = giants.iter().filter(|giant| giant.4).collect();
    let no_deck: Vec<_> = giants
        .iter()
        .filter(|giant| !giant.4 && giant.3 == SectionState::Ok)
        .collect();
    let not_modelled: Vec<_> = giants
        .iter()
        .filter(|giant| !giant.4 && giant.3 != SectionState::Ok)
        .collect();
    let range = |of: &[&Giant], at: fn(&Giant) -> f64| {
        let values: Vec<f64> = of.iter().map(|giant| at(giant)).collect();
        (
            values.iter().copied().fold(f64::INFINITY, f64::min),
            values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    eprintln!("census over {} systems", sample().len());
    eprintln!("giants present at the epoch: {}", giants.len());
    for (what, of) in [
        ("keep great storms (water deck)", &kept),
        ("removed: envelope without a water deck", &no_deck),
        ("removed: envelope not modelled", &not_modelled),
    ] {
        let (t_lo, t_hi) = range(of, |giant| giant.1);
        let (p_lo, p_hi) = range(of, |giant| giant.2);
        let floored = of.iter().filter(|giant| giant.2 < 10.0).count();
        eprintln!(
            "| {what} | {} | T_eq {t_lo:.0}–{t_hi:.0} K | P {p_lo:.3}–{p_hi:.1} yr | {floored} under 10 yr |",
            of.len()
        );
    }
    let long: Vec<_> = kept.iter().filter(|giant| giant.2 > 100.0).collect();
    let longest = long.iter().map(|giant| giant.2).fold(0.0, f64::max);
    eprintln!(
        "kept with P over 100 yr: {}; the longest's storms last {:.1}–{:.1} yr",
        long.len(),
        MIN_DURATION_SHARE * longest,
        storm_kind::MAX_DURATION_SHARE * longest
    );
}
