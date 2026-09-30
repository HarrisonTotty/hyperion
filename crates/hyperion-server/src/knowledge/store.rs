//! One universe's contacts, in memory (plan 12, P12.T7.a).

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use hyperion_sim::id::EventId;

use super::persist::SaveKnowledgeError;
use super::record::{ContactId, ContactRecord, ContactView, Sighting};

/// What the crew has detected in one universe: a contact record per event, keyed by its
/// [`EventId`] inside and by a sequential [`ContactId`] outside (Design note 10).
///
/// [`view`](Self::view) is the only way out, and it degrades each record to its level, so no
/// caller can put a host on the wire before its contact is resolved. Levels never fall. The store
/// is per universe until sessions exist (plan 12's Risks), and [`PersistedKnowledge`] saves it.
///
/// [`PersistedKnowledge`]: super::PersistedKnowledge
///
/// Its `Debug` form lists the records without their events, which name the hosts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KnowledgeStore {
    /// The records, `records[i]` being contact `i + 1`.
    records: Vec<ContactRecord>,
    /// Each event's contact.
    by_event: EventIndex,
}

/// Each event's contact, whose `Debug` form counts them and names none.
#[derive(Clone, Default, PartialEq)]
struct EventIndex(BTreeMap<EventId, ContactId>);

impl fmt::Debug for EventIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EventIndex({} events)", self.0.len())
    }
}

/// A change to a store, as its file records it: each is applied in order to rebuild the store.
#[derive(Debug, Clone, Copy, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "an entry lives for one change and is never kept in bulk"
)]
pub(crate) enum KnowledgeEntry {
    /// A sighting of `event`, which is `contact`: a new contact when it is the next number.
    Sighting {
        contact: ContactId,
        event: EventId,
        sighting: Sighting,
    },
    /// The crew acknowledged `contact`.
    Acknowledged { contact: ContactId },
}

impl KnowledgeStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many contacts the store holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the store holds no contact.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Records a sighting of `event`, returning its contact: the event's own if it has been seen
    /// before, from wherever and whenever, and otherwise the next number. The contact's level rises
    /// to the sighting's if that is higher and never falls.
    ///
    /// # Panics
    ///
    /// If the universe already holds 2³² − 1 contacts.
    pub fn record_sighting(&mut self, event: EventId, sighting: Sighting) -> ContactId {
        let entry = self.sighting_entry(event, sighting);
        let contact = entry.contact();
        self.apply(entry)
            .expect("an entry planned by the store applies to it");
        contact
    }

    /// Marks `contact` acknowledged, returning whether it was not already. An acknowledged contact
    /// stays in the store and in [`contacts`](Self::contacts).
    ///
    /// # Errors
    ///
    /// [`AcknowledgeContactError::UnknownContact`] if the store has no such contact.
    pub fn acknowledge(&mut self, contact: ContactId) -> Result<bool, AcknowledgeContactError> {
        self.record_mut(contact)
            .map(ContactRecord::acknowledge)
            .ok_or(AcknowledgeContactError::UnknownContact(contact))
    }

    /// Every contact, in the order first seen, which is their numbers' order.
    pub fn contacts(&self) -> impl ExactSizeIterator<Item = ContactId> + '_ {
        self.records.iter().map(ContactRecord::contact)
    }

    /// What the wire may know of `contact`: its record degraded to the level reached, or `None`
    /// if the store has no such contact (Design note 10).
    #[must_use]
    pub fn view(&self, contact: ContactId) -> Option<ContactView> {
        self.record(contact).map(ContactRecord::view)
    }

    /// The entry that records a sighting of `event`: under the event's contact, or the next
    /// number for an event not seen before.
    ///
    /// # Panics
    ///
    /// If the universe already holds 2³² − 1 contacts.
    #[must_use]
    pub(crate) fn sighting_entry(&self, event: EventId, sighting: Sighting) -> KnowledgeEntry {
        let contact = self
            .by_event
            .0
            .get(&event)
            .copied()
            .unwrap_or_else(|| self.next_contact());
        KnowledgeEntry::Sighting {
            contact,
            event,
            sighting,
        }
    }

    /// Applies one change, as the store's own methods do and as a file is replayed.
    ///
    /// # Errors
    ///
    /// [`ApplyEntryError`] if the entry does not fit the store: a sighting under a contact that is
    /// neither its event's nor the next number, or an acknowledgement of a contact not yet seen.
    pub(crate) fn apply(&mut self, entry: KnowledgeEntry) -> Result<(), ApplyEntryError> {
        match entry {
            KnowledgeEntry::Sighting {
                contact,
                event,
                sighting,
            } => match self.by_event.0.get(&event).copied() {
                Some(known) if known == contact => {
                    self.record_mut(contact)
                        .expect("an event's contact has a record")
                        .add(sighting);
                    Ok(())
                }
                None if contact == self.next_contact() => {
                    self.by_event.0.insert(event, contact);
                    self.records
                        .push(ContactRecord::new(contact, event, sighting));
                    Ok(())
                }
                // A known event under another contact, or a new one under a number not next.
                Some(_) | None => Err(ApplyEntryError),
            },
            KnowledgeEntry::Acknowledged { contact } => self
                .record_mut(contact)
                .map(|record| {
                    record.acknowledge();
                })
                .ok_or(ApplyEntryError),
        }
    }

    /// The number the next new contact takes.
    ///
    /// # Panics
    ///
    /// If the universe already holds 2³² − 1 contacts.
    fn next_contact(&self) -> ContactId {
        self.records
            .last()
            .map_or(Some(ContactId::FIRST), |last| last.contact().next())
            .expect("a universe holds fewer than 2³² − 1 contacts")
    }

    fn index(contact: ContactId) -> usize {
        usize::try_from(contact.get() - 1).expect("a contact number fits in usize on every target")
    }

    fn record(&self, contact: ContactId) -> Option<&ContactRecord> {
        self.records.get(Self::index(contact))
    }

    fn record_mut(&mut self, contact: ContactId) -> Option<&mut ContactRecord> {
        self.records.get_mut(Self::index(contact))
    }

    /// The record of `contact`, for the tests of this module and the file's.
    #[cfg(test)]
    pub(crate) fn record_of(&self, contact: ContactId) -> Option<&ContactRecord> {
        self.record(contact)
    }
}

impl KnowledgeEntry {
    /// The contact the entry is about.
    #[must_use]
    pub(crate) const fn contact(&self) -> ContactId {
        match self {
            Self::Sighting { contact, .. } | Self::Acknowledged { contact } => *contact,
        }
    }
}

/// An entry that does not fit the store it is applied to: only a damaged file holds one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ApplyEntryError;

/// A contact could not be acknowledged.
#[derive(Debug)]
pub enum AcknowledgeContactError {
    /// The universe has no such contact.
    UnknownContact(ContactId),
    /// The acknowledgement could not be saved, and was not made. Only
    /// [`PersistedKnowledge::acknowledge`](super::PersistedKnowledge::acknowledge) saves.
    Save(SaveKnowledgeError),
}

impl fmt::Display for AcknowledgeContactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownContact(contact) => write!(f, "no {contact} is known"),
            Self::Save(_) => f.write_str("failed to save the acknowledgement"),
        }
    }
}

impl Error for AcknowledgeContactError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::UnknownContact(_) => None,
            Self::Save(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::alerts::AlertBand;
    use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
    use hyperion_sim::galaxy::gas::ccm::Band;
    use hyperion_sim::observe::{Drift, Observer, retarded};
    use hyperion_sim::time::UniverseTime;
    use hyperion_sim::units::WattsPerSquareMetre;

    use super::*;
    use crate::knowledge::record::{Acknowledgement, KnowledgeLevel};
    use crate::knowledge::testing::{event_of, host, other_host};

    /// A sighting of the host from `from` light-years, at `years`, with `flux` in V.
    fn sighting(from: [f64; 3], years: i64, flux: f64, level: KnowledgeLevel) -> Sighting {
        let star = Drift::new(
            GalacticPosition::from_light_years([100.0, 26_000.0, 5.0]).unwrap(),
            GalacticVelocity::new([12e3, -220e3, 7e3]),
        );
        let observer = Observer::new(
            GalacticPosition::from_light_years(from).unwrap(),
            UniverseTime::from_julian_years(years).unwrap(),
        )
        .unwrap();
        Sighting::new(
            &observer,
            &retarded(&observer, &star),
            AlertBand::Photometric(Band::V),
            WattsPerSquareMetre::new(flux),
            level,
        )
    }

    const HERE: [f64; 3] = [0.0, 25_000.0, 0.0];
    const THERE: [f64; 3] = [-3_000.0, 21_000.0, 40.0];

    /// P12.T7.a: a `Bearing` view has no host, distance, light age or position: its type holds
    /// none, and nothing in it names the host.
    #[test]
    fn knowledge_bearing_view_has_no_host_distance_light_age_or_position() {
        let mut store = KnowledgeStore::new();
        let contact = store.record_sighting(
            event_of(host()),
            sighting(HERE, 0, 1e-15, KnowledgeLevel::Bearing),
        );
        let view = store.view(contact).unwrap();
        let ContactView::Bearing(seen) = view else {
            panic!("an unresolved contact is viewed as a bearing: {view:?}");
        };
        assert_eq!(seen.contact(), contact);
        assert!(seen.bearing().is_some());
        let text = format!("{view:?}");
        let logged = format!("{store:?}");
        assert!(
            !logged.contains("SystemId") && !logged.contains("EventId"),
            "{logged}"
        );
        for word in [
            "host",
            "position",
            "light_age",
            "distance",
            "event",
            "System",
        ] {
            assert!(!text.contains(word), "`{word}` in {text}");
        }
        assert!(!text.contains(&format!("{:x}", host().raw())), "{text}");
        assert!(!text.contains(&host().raw().to_string()), "{text}");
    }

    /// P12.T7.a: two sightings of one event from different places share one contact, and other
    /// events take the next numbers.
    #[test]
    fn knowledge_two_sightings_of_one_event_from_different_places_share_one_contact() {
        let mut store = KnowledgeStore::new();
        let event = event_of(host());
        let first = store.record_sighting(event, sighting(HERE, 0, 1e-15, KnowledgeLevel::Bearing));
        let second =
            store.record_sighting(event, sighting(THERE, 40, 3e-16, KnowledgeLevel::Bearing));
        assert_eq!(first, second);
        assert_eq!(first, ContactId::new(1).unwrap());
        assert_eq!(store.len(), 1);
        let seen = *store.view(first).unwrap().seen();
        assert_eq!(seen.sightings(), 2);
        assert_eq!(seen.first_seen(), UniverseTime::EPOCH);
        assert_eq!(
            seen.last_seen(),
            UniverseTime::from_julian_years(40).unwrap()
        );
        // The latest sighting's flux and bearing.
        assert_eq!(seen.flux(), WattsPerSquareMetre::new(3e-16));
        assert_eq!(
            seen.bearing(),
            sighting(THERE, 40, 3e-16, KnowledgeLevel::Bearing).bearing()
        );

        let next = store.record_sighting(
            event_of(other_host()),
            sighting(HERE, 0, 1e-15, KnowledgeLevel::Bearing),
        );
        assert_eq!(next, ContactId::new(2).unwrap());
        assert_eq!(store.contacts().collect::<Vec<_>>(), [first, next]);
    }

    /// P12.T7.a: the level never falls. A resolving sighting resolves the contact; a later one
    /// under the resolve limit leaves it resolved, with the host, the light age and both
    /// positions of the latest sighting.
    #[test]
    fn knowledge_level_never_falls() {
        let mut store = KnowledgeStore::new();
        let event = event_of(host());
        let contact =
            store.record_sighting(event, sighting(THERE, 0, 1e-16, KnowledgeLevel::Bearing));
        assert_eq!(
            store.view(contact).unwrap().level(),
            KnowledgeLevel::Bearing
        );
        store.record_sighting(event, sighting(HERE, 10, 1e-13, KnowledgeLevel::Resolved));
        let faint = sighting(HERE, 20, 1e-16, KnowledgeLevel::Bearing);
        store.record_sighting(event, faint);
        let view = store.view(contact).unwrap();
        let ContactView::Resolved(resolved) = view else {
            panic!("a resolved contact stays resolved: {view:?}");
        };
        assert_eq!(resolved.host(), host());
        assert_eq!(resolved.seen().sightings(), 3);
        assert_eq!(
            resolved.light_age(),
            faint.observed().checked_since(faint.emitted()).unwrap()
        );
        // Some thousand years of light from about 1,000 ly.
        let age = resolved.light_age().as_julian_years_f64();
        assert!((900.0..1_200.0).contains(&age), "{age}");
        assert_eq!(
            *resolved.apparent_position(),
            faint.parts().apparent_position
        );
        assert_eq!(*resolved.present_position(), faint.parts().present_position);
        assert_eq!(
            store.record_of(contact).unwrap().level(),
            KnowledgeLevel::Resolved
        );
    }

    /// Acknowledging keeps the contact listed, is idempotent, and refuses a contact the store has
    /// not got.
    #[test]
    fn knowledge_acknowledgement_keeps_the_contact() {
        let mut store = KnowledgeStore::new();
        let contact = store.record_sighting(
            event_of(host()),
            sighting(HERE, 0, 1e-15, KnowledgeLevel::Bearing),
        );
        assert!(matches!(store.acknowledge(contact), Ok(true)));
        assert!(matches!(store.acknowledge(contact), Ok(false)));
        assert_eq!(
            store.view(contact).unwrap().seen().acknowledged(),
            Acknowledgement::Acknowledged
        );
        assert_eq!(store.contacts().len(), 1);
        let unknown = ContactId::new(7).unwrap();
        assert!(matches!(
            store.acknowledge(unknown),
            Err(AcknowledgeContactError::UnknownContact(id)) if id == unknown
        ));
        assert_eq!(store.view(unknown), None);
        let message = AcknowledgeContactError::UnknownContact(unknown).to_string();
        assert_eq!(message, "no contact 7 is known");
    }

    /// A replayed entry that does not fit is refused: a new event under a number that is not the
    /// next, a known event under another contact, or an acknowledgement of an unseen contact.
    #[test]
    fn knowledge_entries_that_do_not_fit_are_refused() {
        let mut store = KnowledgeStore::new();
        let event = event_of(host());
        let seen = sighting(HERE, 0, 1e-15, KnowledgeLevel::Bearing);
        let skip = KnowledgeEntry::Sighting {
            contact: ContactId::new(2).unwrap(),
            event,
            sighting: seen,
        };
        assert_eq!(store.apply(skip), Err(ApplyEntryError));
        let contact = store.record_sighting(event, seen);
        let other = KnowledgeEntry::Sighting {
            contact: ContactId::new(2).unwrap(),
            event,
            sighting: seen,
        };
        assert_eq!(store.apply(other), Err(ApplyEntryError));
        let unseen = KnowledgeEntry::Acknowledged {
            contact: ContactId::new(2).unwrap(),
        };
        assert_eq!(store.apply(unseen), Err(ApplyEntryError));
        assert_eq!(store.len(), 1);
        assert_eq!(store.view(contact).unwrap().seen().sightings(), 1);
    }
}
