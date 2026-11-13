//! Each universe's contacts, opened once and kept (plan 12, P12.T9.a): the one writer of a
//! universe's contacts file (P12.T7.b), which every alerts subscription of the universe shares.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use crate::compute::SingleFlight;
use crate::knowledge::{LoadKnowledgeError, PersistedKnowledge};
use crate::universe::{UniverseId, UniverseStore};

/// The contacts of every universe asked for so far, each opened once from its save.
pub(crate) struct UniverseContacts {
    held: Arc<Mutex<HashMap<UniverseId, PersistedKnowledge>>>,
    /// The opens in progress, so that two first subscriptions of a universe open it once.
    opening: SingleFlight<UniverseId, PersistedKnowledge, Arc<LoadKnowledgeError>>,
}

impl fmt::Debug for UniverseContacts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UniverseContacts")
            .field("universes", &lock(&self.held).len())
            .finish_non_exhaustive()
    }
}

impl UniverseContacts {
    /// No universe's contacts opened yet.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            held: Arc::new(Mutex::new(HashMap::new())),
            opening: SingleFlight::new(),
        }
    }

    /// Universe `universe`'s contacts, from `store`: opened on the first call and kept, so that
    /// its file has one writer.
    ///
    /// # Errors
    ///
    /// [`LoadKnowledgeError`], shared between the callers that waited on the open, as
    /// [`PersistedKnowledge::open`] gives it; nothing is kept, and the next call opens it again.
    pub(crate) async fn of(
        &self,
        store: &UniverseStore,
        universe: UniverseId,
    ) -> Result<PersistedKnowledge, Arc<LoadKnowledgeError>> {
        if let Some(contacts) = lock(&self.held).get(&universe) {
            return Ok(contacts.clone());
        }
        let (held, store) = (Arc::clone(&self.held), store.clone());
        let opened = self
            .opening
            .run(universe, move || async move {
                // A caller can arrive just after another open has finished and left the
                // registry, and the contacts it opened are the ones to share.
                if let Some(contacts) = lock(&held).get(&universe) {
                    return Ok(Arc::new(contacts.clone()));
                }
                let contacts = PersistedKnowledge::open(&store, universe)
                    .await
                    .map_err(Arc::new)?;
                let contacts = lock(&held).entry(universe).or_insert(contacts).clone();
                Ok(Arc::new(contacts))
            })
            .await?;
        Ok(PersistedKnowledge::clone(&opened))
    }
}

/// The map's lock, taken over a poisoning: the map is only ever read or added to whole.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
