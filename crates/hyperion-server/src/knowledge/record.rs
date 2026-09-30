//! What the crew knows of one contact: its sightings, the level reached, and the views the wire
//! may carry (plan 12, P12.T7.a; Design note 10).

use std::fmt;
use std::num::NonZeroU32;

use hyperion_sim::alerts::AlertBand;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::id::{EventId, SystemId};
use hyperion_sim::observe::{Bearing, Observer, Retardation, bearing, extrapolate_to_present};
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::WattsPerSquareMetre;

/// A contact's identifier on the wire: opaque and sequential per universe, from 1, so that an
/// unresolved contact cannot leak its host through its identifier (Design note 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContactId(NonZeroU32);

impl ContactId {
    /// The first contact of a universe.
    pub(crate) const FIRST: Self = Self(NonZeroU32::MIN);

    /// The contact numbered `number`, or `None` for 0, which no contact has.
    #[must_use]
    pub fn new(number: u32) -> Option<Self> {
        NonZeroU32::new(number).map(Self)
    }

    /// The number, from 1.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// The contact after this one, or `None` past 2³² − 1 contacts.
    #[must_use]
    pub(crate) fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

impl fmt::Display for ContactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "contact {}", self.0)
    }
}

/// How much the crew knows of a contact: its levels in order, so that the highest reached is the
/// greatest (Design note 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum KnowledgeLevel {
    /// A bearing, a flux and a band: something is there.
    #[default]
    Bearing,
    /// The host is identified: its ID, the light's age, the apparent and the extrapolated present
    /// position.
    Resolved,
}

/// One reception of a contact's light: where and when the observer received it, when it left, the
/// bearing it came from, and its flux in one band (Design note 10).
///
/// It keeps the apparent position (where the light left the host) and the position extrapolated
/// to the observer's present, which only a resolved view shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sighting {
    observer: GalacticPosition,
    observed: UniverseTime,
    emitted: UniverseTime,
    apparent_position: GalacticPosition,
    present_position: GalacticPosition,
    bearing: Option<Bearing>,
    flux: WattsPerSquareMetre,
    band: AlertBand,
    level: KnowledgeLevel,
}

impl Sighting {
    /// The sighting of a host whose light `retardation` describes, received by `observer` with
    /// `flux` in `band`; `level` is what this one sighting supports, [`KnowledgeLevel::Resolved`]
    /// when the flux reaches the sensor's resolve limit.
    ///
    /// The present position is the observed one extrapolated to the observer's time
    /// ([`extrapolate_to_present`]), which for a drifting host lands on the star.
    ///
    /// # Panics
    ///
    /// - If `flux` is negative or not finite, which no flux the sim computes is.
    /// - In debug builds, if `retardation` is not for `observer`'s time.
    #[must_use]
    pub fn new(
        observer: &Observer,
        retardation: &Retardation,
        band: AlertBand,
        flux: WattsPerSquareMetre,
        level: KnowledgeLevel,
    ) -> Self {
        debug_assert_eq!(
            retardation.observed(),
            observer.time(),
            "the retardation is not for the observer's time"
        );
        Self::from_parts(SightingParts {
            observer: *observer.position(),
            observed: observer.time(),
            emitted: retardation.emitted(),
            apparent_position: *retardation.apparent_position(),
            present_position: extrapolate_to_present(retardation, observer.time()),
            flux,
            band,
            level,
        })
    }

    /// The sighting from its stored parts; the bearing is computed again from the observer and
    /// the apparent position, which gives the same bits.
    ///
    /// # Panics
    ///
    /// If the flux is negative or not finite.
    #[must_use]
    pub(crate) fn from_parts(parts: SightingParts) -> Self {
        assert!(
            parts.flux.value().is_finite() && parts.flux.value() >= 0.0,
            "a sighting's flux must be finite and non-negative, not {} W m⁻²",
            parts.flux.value()
        );
        Self {
            observer: parts.observer,
            observed: parts.observed,
            emitted: parts.emitted,
            apparent_position: parts.apparent_position,
            present_position: parts.present_position,
            bearing: bearing(&parts.observer, &parts.apparent_position),
            flux: parts.flux,
            band: parts.band,
            level: parts.level,
        }
    }

    /// The parts that are stored.
    #[must_use]
    pub(crate) fn parts(&self) -> SightingParts {
        SightingParts {
            observer: self.observer,
            observed: self.observed,
            emitted: self.emitted,
            apparent_position: self.apparent_position,
            present_position: self.present_position,
            flux: self.flux,
            band: self.band,
            level: self.level,
        }
    }

    /// Where the observer was.
    #[must_use]
    pub const fn observer(&self) -> &GalacticPosition {
        &self.observer
    }

    /// The observer's time: when the light arrived.
    #[must_use]
    pub const fn observed(&self) -> UniverseTime {
        self.observed
    }

    /// When the light left the host.
    #[must_use]
    pub const fn emitted(&self) -> UniverseTime {
        self.emitted
    }

    /// The direction the light came from at the observer, or `None` if the observer stood at the
    /// apparent position itself.
    #[must_use]
    pub const fn bearing(&self) -> Option<Bearing> {
        self.bearing
    }

    /// The flux at the observer in [`band`](Self::band), W m⁻².
    #[must_use]
    pub const fn flux(&self) -> WattsPerSquareMetre {
        self.flux
    }

    /// The band the flux is measured in.
    #[must_use]
    pub const fn band(&self) -> AlertBand {
        self.band
    }

    /// The level this sighting supports on its own.
    #[must_use]
    pub const fn level(&self) -> KnowledgeLevel {
        self.level
    }
}

/// A [`Sighting`]'s stored fields, for the store and its file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SightingParts {
    pub(crate) observer: GalacticPosition,
    pub(crate) observed: UniverseTime,
    pub(crate) emitted: UniverseTime,
    pub(crate) apparent_position: GalacticPosition,
    pub(crate) present_position: GalacticPosition,
    pub(crate) flux: WattsPerSquareMetre,
    pub(crate) band: AlertBand,
    pub(crate) level: KnowledgeLevel,
}

/// Everything the server knows of one contact, keyed by its event inside the server: the event,
/// every sighting in the order it was recorded, the highest level reached and whether it has been
/// acknowledged (Design note 10).
///
/// Nothing of it leaves the server except through
/// [`KnowledgeStore::view`](super::KnowledgeStore::view), which degrades it to its level, and its
/// `Debug` form leaves the event out, so that a log line cannot name an unresolved host.
#[derive(Clone, PartialEq)]
pub(crate) struct ContactRecord {
    contact: ContactId,
    event: EventId,
    sightings: Vec<Sighting>,
    level: KnowledgeLevel,
    acknowledged: Acknowledgement,
}

/// Whether the crew has acknowledged a contact. An acknowledged contact is still listed (Design
/// note 11: never hidden by acknowledgement).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Acknowledgement {
    /// Not yet acknowledged.
    #[default]
    Pending,
    /// Acknowledged.
    Acknowledged,
}

impl fmt::Debug for ContactRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContactRecord")
            .field("contact", &self.contact)
            .field("level", &self.level)
            .field("sightings", &self.sightings.len())
            .field("acknowledged", &self.acknowledged)
            .finish_non_exhaustive()
    }
}

impl ContactRecord {
    /// A record of `event` under `contact`, with its first sighting.
    #[must_use]
    pub(crate) fn new(contact: ContactId, event: EventId, first: Sighting) -> Self {
        Self {
            contact,
            event,
            level: first.level(),
            sightings: vec![first],
            acknowledged: Acknowledgement::Pending,
        }
    }

    /// Adds a sighting; the level rises to the sighting's if that is higher, and never falls.
    pub(crate) fn add(&mut self, sighting: Sighting) {
        self.level = self.level.max(sighting.level());
        self.sightings.push(sighting);
    }

    /// Marks the record acknowledged, returning whether it was not already.
    pub(crate) fn acknowledge(&mut self) -> bool {
        let changed = self.acknowledged == Acknowledgement::Pending;
        self.acknowledged = Acknowledgement::Acknowledged;
        changed
    }

    /// The contact's identifier.
    #[must_use]
    pub(crate) const fn contact(&self) -> ContactId {
        self.contact
    }

    /// The highest level reached.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn level(&self) -> KnowledgeLevel {
        self.level
    }

    /// The degraded form of the record for its level (Design note 10).
    ///
    /// The latest sighting is the one received last on the observer's clock, the one recorded
    /// last among those received at the same time; the bearing, flux and band, and for a resolved
    /// contact the light's age and both positions, are that sighting's.
    #[must_use]
    pub(crate) fn view(&self) -> ContactView {
        let first = self
            .sightings
            .iter()
            .map(Sighting::observed)
            .min()
            .expect("a record holds at least its first sighting");
        // `max_by_key` keeps the last of equal keys, which is the one recorded last.
        let latest = self
            .sightings
            .iter()
            .max_by_key(|sighting| sighting.observed())
            .expect("a record holds at least its first sighting");
        let seen = BearingContact {
            contact: self.contact,
            first_seen: first,
            last_seen: latest.observed(),
            bearing: latest.bearing(),
            flux: latest.flux(),
            band: latest.band(),
            acknowledged: self.acknowledged,
            sightings: self.sightings.len(),
        };
        match self.level {
            KnowledgeLevel::Bearing => ContactView::Bearing(seen),
            KnowledgeLevel::Resolved => ContactView::Resolved(ResolvedContact {
                seen,
                host: self.event.subject().system(),
                light_age: latest
                    .observed()
                    .checked_since(latest.emitted())
                    .expect("a light age inside the clock's range fits a span"),
                apparent_position: latest.apparent_position,
                present_position: latest.present_position,
            }),
        }
    }
}

/// A contact as the wire may carry it: degraded to the level reached (Design note 10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContactView {
    /// Only a bearing, a flux and a band: no host, distance, light age or position.
    Bearing(BearingContact),
    /// The host identified, with the light's age and the apparent and present positions.
    Resolved(ResolvedContact),
}

impl ContactView {
    /// What every level shows.
    #[must_use]
    pub const fn seen(&self) -> &BearingContact {
        match self {
            Self::Bearing(seen) => seen,
            Self::Resolved(resolved) => &resolved.seen,
        }
    }

    /// The level reached.
    #[must_use]
    pub const fn level(&self) -> KnowledgeLevel {
        match self {
            Self::Bearing(_) => KnowledgeLevel::Bearing,
            Self::Resolved(_) => KnowledgeLevel::Resolved,
        }
    }
}

/// What a contact at any level shows: when it was seen, where from, how bright, in which band.
///
/// It holds nothing that names or places the host.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BearingContact {
    contact: ContactId,
    first_seen: UniverseTime,
    last_seen: UniverseTime,
    bearing: Option<Bearing>,
    flux: WattsPerSquareMetre,
    band: AlertBand,
    acknowledged: Acknowledgement,
    sightings: usize,
}

impl BearingContact {
    /// The contact's identifier.
    #[must_use]
    pub const fn contact(&self) -> ContactId {
        self.contact
    }

    /// When its light first arrived, on the observer's clock.
    #[must_use]
    pub const fn first_seen(&self) -> UniverseTime {
        self.first_seen
    }

    /// When its latest sighting arrived, on the observer's clock.
    #[must_use]
    pub const fn last_seen(&self) -> UniverseTime {
        self.last_seen
    }

    /// The latest sighting's bearing, or `None` if its observer stood at the apparent position.
    #[must_use]
    pub const fn bearing(&self) -> Option<Bearing> {
        self.bearing
    }

    /// The latest sighting's flux, W m⁻², in [`band`](Self::band).
    #[must_use]
    pub const fn flux(&self) -> WattsPerSquareMetre {
        self.flux
    }

    /// The latest sighting's band.
    #[must_use]
    pub const fn band(&self) -> AlertBand {
        self.band
    }

    /// Whether the crew has acknowledged it.
    #[must_use]
    pub const fn acknowledged(&self) -> Acknowledgement {
        self.acknowledged
    }

    /// How many sightings the contact has.
    #[must_use]
    pub const fn sightings(&self) -> usize {
        self.sightings
    }
}

/// A resolved contact: what a bearing shows, and the host with its light's age and positions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedContact {
    seen: BearingContact,
    host: SystemId,
    light_age: Span,
    apparent_position: GalacticPosition,
    present_position: GalacticPosition,
}

impl ResolvedContact {
    /// What every level shows.
    #[must_use]
    pub const fn seen(&self) -> &BearingContact {
        &self.seen
    }

    /// The host system.
    #[must_use]
    pub const fn host(&self) -> SystemId {
        self.host
    }

    /// The latest sighting's light age: its arrival less its emission.
    #[must_use]
    pub const fn light_age(&self) -> Span {
        self.light_age
    }

    /// Where the latest sighting's light left the host, in the galactic frame.
    #[must_use]
    pub const fn apparent_position(&self) -> &GalacticPosition {
        &self.apparent_position
    }

    /// The host's position at the latest sighting's arrival, extrapolated from the observed
    /// position and velocity.
    #[must_use]
    pub const fn present_position(&self) -> &GalacticPosition {
        &self.present_position
    }
}
