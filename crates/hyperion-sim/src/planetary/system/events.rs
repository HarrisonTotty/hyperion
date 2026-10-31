//! A system's events on the record (plan 14, P14.T31.c): [`PlanetarySystem::events_between`], every
//! kind of P14.T31.a and T31.b merged into one list sorted by time and then by [`EventId`], on
//! inputs read once from the system's records at the epoch ([`PlanetarySystem::event_models`],
//! [`SystemEvents`], whose documentation states them), behind their hold at generator version 25
//! ([`STORM_EVENTS_VERSION`]).
//!
//! No event leaves a mark (design note 15): nothing here writes to the system or its records, and
//! no event changes any `body_at` result.

use core::f64::consts::TAU;
use core::fmt::Debug;

use super::{Body, Epoch, Part, PlanetarySystem, SystemHalo, fate_host, moon_state};
use crate::events::TimeWindow;
use crate::galaxy::consts::YEARS_PER_GIGAYEAR;
use crate::id::event_tags::{BODY_DUSTSTORM, BODY_ERUPTION, BODY_IMPACT, BODY_STORM, SYSTEM_COMET};
use crate::id::{EventId, EventSubject, SystemId};
use crate::orbit::KeplerElements;
use crate::planetary::context::SystemContext;
use crate::planetary::events::dust_storms::{self, has_dust_storms, year_holds_dust_storms};
use crate::planetary::events::storms::{self, has_great_storms};
use crate::planetary::events::{
    BodyEvent, CometInputs, CometModel, DustStorm, DustStormInputs, DustStormModel, Eruption,
    EruptionInputs, EruptionModel, Impact, ImpactInputs, ImpactModel, Storm, StormInputs,
    StormModel, comets, eruptions, has_water_deck, impacts, storm_period,
};
use crate::planetary::fate::{BodyFate, BodyState};
use crate::planetary::hooks::figures::{ImpactorBody, ImpactorOrbit, impactors};
use crate::planetary::index::BodyIndex;
use crate::planetary::label;
use crate::planetary::placement::OrbitHost;
use crate::planetary::record::{
    BodyRecord, BulkProperties, Modelled, Population, Section, Surface, SystemSnapshot,
};
use crate::planetary::satellites::Satellite;
use crate::substance::by_key;
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::{
    Gigayears, GravitationalParameter, Kilograms, Metres, SolarLuminosities, SolarMasses, Years,
};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which a system's events are on the record: 25, the first planetary
/// bump after 24's, whose membership closed before P14.T31.c (decision-galaxy-25-train §5.2,
/// "Scheduling": its goldens over generated bodies board 25).
///
/// It is P14.T31.c's one hold, over all five kinds. Below it [`StormEventsRules::in_force`] is
/// [`StormEventsRules::Held`]: [`PlanetarySystem::event_models`] reads nothing and gives no model,
/// so that [`PlanetarySystem::events_between`] lists no event, and nothing generated moves. 25's
/// bump deletes this hold, [`StormEventsRules`] with its `Held` arm, the tests' switch and its
/// tripwire (`the_body_events_are_held_only_until_the_25_bump`).
pub(crate) const STORM_EVENTS_VERSION: GeneratorVersion = GeneratorVersion::new(25);

/// Whether P14.T31.c's events are on the record (its hold, [`STORM_EVENTS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum StormEventsRules {
    /// Version 24's and before: no system has events.
    Held,
    /// Version 25's: every kind on the record's inputs at the epoch, the storms with
    /// decision-galaxy-25-train §5.2's floor and water deck.
    Wired,
}

impl StormEventsRules {
    /// The rules this generator version gives: the events from [`STORM_EVENTS_VERSION`], none
    /// below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_storm_events`]); behind the `testing` feature, the events wherever another crate's
    /// test binary has wired them for its whole process
    /// ([`wire_body_events_in_this_process`](crate::planetary::testing::wire_body_events_in_this_process)).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = RULES.with(core::cell::Cell::get) {
            return rules;
        }
        #[cfg(feature = "testing")]
        if WIRED_IN_THIS_PROCESS.load(core::sync::atomic::Ordering::Relaxed) {
            return Self::Wired;
        }
        if GENERATOR_VERSION >= STORM_EVENTS_VERSION {
            Self::Wired
        } else {
            Self::Held
        }
    }
}

/// Whether another crate's test binary has put the events on the record for every thread of its
/// process ahead of their hold
/// ([`wire_body_events_in_this_process`](crate::planetary::testing::wire_body_events_in_this_process)):
/// its code answers on threads of its own, such as the server's pool, which no thread's switch
/// reaches. Never set in this crate's tests, nor outside the `testing` feature. 25's bump deletes it
/// with the hold.
#[cfg(feature = "testing")]
pub(crate) static WIRED_IN_THIS_PROCESS: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Whether this generator version puts P14.T31.c's events on the record: from 25, its hold
/// (`STORM_EVENTS_VERSION`), and in a test where its switch, or the process-wide one another
/// crate's test binary sets, puts them there ahead of it.
///
/// A server answers `body_events` only where it is `true`, and otherwise as a server built before
/// the events would, so that no client reads a held version's empty list as a quiet system. 25's
/// bump deletes it with the hold.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::system::events_on_the_record;
///
/// // Version 24, under the hold.
/// assert!(!events_on_the_record());
/// ```
#[must_use]
pub fn events_on_the_record() -> bool {
    StormEventsRules::in_force() == StormEventsRules::Wired
}

#[cfg(test)]
thread_local! {
    /// The events' rules a test sets on its thread, `None` for the version's.
    static RULES: core::cell::Cell<Option<StormEventsRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the events' rules `rules` on this thread, for the tests to reach the events
/// ahead of the bump; the version's rules are set back after, a panic in `run` included. 25's bump
/// deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_storm_events<R>(rules: StormEventsRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<StormEventsRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// Puts the events on the record for every thread of this process, ahead of their hold
/// ([`WIRED_IN_THIS_PROCESS`]); one-way.
#[cfg(feature = "testing")]
pub(crate) fn wire_in_this_process() {
    WIRED_IN_THIS_PROCESS.store(true, core::sync::atomic::Ordering::Relaxed);
}

/// A system's event models, every kind's for each of its bodies, read once from its records at
/// the epoch ([`PlanetarySystem::event_models`]): what [`PlanetarySystem::events_between`] lists
/// from, for a caller that asks about one system again and again to keep.
///
/// # The inputs, at the epoch
///
/// Each kind's rate, clock and marks read the body's record at [`UniverseTime::EPOCH`] (plan 06's
/// design note 23: rates at the epoch age, so that no rate depends on the time asked), for each
/// planet, belt member and moon present then:
///
/// - **Impacts** (`body.impact`): the bulk section's radius and the system's age at the epoch,
///   with the impactors of the surface section's figures (P14.T48.f). A planet or a member without
///   a surface section, a giant or a sub-Neptune, has its impactors found as its figures would
///   find them, on its primordial orbit about its host's sources and its bulk section; its
///   equilibrium temperature and no water stand for the surface those read, which move only the
///   crust and the crater target, neither of which an impact reads. A moon without a surface
///   section has none.
/// - **Eruptions** (`body.eruption`): a body with a surface section, at its figures' heat flow and
///   its bulk radius.
/// - **Great storms** (`body.storm`): a giant ([`has_great_storms`]) whose envelope section lists
///   a water deck ([`has_water_deck`]), on a clock of the record's orbital period floored at
///   [`STORM_PERIOD_FLOOR`](crate::planetary::events::STORM_PERIOD_FLOOR) ([`storm_period`]) from
///   a perihelion passage (decision-galaxy-25-train §5.2, points 1 and 2).
/// - **Global dust storms** (`body.duststorm`): a body with a surface section whose aerosols hold
///   dust ([`has_dust_storms`] of the `mars_dust` mode's τ(550)) on an orbital period of at least
///   [`LEAST_PERIOD`](dust_storms::LEAST_PERIOD), 301 days ([`year_holds_dust_storms`]), on a
///   clock of that period from a perihelion passage with the orbit's eccentricity. A moon's seasons
///   are its planet's, so a moon's clock and its period's gate are its planet's orbit about its
///   host.
/// - **Comets** (`system.comet`): the cometary halo present at the epoch, at its record's comet
///   rate, about its host's stars then: their summed luminosity and mass, and the largest one's
///   radius, into which a comet plunges.
///
/// A perihelion passage is the epoch less the elements' mean anomaly then, as a share of their
/// period. A model a kind refuses (a period under
/// [`LinearClock::MIN_PERIOD`](crate::events::LinearClock::MIN_PERIOD), an input out of its range)
/// leaves that kind out for the body: the generator's own inputs never reach one, which the tests'
/// debug builds check.
///
/// # Each body's existence
///
/// A body's events are listed only while it is present ([`BodyFate::formed_at`] to its
/// [`ending`](BodyFate::ending), a moon also until its own loss), so that a body destroyed or
/// unbound within the window has none after; the halo's comets end where its record is unbound
/// within the clock window, at its host's first death. A body not present at the epoch, and every
/// body of a system not yet born then, has no events: its inputs are not on the record.
///
/// # Examples
///
/// A system's events of a century, in time order, each of a body present then:
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::planetary::{SystemContext, generate};
/// use hyperion_sim::time::UniverseTime;
///
/// let seed = Seed::new(0x5eed_0000_0014_0032);
/// let galaxy = Galaxy::from_params(seed, GalaxyParams::milky_way_like())?;
/// let ctx = SystemContext::for_system(&galaxy, SystemId::from_raw(0x4200_6cba_0000_0009)?)?;
/// let system = generate(seed, &ctx);
/// let models = system.event_models(&ctx);
/// let century = TimeWindow::new(
///     UniverseTime::from_julian_years(-50).ok_or("in range")?,
///     UniverseTime::from_julian_years(50).ok_or("in range")?,
/// )?;
/// let events = models.between(century);
/// assert!(events.windows(2).all(|pair| (pair[0].time(), pair[0].id()) < (pair[1].time(), pair[1].id())));
/// assert_eq!(events, system.events_between(&ctx, century.start(), century.end()));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SystemEvents {
    system: SystemId,
    bodies: Vec<BodyModels>,
    comets: Option<HaloComets>,
}

/// One body's event models, with when it is present.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BodyModels {
    index: BodyIndex,
    presence: Presence,
    impacts: Option<ImpactModel>,
    eruptions: Option<EruptionModel>,
    storms: Option<StormModel>,
    dust_storms: Option<DustStormModel>,
}

/// The cometary halo's comets, until the halo is unbound if it is within the clock window.
#[derive(Debug, Clone, Copy, PartialEq)]
struct HaloComets {
    halo: BodyIndex,
    model: CometModel,
    until: Option<UniverseTime>,
}

impl HaloComets {
    /// Whether the halo is there to send in a comet at `t`.
    #[must_use]
    fn holds_at(&self, t: UniverseTime) -> bool {
        self.until.is_none_or(|until| t < until)
    }
}

/// The events [`SystemEvents::listed_about_middle`] lists, and the span they are every event of.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedEvents {
    span: TimeWindow,
    events: Vec<BodyEvent>,
}

impl ListedEvents {
    /// The span every event of which is listed: the window asked about, or a part of it about its
    /// middle.
    #[must_use]
    pub const fn span(&self) -> TimeWindow {
        self.span
    }

    /// The span's events, sorted by time and then by [`EventId`].
    #[must_use]
    pub fn events(&self) -> &[BodyEvent] {
        &self.events
    }
}

/// The grains of half a window a listing's span about its middle is measured in
/// ([`SystemEvents::listed_about_middle`]): 2¹⁵, each rounded down to whole nanoseconds and at
/// least one, so about 0.56 days of a century and 5.6 days of a 1,000-year window.
const LISTING_GRAINS: i64 = 1 << 15;

/// [`LISTING_GRAINS`] as a `u32`, for the tests' arithmetic in floats.
#[cfg(test)]
const LISTING_GRAINS_U32: u32 = 1 << 15;

/// Nanoseconds in a second.
const NANOS_PER_SECOND: i128 = 1_000_000_000;

/// `span` in whole nanoseconds.
#[must_use]
fn nanos_of(span: Span) -> i128 {
    i128::from(span.seconds()) * NANOS_PER_SECOND + i128::from(span.subsec_nanos())
}

/// The instant `nanos` nanoseconds from the epoch.
///
/// # Panics
///
/// Never for an instant from a window's start to its end, which the clock holds.
#[must_use]
fn time_of_nanos(nanos: i128) -> UniverseTime {
    let seconds = i64::try_from(nanos.div_euclid(NANOS_PER_SECOND))
        .expect("an instant of a window is one of the clock's");
    let nanos = u32::try_from(nanos.rem_euclid(NANOS_PER_SECOND))
        .expect("a remainder of a second is below 10⁹");
    UniverseTime::new(seconds, nanos)
        .expect("whole seconds and a remainder below a second are an instant")
}

/// When a planet or a belt member is present: from its formation, if it forms, until its ending,
/// if it ends ([`BodyFate::formed_at`], [`BodyFate::ending`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Lifetime {
    formed_at: Option<UniverseTime>,
    ending: Option<BodyState>,
}

impl Lifetime {
    /// The lifetime `fate` gives.
    #[must_use]
    const fn of(fate: &BodyFate<'_>) -> Self {
        Self {
            formed_at: fate.formed_at(),
            ending: fate.ending(),
        }
    }

    /// The body's state at `t`, as [`BodyFate::at`] gives it.
    #[must_use]
    fn state_at(&self, t: UniverseTime) -> BodyState {
        match self.formed_at {
            Some(formed) if t >= formed => {}
            Some(_) | None => return BodyState::NotYetFormed,
        }
        match self.ending {
            Some(ending) if ending.ended_at().is_some_and(|at| t >= at) => ending,
            Some(_) | None => BodyState::Present,
        }
    }
}

/// When a body is present: a planet's or member's [`Lifetime`], or a moon's planet's with its
/// own loss, as its record gives it.
#[derive(Debug, Clone, Copy, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "one a body in models built once a call of `event_models`: boxing a moon's satellite \
              would allocate for each moon to spare every other body its bytes"
)]
enum Presence {
    /// A planet or a belt member.
    Primary(Lifetime),
    /// A moon, of its planet's lifetime, in a system `age_at_epoch` old at the epoch.
    Moon {
        planet: Lifetime,
        satellite: Satellite,
        age_at_epoch: Years,
    },
}

impl Presence {
    /// Whether the body is present at `t`.
    #[must_use]
    fn holds_at(&self, t: UniverseTime) -> bool {
        let state = match self {
            Self::Primary(lifetime) => lifetime.state_at(t),
            Self::Moon {
                planet,
                satellite,
                age_at_epoch,
            } => moon_state(planet.state_at(t), satellite, *age_at_epoch, t),
        };
        state == BodyState::Present
    }
}

/// What [`BodyModels::list`] lists each kind into, emptied as it goes, so that one set of buffers
/// serves every body of a listing.
#[derive(Debug, Default)]
struct Found {
    impacts: Vec<Impact>,
    eruptions: Vec<Eruption>,
    storms: Vec<Storm>,
    dust_storms: Vec<DustStorm>,
}

impl BodyModels {
    /// Appends to `out` the body's events in `window` while it is present, kind by kind, each kind
    /// listed into its buffer of `found` and drained from it.
    fn list(&self, window: TimeWindow, found: &mut Found, out: &mut Vec<BodyEvent>) {
        let body = self.index;
        let present = |t: UniverseTime| self.presence.holds_at(t);
        if let Some(model) = &self.impacts {
            impacts::impacts(model, window, &mut found.impacts);
            out.extend(
                found
                    .impacts
                    .drain(..)
                    .filter(|impact| present(impact.time()))
                    .map(|impact| BodyEvent::Impact { body, impact }),
            );
        }
        if let Some(model) = &self.eruptions {
            eruptions::eruptions(model, window, &mut found.eruptions);
            out.extend(
                found
                    .eruptions
                    .drain(..)
                    .filter(|eruption| present(eruption.onset()))
                    .map(|eruption| BodyEvent::Eruption { body, eruption }),
            );
        }
        if let Some(model) = &self.storms {
            storms::storms(model, window, &mut found.storms);
            out.extend(
                found
                    .storms
                    .drain(..)
                    .filter(|storm| present(storm.onset()))
                    .map(|storm| BodyEvent::Storm { body, storm }),
            );
        }
        if let Some(model) = &self.dust_storms {
            dust_storms::dust_storms(model, window, &mut found.dust_storms);
            out.extend(
                found
                    .dust_storms
                    .drain(..)
                    .filter(|dust_storm| present(dust_storm.onset()))
                    .map(|dust_storm| BodyEvent::DustStorm { body, dust_storm }),
            );
        }
    }

    /// The body's event `id`, of the kind its tag names, if the body is present then.
    #[must_use]
    fn event(&self, id: EventId) -> Option<BodyEvent> {
        let body = self.index;
        let tag = id.word().tag();
        let event = if tag == BODY_IMPACT {
            impacts::impact(self.impacts.as_ref()?, id)
                .map(|impact| BodyEvent::Impact { body, impact })
        } else if tag == BODY_ERUPTION {
            eruptions::eruption(self.eruptions.as_ref()?, id)
                .map(|eruption| BodyEvent::Eruption { body, eruption })
        } else if tag == BODY_STORM {
            storms::storm(self.storms.as_ref()?, id).map(|storm| BodyEvent::Storm { body, storm })
        } else if tag == BODY_DUSTSTORM {
            dust_storms::dust_storm(self.dust_storms.as_ref()?, id)
                .map(|dust_storm| BodyEvent::DustStorm { body, dust_storm })
        } else {
            None
        }?;
        self.presence.holds_at(event.time()).then_some(event)
    }
}

impl SystemEvents {
    /// No models: a system's under the hold ([`STORM_EVENTS_VERSION`]).
    #[must_use]
    const fn none(system: SystemId) -> Self {
        Self {
            system,
            bodies: Vec::new(),
            comets: None,
        }
    }

    /// The system the models are of.
    #[must_use]
    pub const fn system(&self) -> SystemId {
        self.system
    }

    /// The bytes the models own on the heap, beyond `size_of::<SystemEvents>()`: one entry a
    /// planet, member and moon present at the epoch, by capacity, each holding nothing of its own on
    /// the heap. It is what a server charges cached models against its byte budget (plan 14,
    /// P14.T36.b); nothing generated reads it.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.bodies.capacity() * size_of::<BodyModels>()
    }

    /// Whether no body of the system has an event of any kind: a system with no body present at
    /// the epoch, or one under the hold (generator versions before 25).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.comets.is_none()
            && self.bodies.iter().all(|body| {
                body.impacts.is_none()
                    && body.eruptions.is_none()
                    && body.storms.is_none()
                    && body.dust_storms.is_none()
            })
    }

    /// The system's events whose time lies in `window` ([`BodyEvent::time`]), each while its body
    /// is present, sorted by time and then by [`EventId`].
    ///
    /// Each event is a function of its own bin or cycle, and whether its body is present of its
    /// own time, so the listing over a window is the listings over any partition of it, one after
    /// another, in any order of asking.
    #[must_use]
    pub fn between(&self, window: TimeWindow) -> Vec<BodyEvent> {
        let mut out = Vec::new();
        let mut found = Found::default();
        for body in &self.bodies {
            body.list(window, &mut found, &mut out);
        }
        if let Some(comets) = &self.comets {
            let mut listed = Vec::new();
            comets::comets(&comets.model, window, &mut listed);
            out.extend(
                listed
                    .into_iter()
                    .filter(|comet| comets.holds_at(comet.perihelion_time()))
                    .map(|comet| BodyEvent::Comet {
                        halo: comets.halo,
                        comet: Box::new(comet),
                    }),
            );
        }
        // An ID names one event, so the order is total and the sort's stability moot.
        out.sort_unstable_by_key(|event| (event.time(), event.id()));
        out
    }

    /// The events of `window`, at most `most` of them, and the span they are every event of
    /// ([`ListedEvents`]): the whole window where it holds no more; otherwise the widest span about
    /// the window's middle, reaching as far either side as the window allows, of a half-width a
    /// whole number of grains, 2¹⁵ of them to the window's half, that holds no more. Where even
    /// one grain either side holds more, the span is the empty one at the middle.
    ///
    /// It is for a caller that must bound what it lists, as a server bounds an answer (plan 14,
    /// P14.T36.b). The span is found by doubling its half-width from one grain until it holds more
    /// than `most`, then halving the gap, so at most about 32 listings ([`between`](Self::between))
    /// are made, each of a span the last one holding no more than `most` at most doubles; a span's
    /// count of events never falls as it widens.
    ///
    /// # Panics
    ///
    /// Never: the span is worked in nanoseconds from the epoch, in which no window overflows, and
    /// held to the window's ends, which the clock holds.
    ///
    /// # Examples
    ///
    /// No more than `most` events are listed, and every event of the span is:
    ///
    /// ```
    /// use hyperion_sim::events::TimeWindow;
    /// use hyperion_sim::planetary::{SystemContext, generate};
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::{Dex, SolarMasses, Years};
    /// use hyperion_sim::{Seed, id::SystemId};
    ///
    /// let ctx = SystemContext::builder()
    ///     .system(SystemId::from_raw(0x0200_0800_2000_0000)?)
    ///     .star(SolarMasses::new(1.0))
    ///     .fe_h(Dex::ZERO)
    ///     .age_at_epoch(Years::new(4.6e9))
    ///     .build()?;
    /// let models = generate(Seed::new(7), &ctx).event_models(&ctx);
    /// let century = TimeWindow::new(
    ///     UniverseTime::from_julian_years(-50).ok_or("in range")?,
    ///     UniverseTime::from_julian_years(50).ok_or("in range")?,
    /// )?;
    /// let listed = models.listed_about_middle(century, 10);
    /// assert!(listed.events().len() <= 10);
    /// assert_eq!(listed.events(), models.between(listed.span()));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn listed_about_middle(&self, window: TimeWindow, most: usize) -> ListedEvents {
        // In nanoseconds from the epoch, where even the clock's whole window is far from
        // overflowing an `i128`.
        let start = nanos_of(window.start().since_epoch());
        let end = nanos_of(window.end().since_epoch());
        let half = (end - start) / 2;
        let middle = start + half;
        let grain = (half / i128::from(LISTING_GRAINS)).max(1);
        // Enough grains either side to reach both of the window's ends.
        let whole = (half + grain) / grain;
        let span = |grains: i128| {
            let reach = grains * grain;
            TimeWindow::new(
                time_of_nanos((middle - reach).max(start)),
                time_of_nanos((middle + reach).min(end)),
            )
            .expect("a span about the middle has its ends in order")
        };
        let (mut good, mut events) = (0, Vec::new());
        let mut grains = 1;
        let bad = loop {
            let found = self.between(span(grains));
            if found.len() > most {
                break grains;
            }
            (good, events) = (grains, found);
            if grains >= whole {
                return ListedEvents {
                    span: span(grains),
                    events,
                };
            }
            grains = (grains * 2).min(whole);
        };
        let mut bad = bad;
        while bad - good > 1 {
            let mid = good + (bad - good) / 2;
            let found = self.between(span(mid));
            if found.len() > most {
                bad = mid;
            } else {
                (good, events) = (mid, found);
            }
        }
        ListedEvents {
            span: span(good),
            events,
        }
    }

    /// The event `id` names, regenerated on its own: the one [`between`](Self::between) lists
    /// under that ID.
    ///
    /// `None` for an ID of another system, of a body or a kind with no model, of an event its kind
    /// does not have (a skipped cycle, a number beyond its bin's count), or of an event at a time
    /// its body is not present.
    #[must_use]
    pub fn event(&self, id: EventId) -> Option<BodyEvent> {
        match id.subject() {
            EventSubject::Body(body) if body.system() == self.system => self
                .bodies
                .iter()
                .find(|models| models.index.get() == body.body_index())?
                .event(id),
            EventSubject::System(system) if system == self.system => {
                let comets = self.comets.as_ref()?;
                if id.word().tag() != SYSTEM_COMET {
                    return None;
                }
                comets::comet(&comets.model, id)
                    .filter(|comet| comets.holds_at(comet.perihelion_time()))
                    .map(|comet| BodyEvent::Comet {
                        halo: comets.halo,
                        comet: Box::new(comet),
                    })
            }
            EventSubject::Body(_) | EventSubject::System(_) => None,
        }
    }
}

impl PlanetarySystem {
    /// The system's events whose time lies in [`from`, `to`), each while its body is present,
    /// sorted by time and then by [`EventId`] (plan 14, P14.T31.c): its bodies' impacts, eruptions,
    /// great storms and global dust storms, and its halo's comets ([`BodyEvent`]), on inputs read
    /// from its records at the epoch ([`event_models`](Self::event_models), [`SystemEvents`]); none
    /// where `to` is not after `from`.
    ///
    /// None at all before generator version 25 (its hold, `STORM_EVENTS_VERSION`).
    ///
    /// It reads the system's records at the epoch on every call, a whole
    /// [`snapshot_at`](Self::snapshot_at): a caller asking about one system again keeps its
    /// [`SystemEvents`] instead.
    ///
    /// # Panics
    ///
    /// As [`snapshot_at`](Self::snapshot_at).
    #[must_use]
    pub fn events_between(
        &self,
        ctx: &SystemContext,
        from: UniverseTime,
        to: UniverseTime,
    ) -> Vec<BodyEvent> {
        // A window that ends before it starts holds no time, so no event.
        let Ok(window) = TimeWindow::new(from, to) else {
            return Vec::new();
        };
        if window.is_empty() {
            return Vec::new();
        }
        self.event_models(ctx).between(window)
    }

    /// The system's event models, read once from its records at the epoch ([`SystemEvents`]'
    /// documentation): none before generator version 25 (its hold, `STORM_EVENTS_VERSION`).
    ///
    /// # Panics
    ///
    /// As [`snapshot_at`](Self::snapshot_at).
    #[must_use]
    pub fn event_models(&self, ctx: &SystemContext) -> SystemEvents {
        match StormEventsRules::in_force() {
            StormEventsRules::Held => return SystemEvents::none(self.system),
            StormEventsRules::Wired => {}
        }
        // A rogue planet's bodies have no events (P14.T27.b): impacts are a host's belts', storms
        // a host's light.
        if self.rogue_planet().is_some() {
            return SystemEvents::none(self.system);
        }
        let snapshot = self.snapshot_at(ctx, UniverseTime::EPOCH);
        let system_age = Gigayears::new(ctx.age_at_epoch().value() / YEARS_PER_GIGAYEAR);
        let mut bodies = Vec::new();
        for (body, presence) in self.presences(ctx) {
            let record = snapshot
                .body(body.index)
                .expect("a snapshot holds every body of its system");
            if record.identity().state() != BodyState::Present {
                continue;
            }
            let Some(bulk) = record.bulk().ok() else {
                continue;
            };
            let orbit = match body.part {
                Part::Moon(_) => snapshot
                    .body(parent_of(body))
                    .and_then(|parent| parent.orbit().ok()),
                Part::Planet(_) | Part::Member(_) | Part::Ring(_) => record.orbit().ok(),
            }
            .map(|orbit| *orbit.elements());
            bodies.push(BodyModels {
                index: body.index,
                presence,
                impacts: self.impact_model(body, record, bulk, system_age),
                eruptions: self.eruption_model(body, record, bulk),
                storms: self.storm_model(body, record, bulk, orbit.as_ref()),
                dust_storms: self.dust_storm_model(body, record, orbit.as_ref()),
            });
        }
        SystemEvents {
            system: self.system,
            bodies,
            comets: self
                .halo
                .as_ref()
                .and_then(|halo| self.halo_comets(ctx, &snapshot, halo)),
        }
    }

    /// Every planet, member and moon of the system with when it is present, in body order: a
    /// planet's and member's lifetime from its fate about its zone's host, a moon's its planet's
    /// with its own loss.
    #[must_use]
    fn presences(&self, ctx: &SystemContext) -> Vec<(&Body, Presence)> {
        let hosts: Vec<_> = self.zones.iter().map(|zone| fate_host(ctx, zone)).collect();
        let fates = self.fates(&hosts);
        let mut lifetimes: Vec<(BodyIndex, Lifetime)> = Vec::new();
        let mut presences = Vec::with_capacity(self.bodies.len());
        for (body, fate) in self.bodies.iter().zip(&fates) {
            let presence = match &body.part {
                Part::Planet(_) | Part::Member(_) => {
                    let lifetime = Lifetime::of(
                        fate.as_ref()
                            .expect("every planet and member orbits one of its system's zones"),
                    );
                    lifetimes.push((body.index, lifetime));
                    Presence::Primary(lifetime)
                }
                Part::Moon(moon) => {
                    let parent = parent_of(body);
                    let planet = lifetimes
                        .iter()
                        .find(|(at, _)| *at == parent)
                        .map(|(_, lifetime)| *lifetime)
                        .expect("a satellite's parent comes before it in index order");
                    Presence::Moon {
                        planet,
                        satellite: moon.satellite,
                        age_at_epoch: ctx.age_at_epoch(),
                    }
                }
                Part::Ring(_) => continue,
            };
            presences.push((body, presence));
        }
        presences
    }

    /// The impacts of the present `body` of record `record` and bulk `bulk`, in a system
    /// `system_age` old at the epoch: its figures' impactors, or for a planet or a member without
    /// figures its impactors found on its primordial orbit ([`SystemEvents`]' documentation).
    #[must_use]
    fn impact_model(
        &self,
        body: &Body,
        record: &BodyRecord,
        bulk: &BulkProperties,
        system_age: Gigayears,
    ) -> Option<ImpactModel> {
        let radius = Metres::from(bulk.radius());
        let found = match record.surface().ok() {
            Some(surface) => *surface.figures().impactors(),
            None => match body.part {
                Part::Planet(_) | Part::Member(_) => {
                    let sources = self.impactor_sources(body.host)?;
                    kept(
                        body.index,
                        impactors(&ImpactorBody {
                            orbit: ImpactorOrbit::Host(self.host_orbiter(body)?),
                            radius,
                            density: bulk.density(),
                            surface_gravity: bulk.surface_gravity(),
                            surface_temperature: bulk.equilibrium_temperature(),
                            water: Kilograms::ZERO,
                            sources: &sources,
                        }),
                    )?
                }
                Part::Moon(_) | Part::Ring(_) => return None,
            },
        };
        kept(
            body.index,
            ImpactModel::new(
                self.seed,
                body.index.body_id(self.system),
                &ImpactInputs::new(radius, &found, system_age),
            ),
        )
    }

    /// The eruptions of the present `body` of record `record` and bulk `bulk`: at its figures'
    /// heat flow, for a body with a surface section.
    #[must_use]
    fn eruption_model(
        &self,
        body: &Body,
        record: &BodyRecord,
        bulk: &BulkProperties,
    ) -> Option<EruptionModel> {
        let surface = record.surface().ok()?;
        kept(
            body.index,
            EruptionModel::new(
                self.seed,
                body.index.body_id(self.system),
                &EruptionInputs {
                    heat_flow: surface.figures().heat_flow(),
                    radius: Metres::from(bulk.radius()),
                },
            ),
        )
    }

    /// The great storms of the present `body` of record `record` and bulk `bulk` on the orbit
    /// `orbit`: a giant's with a water deck, on its orbital period floored at
    /// [`STORM_PERIOD_FLOOR`](crate::planetary::events::STORM_PERIOD_FLOOR) from a perihelion
    /// passage.
    #[must_use]
    fn storm_model(
        &self,
        body: &Body,
        record: &BodyRecord,
        bulk: &BulkProperties,
        orbit: Option<&KeplerElements>,
    ) -> Option<StormModel> {
        if !(has_great_storms(bulk.class()) && has_water_deck(record.envelope())) {
            return None;
        }
        let (period, perihelion) = clock_of(orbit?)?;
        kept(
            body.index,
            StormModel::new(
                self.seed,
                body.index.body_id(self.system),
                &StormInputs {
                    orbital_period: storm_period(period),
                    origin: perihelion,
                },
            ),
        )
    }

    /// The global dust storms of the present `body` of record `record`, whose seasons follow the
    /// orbit `orbit`: a body's with dust in its air, on an orbital period of at least
    /// [`LEAST_PERIOD`](dust_storms::LEAST_PERIOD), from a perihelion passage. The period is gated
    /// before the model is built, since [`kept`] holds that the generator's inputs are never
    /// refused.
    #[must_use]
    fn dust_storm_model(
        &self,
        body: &Body,
        record: &BodyRecord,
        orbit: Option<&KeplerElements>,
    ) -> Option<DustStormModel> {
        if !has_dust_storms(dust_optical_depth_of(record)) {
            return None;
        }
        let orbit = orbit?;
        let (period, perihelion) = clock_of(orbit)?;
        if !year_holds_dust_storms(period) {
            return None;
        }
        kept(
            body.index,
            DustStormModel::new(
                self.seed,
                body.index.body_id(self.system),
                &DustStormInputs {
                    orbital_period: period,
                    perihelion,
                    eccentricity: orbit.eccentricity(),
                },
            ),
        )
    }

    /// The comets of the system's halo `halo`, if its record in `snapshot`, the epoch's, is
    /// present: at its record's comet rate, about its host's stars at the epoch, until its record
    /// is unbound if that falls within the clock window ([`SystemEvents`]' documentation).
    #[must_use]
    fn halo_comets(
        &self,
        ctx: &SystemContext,
        snapshot: &SystemSnapshot,
        halo: &SystemHalo,
    ) -> Option<HaloComets> {
        let index = halo.halo.index();
        let record = snapshot.body(index)?;
        let Section::Ok(Population::CometaryHalo(population)) = record.population() else {
            return None;
        };
        let zone = self.zone(halo.host)?;
        let states: Option<Vec<_>> = zone
            .members()
            .map(|member| ctx.stars()[usize::from(member)].state_at(snapshot.time()))
            .collect();
        let states = states?;
        let luminosity = states
            .iter()
            .fold(0.0, |sum, state| sum + state.luminosity().value());
        let mass = states
            .iter()
            .fold(SolarMasses::ZERO, |sum, state| sum + state.mass());
        let radius = states
            .iter()
            .map(|state| Metres::from(state.radius()).value())
            .fold(0.0, f64::max);
        let model = kept(
            index,
            CometModel::new(
                self.seed,
                self.system,
                &CometInputs {
                    rate: population.comet_rate(),
                    luminosity: SolarLuminosities::new(luminosity),
                    host: GravitationalParameter::from_solar_masses(mass),
                    host_radius: Metres::new(radius),
                },
            ),
        )?;
        // The halo's state at the window's end: unbound there, it was unbound at its host's first
        // death, which no later state undoes.
        let label = label::label(self, index).expect("every body of a system has a label");
        let end = self.halo_record(&Epoch::new(self, ctx, ClockWindow::END), halo, label);
        let until = match end.identity().state() {
            BodyState::Unbound { at } | BodyState::Destroyed { at, .. } => Some(at),
            BodyState::Present | BodyState::NotYetFormed => None,
        };
        Some(HaloComets {
            halo: index,
            model,
            until,
        })
    }
}

/// `built`'s model, or `None` where its kind refused the body `index`'s inputs, which leaves that
/// kind out for the body ([`SystemEvents`]' documentation). The generator's own inputs never reach
/// a refusal, so one is a bug, which the tests' debug builds catch.
#[must_use]
fn kept<T, E: Debug>(index: BodyIndex, built: Result<T, E>) -> Option<T> {
    debug_assert!(built.is_ok(), "{index:?}: {:?}", built.as_ref().err());
    built.ok()
}

/// The index of the moon `body`'s parent.
///
/// # Panics
///
/// If `body` is not a satellite, whose host is a body.
#[must_use]
fn parent_of(body: &Body) -> BodyIndex {
    match body.host {
        OrbitHost::Body(parent) => parent,
        OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre => {
            unreachable!("a moon's host is a body")
        }
    }
}

/// The τ(550) of the dust in the air of `record`'s surface section, 0 where it has none (the
/// `mars_dust` mode of P14.T24.c's aerosols).
///
/// # Panics
///
/// Never: the registry holds `mars_dust`.
#[must_use]
fn dust_optical_depth_of(record: &BodyRecord) -> f64 {
    let dust = by_key("mars_dust").expect("the registry holds mars_dust");
    match record.surface().ok().map(Surface::aerosols) {
        Some(Modelled::Ok(aerosols)) => aerosols.deck_optical_depth(dust).unwrap_or(0.0),
        Some(Modelled::NotModelled) | None => 0.0,
    }
}

/// The period of `orbit` as a span, with its perihelion passage before or at the epoch: the epoch
/// less its mean anomaly then as a share of the period. `None` for a period no span holds.
#[must_use]
fn clock_of(orbit: &KeplerElements) -> Option<(Span, UniverseTime)> {
    let period = orbit.period().value();
    let since = orbit.mean_anomaly_at_epoch().value() / TAU * period;
    Some((
        Span::from_seconds_f64(period)?,
        UniverseTime::EPOCH.checked_sub(Span::from_seconds_f64(since)?)?,
    ))
}

#[cfg(test)]
mod tests;
