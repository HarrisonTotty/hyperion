//! A system's events as one list (plan 14, P14.T31.c): [`BodyEvent`], one event of any of the five
//! kinds with the body it falls on, and the two inputs decision-galaxy-25-train §5.2 binds on the
//! great storms, the storm clock's floor ([`STORM_PERIOD_FLOOR`], [`storm_period`]) and the water
//! deck their eligibility reads ([`has_water_deck`]).
//!
//! [`PlanetarySystem::events_between`](crate::planetary::PlanetarySystem::events_between) and
//! [`SystemEvents`](crate::planetary::system::SystemEvents) give the list, sorted by time and then
//! by [`EventId`].

use super::{Comet, DustStorm, Eruption, Impact, Storm};
use crate::id::EventId;
use crate::planetary::index::BodyIndex;
use crate::planetary::record::{Envelope, Section};
use crate::substance::by_key;
use crate::time::{Span, UniverseTime};

/// The least period of a giant's storm clock: 10 Julian years, 315,576,000 s
/// (decision-galaxy-25-train §5.2, ours). Part of the generator version.
///
/// A great storm is water's moist convection, suppressed above the water cloud until the
/// troposphere above has cooled radiatively, and then released (Li and Ingersoll 2015, Nature
/// Geosci. 8, 398; Sugiyama et al. 2014, Icarus 229, 71, a model of Jupiter's whose interval is
/// likewise set by radiative cooling): an internal time, not the orbit's. T31.b's clock keeps the
/// orbital period as P (design note 15), so that a giant of a few days' orbit would break out
/// every few days for minutes. Floored at 10 years, the shortest share,
/// [`MIN_DURATION_SHARE`](super::storms::MIN_DURATION_SHARE) 0.0024, lasts at least 8.77 days,
/// against the 26–201 days Saturn's six great white spots lasted (its sources), while Saturn's
/// 29.4 years and Jupiter's 11.9 keep their own.
pub const STORM_PERIOD_FLOOR: Span = Span::from_seconds(10 * 31_557_600);

/// The period of a giant's storm clock for an orbital period `orbital_period`: the larger of the
/// two and [`STORM_PERIOD_FLOOR`], the [`StormInputs::orbital_period`](super::StormInputs) that
/// P14.T31.c passes T31.b's module.
///
/// # Examples
///
/// Saturn keeps its orbit's period; a hot Jupiter takes the floor:
///
/// ```
/// use hyperion_sim::planetary::events::{STORM_PERIOD_FLOOR, storm_period};
/// use hyperion_sim::time::Span;
///
/// let saturn = Span::from_seconds(10_756 * 86_400);
/// assert_eq!(storm_period(saturn), saturn);
/// assert_eq!(storm_period(Span::from_seconds(3 * 86_400)), STORM_PERIOD_FLOOR);
/// ```
#[must_use]
pub fn storm_period(orbital_period: Span) -> Span {
    orbital_period.max(STORM_PERIOD_FLOOR)
}

/// Whether the envelope section `envelope` lists a water (H₂O) condensation deck, liquid or ice
/// (P14.T52.c's decks, on the record from generator version 24): the great storms' eligibility
/// beside [`has_great_storms`](super::storms::has_great_storms)'s class (decision-galaxy-25-train
/// §5.2, point 2).
///
/// The storms are water's moist convection, suppressed and then released above the water cloud
/// (Li and Ingersoll 2015; [`storms`](super::storms)), so a giant with no water cloud has none: a
/// hot or warm giant, whose water stays vapour, and an ice giant whose water deck lies below the
/// record's 100-bar base; Neptune's dark spots, another kind of storm, are not modelled. An
/// envelope section that is not [`Section::Ok`], `NotModelled` among them, lists no deck and gives
/// none. Li and Ingersoll's critical water ratio, about 1%, which Jupiter's lower water may not
/// reach, is not built.
///
/// # Panics
///
/// Never: the registry holds water.
#[must_use]
pub fn has_water_deck(envelope: &Section<Envelope>) -> bool {
    let water = by_key("H2O").expect("the registry holds water");
    envelope.ok().is_some_and(|envelope| {
        envelope
            .atmosphere()
            .decks()
            .iter()
            .any(|deck| deck.condensate() == water)
    })
}

/// One event on a body of a system, of any of P14.T31's five kinds, with the body it falls on
/// (design note 15): what
/// [`PlanetarySystem::events_between`](crate::planetary::PlanetarySystem::events_between) lists.
///
/// Its [`time`](Self::time) is the instant of an impact, the onset of an eruption or a storm, and a
/// comet's perihelion passage. Its [`id`](Self::id) regenerates it
/// ([`SystemEvents::event`](crate::planetary::system::SystemEvents::event)).
///
/// A comet, which carries its orbit and listing, is boxed, so that the many impacts and eruptions
/// of a list stay small.
#[derive(Debug, Clone, PartialEq)]
pub enum BodyEvent {
    /// An impact on `body` (`body.impact`, [`impacts`](super::impacts)).
    Impact {
        /// The body struck.
        body: BodyIndex,
        /// The impact.
        impact: Impact,
    },
    /// An eruption on `body` (`body.eruption`, [`eruptions`](super::eruptions)).
    Eruption {
        /// The body erupting.
        body: BodyIndex,
        /// The eruption.
        eruption: Eruption,
    },
    /// A great storm on the giant `body` (`body.storm`, [`storms`](super::storms)).
    Storm {
        /// The giant.
        body: BodyIndex,
        /// The storm.
        storm: Storm,
    },
    /// A global dust storm on `body` (`body.duststorm`, [`dust_storms`](super::dust_storms)).
    DustStorm {
        /// The body.
        body: BodyIndex,
        /// The dust storm.
        dust_storm: DustStorm,
    },
    /// A long-period comet's perihelion passage, sent in from the system's cometary halo `halo`
    /// (`system.comet`, [`comets`](super::comets)).
    Comet {
        /// The halo.
        halo: BodyIndex,
        /// The comet.
        comet: Box<Comet>,
    },
}

impl BodyEvent {
    /// The event's ID.
    #[must_use]
    pub fn id(&self) -> EventId {
        match self {
            Self::Impact { impact, .. } => impact.id(),
            Self::Eruption { eruption, .. } => eruption.id(),
            Self::Storm { storm, .. } => storm.id(),
            Self::DustStorm { dust_storm, .. } => dust_storm.id(),
            Self::Comet { comet, .. } => comet.id(),
        }
    }

    /// The event's time: an impact's instant, an eruption's or a storm's onset, a comet's
    /// perihelion passage.
    #[must_use]
    pub fn time(&self) -> UniverseTime {
        match self {
            Self::Impact { impact, .. } => impact.time(),
            Self::Eruption { eruption, .. } => eruption.onset(),
            Self::Storm { storm, .. } => storm.onset(),
            Self::DustStorm { dust_storm, .. } => dust_storm.onset(),
            Self::Comet { comet, .. } => comet.perihelion_time(),
        }
    }

    /// The body it falls on: the halo, for a comet.
    #[must_use]
    pub const fn body(&self) -> BodyIndex {
        match self {
            Self::Impact { body, .. }
            | Self::Eruption { body, .. }
            | Self::Storm { body, .. }
            | Self::DustStorm { body, .. }
            | Self::Comet { halo: body, .. } => *body,
        }
    }
}
