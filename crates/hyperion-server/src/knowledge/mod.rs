//! Knowledge: what the crew has detected, separate from what is true, and as old as the light that
//! brought it (plan 12, P12.T7; brainstorm, "Overlays and persistence"; Design note 10).
//!
//! Only what alerts need is built. A contact is one transient event: the server keys its record
//! by the event's [`EventId`](hyperion_sim::id::EventId), which names the host,
//! and the wire by an opaque, sequential [`ContactId`], so that an unresolved contact cannot leak
//! its host through its identifier. A record holds every [`Sighting`] and the highest
//! [`KnowledgeLevel`] reached, [`Bearing`](KnowledgeLevel::Bearing) or
//! [`Resolved`](KnowledgeLevel::Resolved), which never falls. [`KnowledgeStore::view`] is the only
//! way out, and a bearing's view holds no host, distance, light age or position.
//!
//! The store is per universe until sessions exist, and [`PersistedKnowledge`] keeps it in versioned
//! JSON lines in the universe's reserved `knowledge/` directory. Beside it, [`surveys`] keeps what
//! the ship has surveyed of each body's surface (plan R09, R09.T18): survey passes in a log of
//! their own, folded on load into each body's coverage. Nothing else of the brainstorm's Knowledge
//! overlay is built: no degraded body records, no per-console views, no sensor model.

mod jsonl;
mod persist;
mod record;
mod store;
pub mod surveys;
#[cfg(test)]
mod testing;

pub use persist::{KNOWLEDGE_FORMAT, LoadKnowledgeError, PersistedKnowledge, SaveKnowledgeError};
pub use record::{
    Acknowledgement, BearingContact, ContactId, ContactView, KnowledgeLevel, ResolvedContact,
    Sighting,
};
pub use store::{AcknowledgeContactError, KnowledgeStore};
