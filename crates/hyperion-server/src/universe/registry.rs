//! The registry: every universe the server knows, created, listed and opened under one lock.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use hyperion_sim::{GENERATOR_VERSION, GeneratorVersion};

use super::{
    Compatibility, DrawEntropyError, Entropy, ParseUniverseNameError, SAVE_FORMAT, SavedUniverse,
    ScanStoreError, Universe, UniverseId, UniverseName, UniverseStore, WriteSaveError,
};
use crate::limits::MAX_UNIVERSES;

/// How many IDs one create draws before giving up, should the entropy source keep returning IDs
/// that are taken. With [`OsEntropy`](super::OsEntropy) a second draw is already a 2⁻⁵⁶ event.
pub const MAX_ID_DRAWS: usize = 16;

/// Every universe the server knows, and the one way to create more.
///
/// Cheap to clone: clones share one registry. All file I/O runs under
/// [`tokio::task::spawn_blocking`], and no lock is held across an `.await`.
#[derive(Debug, Clone)]
pub struct UniverseRegistry {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    store: UniverseStore,
    entropy: Arc<dyn Entropy>,
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    /// Every loaded or created universe.
    universes: BTreeMap<UniverseId, Arc<Universe>>,
    /// Saves in a later format, by ID, with the format they declare.
    unsupported: BTreeMap<UniverseId, u32>,
    /// The folded names of every universe in `universes` and of every create in progress.
    names: HashSet<String>,
    /// The IDs of creates in progress, between the draw and the write.
    pending: BTreeSet<UniverseId>,
    /// Creates in progress, from the name's reservation on, each holding a place under
    /// [`MAX_UNIVERSES`].
    creating: usize,
}

impl UniverseRegistry {
    /// Scans the store once and holds what it found.
    ///
    /// # Errors
    ///
    /// [`LoadRegistryError::Scan`] if the store cannot be listed, and
    /// [`LoadRegistryError::Interrupted`] if the scan's blocking task did not finish.
    pub async fn load(
        store: UniverseStore,
        entropy: Arc<dyn Entropy>,
    ) -> Result<Self, LoadRegistryError> {
        let scan = {
            let store = store.clone();
            tokio::task::spawn_blocking(move || store.scan())
                .await
                .map_err(|_| LoadRegistryError::Interrupted)?
                .map_err(LoadRegistryError::Scan)?
        };
        let (saves, unsupported) = scan.into_parts();
        let mut state = State::default();
        for save in saves {
            let universe = Universe::from(save);
            if !state.names.insert(universe.name.folded()) {
                tracing::warn!(
                    id = %universe.id,
                    name = %universe.name,
                    "two saves share a name; both are loaded"
                );
            }
            state.universes.insert(universe.id, Arc::new(universe));
        }
        state.unsupported = unsupported
            .into_iter()
            .map(|save| (save.id(), save.format()))
            .collect();
        tracing::info!(
            universes = state.universes.len(),
            unsupported = state.unsupported.len(),
            data_dir = %store.data_dir().display(),
            "loaded universes"
        );
        Ok(Self {
            inner: Arc::new(Inner {
                store,
                entropy,
                state: Mutex::new(state),
            }),
        })
    }

    /// Creates and saves a universe.
    ///
    /// The name is trimmed and checked, then reserved; `seed` is kept if given and otherwise
    /// drawn from the entropy source, and then the ID is drawn, in that order. An ID that is
    /// taken, in the registry or on disk, is drawn again. The universe is listed once its save is
    /// written; if the write fails the name is free again.
    ///
    /// Cancellation-safe: the work runs on a blocking task that finishes, and updates the
    /// registry, even if this future is dropped.
    ///
    /// # Errors
    ///
    /// [`CreateUniverseError`]: an invalid name, a name in use (compared without regard to case),
    /// the [`MAX_UNIVERSES`] limit, a failed draw or write, or no free ID after
    /// [`MAX_ID_DRAWS`] draws.
    pub async fn create(
        &self,
        name: &str,
        seed: Option<u64>,
    ) -> Result<Arc<Universe>, CreateUniverseError> {
        let name: UniverseName = name.parse().map_err(CreateUniverseError::InvalidName)?;
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || inner.create(&name, seed))
            .await
            .map_err(|_| CreateUniverseError::Interrupted)?
    }

    /// Every universe, sorted by name without regard to case, then by ID.
    #[must_use]
    pub fn list(&self) -> Vec<Arc<Universe>> {
        let mut universes: Vec<_> = self.inner.lock().universes.values().cloned().collect();
        universes.sort_by_cached_key(|universe| (universe.name.folded(), universe.id));
        universes
    }

    /// The universe `id`, if this server can open it.
    ///
    /// # Errors
    ///
    /// [`OpenUniverseError::UnknownUniverse`] if there is no such save,
    /// [`OpenUniverseError::GeneratorVersionMismatch`] if it was made under another generator
    /// version, and [`OpenUniverseError::UnsupportedSaveFormat`] if it is in a later format.
    pub fn open(&self, id: UniverseId) -> Result<Arc<Universe>, OpenUniverseError> {
        let state = self.inner.lock();
        if let Some(universe) = state.universes.get(&id) {
            return match universe.compatibility() {
                Compatibility::Compatible => Ok(Arc::clone(universe)),
                Compatibility::GeneratorMismatch => {
                    Err(OpenUniverseError::GeneratorVersionMismatch {
                        id,
                        saved: universe.generator_version,
                        server: GENERATOR_VERSION,
                    })
                }
            };
        }
        match state.unsupported.get(&id) {
            Some(&format) => Err(OpenUniverseError::UnsupportedSaveFormat { id, format }),
            None => Err(OpenUniverseError::UnknownUniverse { id }),
        }
    }

    /// The store the registry reads and writes.
    #[must_use]
    pub fn store(&self) -> &UniverseStore {
        &self.inner.store
    }
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        // Every critical section is a handful of map operations that leave the state whole, so a
        // poisoned lock still guards consistent state.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The blocking body of [`UniverseRegistry::create`].
    fn create(
        &self,
        name: &UniverseName,
        seed: Option<u64>,
    ) -> Result<Arc<Universe>, CreateUniverseError> {
        let mut reservation = self.reserve_name(name)?;
        let seed = match seed {
            Some(seed) => seed,
            None => self
                .entropy
                .draw_u64()
                .map_err(CreateUniverseError::Entropy)?,
        };
        for _ in 0..MAX_ID_DRAWS {
            let id = UniverseId::new(
                self.entropy
                    .draw_u64()
                    .map_err(CreateUniverseError::Entropy)?,
            );
            if !reservation.reserve_id(id) {
                continue;
            }
            let saved = SavedUniverse::new(id, name.clone(), seed, GENERATOR_VERSION);
            match self.store.write(&saved) {
                Ok(()) => return Ok(reservation.commit(Universe::from(saved))),
                Err(WriteSaveError::AlreadyExists { .. }) => {
                    // A directory the scan skipped holds this ID. Leave it alone and draw again.
                    reservation.release_id();
                }
                Err(error) => return Err(CreateUniverseError::Storage(error)),
            }
        }
        Err(CreateUniverseError::NoFreeId)
    }

    /// Checks the limit and reserves the name, both under one lock.
    fn reserve_name(&self, name: &UniverseName) -> Result<Reservation<'_>, CreateUniverseError> {
        let folded = name.folded();
        let mut state = self.lock();
        if state.names.contains(&folded) {
            return Err(CreateUniverseError::NameTaken {
                name: name.as_str().to_owned(),
            });
        }
        if state.universes.len() + state.creating >= MAX_UNIVERSES {
            return Err(CreateUniverseError::LimitReached {
                limit: MAX_UNIVERSES,
            });
        }
        state.names.insert(folded.clone());
        state.creating += 1;
        Ok(Reservation {
            inner: self,
            folded: Some(folded),
            id: None,
        })
    }
}

/// A name, a place under the limit, and then an ID, held for a create in progress. Dropping it
/// releases them, so that every way out of a create other than [`Reservation::commit`], a panic
/// included, frees them.
struct Reservation<'a> {
    inner: &'a Inner,
    folded: Option<String>,
    id: Option<UniverseId>,
}

impl Reservation<'_> {
    /// Reserves `id` unless a universe, a later-format save or another create holds it.
    fn reserve_id(&mut self, id: UniverseId) -> bool {
        let mut state = self.inner.lock();
        let taken = state.universes.contains_key(&id)
            || state.unsupported.contains_key(&id)
            || !state.pending.insert(id);
        if !taken {
            self.id = Some(id);
        }
        !taken
    }

    /// Releases the reserved ID, keeping the name.
    fn release_id(&mut self) {
        if let Some(id) = self.id.take() {
            self.inner.lock().pending.remove(&id);
        }
    }

    /// Lists the universe and turns the reservation into its entry.
    fn commit(mut self, universe: Universe) -> Arc<Universe> {
        let universe = Arc::new(universe);
        let mut state = self.inner.lock();
        state.pending.remove(&universe.id);
        state.universes.insert(universe.id, Arc::clone(&universe));
        // The place is now the listed universe's.
        state.creating -= 1;
        // The name stays in `names`, now for the listed universe.
        self.folded = None;
        self.id = None;
        universe
    }
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        if self.folded.is_none() && self.id.is_none() {
            return;
        }
        let mut state = self.inner.lock();
        if let Some(folded) = self.folded.take() {
            state.names.remove(&folded);
            state.creating -= 1;
        }
        if let Some(id) = self.id.take() {
            state.pending.remove(&id);
        }
    }
}

/// The registry could not be loaded.
#[derive(Debug)]
pub enum LoadRegistryError {
    /// The store could not be listed.
    Scan(ScanStoreError),
    /// The scan's blocking task panicked or was cancelled by the runtime shutting down.
    Interrupted,
}

impl fmt::Display for LoadRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scan(_) => f.write_str("failed to scan the saved universes"),
            Self::Interrupted => f.write_str("the scan of the saved universes was interrupted"),
        }
    }
}

impl Error for LoadRegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scan(error) => Some(error),
            Self::Interrupted => None,
        }
    }
}

/// A universe could not be created.
#[derive(Debug)]
pub enum CreateUniverseError {
    /// The name broke a rule of [`UniverseName`].
    InvalidName(ParseUniverseNameError),
    /// Another universe has this name, compared without regard to case.
    NameTaken {
        /// The name asked for, trimmed.
        name: String,
    },
    /// The server holds [`MAX_UNIVERSES`] universes already.
    LimitReached {
        /// The limit.
        limit: usize,
    },
    /// A seed or an ID could not be drawn.
    Entropy(DrawEntropyError),
    /// The save could not be written; nothing was listed.
    Storage(WriteSaveError),
    /// Every ID drawn was taken.
    NoFreeId,
    /// The blocking task panicked or was cancelled by the runtime shutting down.
    Interrupted,
}

impl fmt::Display for CreateUniverseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(error) => write!(f, "invalid universe name: {error}"),
            Self::NameTaken { name } => write!(f, "a universe named {name:?} exists already"),
            Self::LimitReached { limit } => {
                write!(f, "the server holds its limit of {limit} universes")
            }
            Self::Entropy(_) => f.write_str("failed to draw a random value"),
            Self::Storage(_) => f.write_str("failed to save the universe"),
            Self::NoFreeId => write!(f, "no free universe id in {MAX_ID_DRAWS} draws"),
            Self::Interrupted => f.write_str("the create was interrupted"),
        }
    }
}

impl Error for CreateUniverseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Entropy(error) => Some(error),
            Self::Storage(error) => Some(error),
            Self::InvalidName(_)
            | Self::NameTaken { .. }
            | Self::LimitReached { .. }
            | Self::NoFreeId
            | Self::Interrupted => None,
        }
    }
}

/// A universe could not be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpenUniverseError {
    /// No save has this ID.
    UnknownUniverse {
        /// The ID asked for.
        id: UniverseId,
    },
    /// The save was made under another generator version than this server's.
    GeneratorVersionMismatch {
        /// The universe.
        id: UniverseId,
        /// The version in the save.
        saved: GeneratorVersion,
        /// The version this server runs.
        server: GeneratorVersion,
    },
    /// The save is in a format this server cannot read.
    UnsupportedSaveFormat {
        /// The universe.
        id: UniverseId,
        /// The format the save declares.
        format: u32,
    },
}

impl fmt::Display for OpenUniverseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownUniverse { id } => write!(f, "no universe has id {id}"),
            Self::GeneratorVersionMismatch { id, saved, server } => write!(
                f,
                "universe {id} was created with generator version {saved}, \
                 and this server runs generator version {server}"
            ),
            Self::UnsupportedSaveFormat { id, format } => write!(
                f,
                "universe {id} is saved in format {format}, \
                 and this server reads format {SAVE_FORMAT}"
            ),
        }
    }
}

impl Error for OpenUniverseError {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::mpsc;
    use std::time::Duration;

    use tokio::sync::oneshot;
    use tokio::time::timeout;

    use super::*;
    use crate::universe::SequenceEntropy;

    /// Upper bound on any wait, so that a hung create fails the test instead of the suite.
    const WAIT: Duration = Duration::from_secs(10);

    async fn load(dir: &Path, entropy: impl Entropy + 'static) -> UniverseRegistry {
        timeout(
            WAIT,
            UniverseRegistry::load(UniverseStore::new(dir), Arc::new(entropy)),
        )
        .await
        .expect("timed out loading")
        .unwrap()
    }

    async fn create(
        registry: &UniverseRegistry,
        name: &str,
        seed: Option<u64>,
    ) -> Result<Arc<Universe>, CreateUniverseError> {
        timeout(WAIT, registry.create(name, seed))
            .await
            .expect("timed out creating")
    }

    /// Writes a save file by hand, without syncing, as an operator or another build might.
    fn plant(dir: &Path, id: u64, name: &str, generator_version: u32) {
        let dir = dir.join("universes").join(format!("{id:016x}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("universe.json"),
            format!(
                r#"{{"format":1,"id":"{id:016x}","name":"{name}","seed":"0000000000000001","generator_version":{generator_version}}}"#
            ),
        )
        .unwrap();
    }

    fn names(registry: &UniverseRegistry) -> Vec<String> {
        registry
            .list()
            .iter()
            .map(|universe| universe.name().as_str().to_owned())
            .collect()
    }

    /// Entropy whose first draw signals `entered` and then blocks until the test opens the gate,
    /// so that a test can act while a create holds its reservation.
    #[derive(Debug)]
    struct GatedEntropy {
        entered: Mutex<Option<oneshot::Sender<()>>>,
        gate: Mutex<mpsc::Receiver<()>>,
        values: SequenceEntropy,
    }

    impl GatedEntropy {
        fn new(
            values: impl IntoIterator<Item = u64>,
        ) -> (Self, oneshot::Receiver<()>, mpsc::Sender<()>) {
            let (entered_tx, entered_rx) = oneshot::channel();
            let (gate_tx, gate_rx) = mpsc::channel();
            let entropy = Self {
                entered: Mutex::new(Some(entered_tx)),
                gate: Mutex::new(gate_rx),
                values: SequenceEntropy::new(values),
            };
            (entropy, entered_rx, gate_tx)
        }
    }

    impl Entropy for GatedEntropy {
        fn draw_u64(&self) -> Result<u64, DrawEntropyError> {
            let entered = self.entered.lock().unwrap().take();
            if let Some(entered) = entered {
                entered.send(()).unwrap();
                // Blocks the create's blocking thread, never the runtime.
                self.gate.lock().unwrap().recv().unwrap();
            }
            self.values.draw_u64()
        }
    }

    #[tokio::test]
    async fn names_are_checked_before_anything_is_drawn() {
        let dir = tempfile::tempdir().unwrap();
        let registry = load(dir.path(), SequenceEntropy::new([0xab])).await;
        let invalid = |error| match error {
            CreateUniverseError::InvalidName(error) => error,
            other => panic!("expected an invalid name, got {other:?}"),
        };
        assert_eq!(
            invalid(create(&registry, "  ", None).await.unwrap_err()),
            ParseUniverseNameError::Empty
        );
        assert_eq!(
            invalid(create(&registry, &"a".repeat(49), None).await.unwrap_err()),
            ParseUniverseNameError::TooLong { chars: 49 }
        );
        assert_eq!(
            invalid(create(&registry, "Kepler\tReach", None).await.unwrap_err()),
            ParseUniverseNameError::ControlCharacter
        );
        let kepler = create(&registry, " Kepler Reach ", Some(7)).await.unwrap();
        assert_eq!(kepler.name().as_str(), "Kepler Reach");
        assert_eq!(
            kepler.id(),
            UniverseId::new(0xab),
            "the refused creates drew nothing"
        );
        match create(&registry, "KEPLER reach", None).await.unwrap_err() {
            CreateUniverseError::NameTaken { name } => assert_eq!(name, "KEPLER reach"),
            other => panic!("expected a taken name, got {other:?}"),
        }
        assert_eq!(names(&registry), ["Kepler Reach"]);
        assert_eq!(
            fs::read_dir(dir.path().join("universes")).unwrap().count(),
            1,
            "only the valid create wrote a save"
        );
    }

    #[tokio::test]
    async fn a_given_seed_is_kept_and_a_missing_one_is_drawn_before_the_id() {
        let dir = tempfile::tempdir().unwrap();
        let registry = load(dir.path(), SequenceEntropy::new([0x11, 0x22, 0x33])).await;
        let given = create(&registry, "Given", Some(1234)).await.unwrap();
        assert_eq!(given.seed(), 1234);
        assert_eq!(given.id(), UniverseId::new(0x11));
        assert_eq!(given.generator_version(), GENERATOR_VERSION);
        assert_eq!(given.compatibility(), Compatibility::Compatible);
        let drawn = create(&registry, "Drawn", None).await.unwrap();
        assert_eq!(drawn.seed(), 0x22);
        assert_eq!(drawn.id(), UniverseId::new(0x33));
        assert_eq!(
            drawn.key(),
            crate::compute::GalaxyKey::new(0x22, GENERATOR_VERSION)
        );
    }

    #[tokio::test]
    async fn the_universe_beyond_the_limit_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        for id in 1..u64::try_from(MAX_UNIVERSES).unwrap() {
            plant(
                dir.path(),
                id,
                &format!("Planted {id}"),
                GENERATOR_VERSION.get(),
            );
        }
        let registry = load(dir.path(), SequenceEntropy::new([0x1000, 0x1001])).await;
        assert_eq!(registry.list().len(), MAX_UNIVERSES - 1);
        create(&registry, "Last", Some(1)).await.unwrap();
        match create(&registry, "One too many", Some(1))
            .await
            .unwrap_err()
        {
            CreateUniverseError::LimitReached { limit } => assert_eq!(limit, MAX_UNIVERSES),
            other => panic!("expected the limit, got {other:?}"),
        }
        assert_eq!(registry.list().len(), MAX_UNIVERSES);
        assert_eq!(
            fs::read_dir(dir.path().join("universes")).unwrap().count(),
            MAX_UNIVERSES
        );
    }

    #[tokio::test]
    async fn a_create_in_progress_counts_towards_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        for id in 1..u64::try_from(MAX_UNIVERSES).unwrap() {
            plant(
                dir.path(),
                id,
                &format!("Planted {id}"),
                GENERATOR_VERSION.get(),
            );
        }
        // The first draw, the seed of the first create, waits for the gate.
        let (entropy, entered, gate) = GatedEntropy::new([0x1000, 0x1001, 0x1002, 0x1003]);
        let registry = load(dir.path(), entropy).await;
        let first = tokio::spawn({
            let registry = registry.clone();
            async move { registry.create("Last", None).await }
        });
        // The first create holds the last place, though it has drawn no ID yet.
        timeout(WAIT, entered).await.expect("timed out").unwrap();
        match create(&registry, "One too many", Some(1)).await {
            Err(CreateUniverseError::LimitReached { limit }) => assert_eq!(limit, MAX_UNIVERSES),
            other => panic!("expected the limit, got {other:?}"),
        }
        gate.send(()).unwrap();
        timeout(WAIT, first)
            .await
            .expect("timed out")
            .unwrap()
            .unwrap();
        assert_eq!(registry.list().len(), MAX_UNIVERSES);
        // A failed create gives its place back.
        assert!(matches!(
            create(&registry, "Later", Some(1)).await,
            Err(CreateUniverseError::LimitReached { .. })
        ));
        assert_eq!(registry.inner.lock().creating, 0);
    }

    #[tokio::test]
    async fn a_reloaded_registry_lists_the_same_universes() {
        let dir = tempfile::tempdir().unwrap();
        let registry = load(dir.path(), SequenceEntropy::new([0x30, 0x10, 0x20])).await;
        for name in ["zeta", "Alpha", "beta"] {
            create(&registry, name, Some(9)).await.unwrap();
        }
        assert_eq!(names(&registry), ["Alpha", "beta", "zeta"]);
        let reloaded = load(dir.path(), SequenceEntropy::new([])).await;
        assert_eq!(reloaded.list(), registry.list());
        assert_eq!(
            reloaded.open(UniverseId::new(0x10)).unwrap(),
            registry.open(UniverseId::new(0x10)).unwrap()
        );
    }

    #[tokio::test]
    async fn a_save_from_another_generator_version_is_listed_but_not_opened_or_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let other = GeneratorVersion::new(GENERATOR_VERSION.get() + 1);
        plant(dir.path(), 0xab, "Elsewhen", other.get());
        let store = UniverseStore::new(dir.path());
        let path = store.save_path(UniverseId::new(0xab));
        let before = fs::read(&path).unwrap();

        let registry = load(dir.path(), SequenceEntropy::new([0xcd])).await;
        let listed = registry.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].compatibility(), Compatibility::GeneratorMismatch);
        assert_eq!(listed[0].generator_version(), other);
        let error = registry.open(UniverseId::new(0xab)).unwrap_err();
        assert_eq!(
            error,
            OpenUniverseError::GeneratorVersionMismatch {
                id: UniverseId::new(0xab),
                saved: other,
                server: GENERATOR_VERSION,
            }
        );
        let message = error.to_string();
        assert!(
            message.contains(&other.to_string())
                && message.contains(&GENERATOR_VERSION.to_string()),
            "both versions are named: {message}"
        );

        create(&registry, "Now", Some(1)).await.unwrap();
        drop(registry);
        load(dir.path(), SequenceEntropy::new([])).await;
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[tokio::test]
    async fn a_save_in_a_later_format_refuses_to_open_and_keeps_its_id() {
        let dir = tempfile::tempdir().unwrap();
        let save_dir = dir.path().join("universes").join("00000000000000ab");
        fs::create_dir_all(&save_dir).unwrap();
        fs::write(save_dir.join("universe.json"), r#"{"format":2}"#).unwrap();
        let registry = load(dir.path(), SequenceEntropy::new([0xab, 0xcd])).await;
        assert!(registry.list().is_empty());
        assert_eq!(
            registry.open(UniverseId::new(0xab)),
            Err(OpenUniverseError::UnsupportedSaveFormat {
                id: UniverseId::new(0xab),
                format: 2,
            })
        );
        let created = create(&registry, "New", Some(1)).await.unwrap();
        assert_eq!(created.id(), UniverseId::new(0xcd), "0xab is drawn again");
    }

    #[tokio::test]
    async fn an_unknown_id_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let registry = load(dir.path(), SequenceEntropy::new([])).await;
        assert_eq!(
            registry.open(UniverseId::new(0x99)),
            Err(OpenUniverseError::UnknownUniverse {
                id: UniverseId::new(0x99)
            })
        );
    }

    #[tokio::test]
    async fn a_taken_id_is_drawn_again() {
        let dir = tempfile::tempdir().unwrap();
        // 0xee holds a directory the scan skips: its file is missing.
        fs::create_dir_all(dir.path().join("universes").join("00000000000000ee")).unwrap();
        let registry = load(dir.path(), SequenceEntropy::new([0xab, 0xab, 0xee, 0xcd])).await;
        create(&registry, "First", Some(1)).await.unwrap();
        let second = create(&registry, "Second", Some(1)).await.unwrap();
        assert_eq!(second.id(), UniverseId::new(0xcd));
        assert!(
            fs::read_dir(dir.path().join("universes").join("00000000000000ee"))
                .unwrap()
                .next()
                .is_none(),
            "the skipped directory is left alone"
        );
    }

    #[tokio::test]
    async fn a_create_gives_up_when_every_id_is_taken_and_frees_the_name() {
        let dir = tempfile::tempdir().unwrap();
        let taken = std::iter::repeat_n(0xab, MAX_ID_DRAWS + 1);
        let registry = load(dir.path(), SequenceEntropy::new(taken.chain([0xcd]))).await;
        create(&registry, "First", Some(1)).await.unwrap();
        assert!(matches!(
            create(&registry, "Second", Some(1)).await,
            Err(CreateUniverseError::NoFreeId)
        ));
        let second = create(&registry, "Second", Some(1)).await.unwrap();
        assert_eq!(second.id(), UniverseId::new(0xcd));
    }

    #[tokio::test]
    async fn a_failed_draw_or_write_frees_the_name() {
        let dir = tempfile::tempdir().unwrap();
        let registry = load(dir.path(), SequenceEntropy::new([0xab, 0xcd])).await;
        // A file where the directory of universes belongs makes the write fail.
        let universes = dir.path().join("universes");
        fs::write(&universes, "").unwrap();
        assert!(matches!(
            create(&registry, "Kepler", Some(1)).await,
            Err(CreateUniverseError::Storage(WriteSaveError::Io { .. }))
        ));
        assert!(registry.list().is_empty());
        fs::remove_file(&universes).unwrap();
        let kepler = create(&registry, "Kepler", Some(1)).await.unwrap();
        assert_eq!(kepler.id(), UniverseId::new(0xcd));
        // The sequence is spent: the draw fails, and the name is free again afterwards.
        assert!(matches!(
            create(&registry, "Reach", None).await,
            Err(CreateUniverseError::Entropy(DrawEntropyError::Exhausted))
        ));
        assert_eq!(registry.inner.lock().names.len(), 1);
        assert!(registry.inner.lock().pending.is_empty());
    }

    #[tokio::test]
    async fn a_name_is_held_while_its_create_is_in_progress() {
        let dir = tempfile::tempdir().unwrap();
        let (entropy, entered, gate) = GatedEntropy::new([0xab]);
        let registry = load(dir.path(), entropy).await;
        let first = tokio::spawn({
            let registry = registry.clone();
            async move { registry.create("Kepler", Some(1)).await }
        });
        // The first create has reserved the name and waits in its draw of the ID.
        timeout(WAIT, entered).await.expect("timed out").unwrap();
        assert!(matches!(
            create(&registry, "KEPLER", Some(2)).await,
            Err(CreateUniverseError::NameTaken { .. })
        ));
        gate.send(()).unwrap();
        let first = timeout(WAIT, first)
            .await
            .expect("timed out")
            .unwrap()
            .unwrap();
        assert_eq!(first.id(), UniverseId::new(0xab));
        assert_eq!(names(&registry), ["Kepler"]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_creates_of_one_name_yield_one_success() {
        let dir = tempfile::tempdir().unwrap();
        let registry = load(dir.path(), SequenceEntropy::new(1..=8)).await;
        let creates: Vec<_> = [
            "kepler", "Kepler", "KEPLER", "kEpLeR", "kepleR", "Kepler ", " kepler", "KEPler",
        ]
        .into_iter()
        .map(|name| {
            let registry = registry.clone();
            tokio::spawn(async move { registry.create(name, Some(1)).await })
        })
        .collect();
        let mut successes = 0;
        for create in creates {
            match timeout(WAIT, create).await.expect("timed out").unwrap() {
                Ok(_) => successes += 1,
                Err(CreateUniverseError::NameTaken { .. }) => {}
                Err(other) => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(successes, 1);
        assert_eq!(registry.list().len(), 1);
    }

    #[tokio::test]
    async fn a_create_whose_caller_leaves_still_completes() {
        let dir = tempfile::tempdir().unwrap();
        let (entropy, entered, gate) = GatedEntropy::new([0xab]);
        let registry = load(dir.path(), entropy).await;
        let caller = tokio::spawn({
            let registry = registry.clone();
            async move { registry.create("Kepler", Some(1)).await }
        });
        timeout(WAIT, entered).await.expect("timed out").unwrap();
        caller.abort();
        assert!(
            timeout(WAIT, caller)
                .await
                .expect("timed out")
                .unwrap_err()
                .is_cancelled()
        );
        gate.send(()).unwrap();
        // The blocking task finishes on its own; wait for it to list the universe.
        timeout(WAIT, async {
            while registry.list().is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the create never completed");
        assert_eq!(
            registry
                .open(UniverseId::new(0xab))
                .unwrap()
                .name()
                .as_str(),
            "Kepler"
        );
        assert!(registry.inner.lock().pending.is_empty());
    }
}
