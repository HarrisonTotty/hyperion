//! Universes: their identity, their saves, and the registry that creates, lists and opens them.
//!
//! A universe is `(seed, generator_version)` plus a save identity, [`UniverseId`], and an
//! operator-given name (plan 04, design note 16). Only that identity is persisted; everything
//! generated is recomputed on demand. A save made under another generator version is listed but
//! cannot be opened (design note 18): its file is never rewritten and nothing is regenerated under
//! the new version.
//!
//! Design note 20 describes a universe as immutable identity plus a lazily built, shared
//! `Arc<Galaxy>`. Here [`Universe`] is the identity alone. The galaxy is built and held by the
//! compute layer's galaxy cache (P04.T11, which needs plan 02's `Galaxy`), keyed by
//! [`Universe::key`], so that two saves of one seed share one galaxy.

mod entropy;
mod id;
mod name;
mod registry;
mod store;

use hyperion_sim::GeneratorVersion;

pub use entropy::{DrawEntropyError, Entropy, OsEntropy, OsRandomError, SequenceEntropy};
pub use id::{ParseHex64Error, UniverseId};
pub use name::{ParseUniverseNameError, UniverseName};
pub use registry::{
    CreateUniverseError, LoadRegistryError, MAX_ID_DRAWS, OpenUniverseError, UniverseRegistry,
};
pub use store::{
    SAVE_FORMAT, SavedUniverse, ScanStoreError, StoreScan, UniverseStore, UnsupportedSave,
    WriteSaveError,
};

use crate::compute::GalaxyKey;

/// Whether this server can open a universe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Compatibility {
    /// Saved under this server's generator version.
    Compatible,
    /// Saved under another generator version: listed, but not opened or queried.
    GeneratorMismatch,
}

/// One universe's identity, as the registry holds it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Universe {
    id: UniverseId,
    name: UniverseName,
    seed: u64,
    generator_version: GeneratorVersion,
}

impl Universe {
    /// The save's identity.
    #[must_use]
    pub fn id(&self) -> UniverseId {
        self.id
    }

    /// The operator-given name.
    #[must_use]
    pub fn name(&self) -> &UniverseName {
        &self.name
    }

    /// The seed the galaxy is generated from. The server keeps it as a `u64` and wraps it in
    /// [`hyperion_sim::Seed`] only when it calls the sim.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The generator version the universe was created with.
    #[must_use]
    pub fn generator_version(&self) -> GeneratorVersion {
        self.generator_version
    }

    /// What identifies the generated galaxy: `(seed, generator_version)`, the key of every cache.
    #[must_use]
    pub fn key(&self) -> GalaxyKey {
        GalaxyKey::new(self.seed, self.generator_version)
    }

    /// Whether this server can open the universe.
    #[must_use]
    pub fn compatibility(&self) -> Compatibility {
        if self.generator_version.is_supported() {
            Compatibility::Compatible
        } else {
            Compatibility::GeneratorMismatch
        }
    }
}

impl From<SavedUniverse> for Universe {
    fn from(saved: SavedUniverse) -> Self {
        Self {
            id: saved.id(),
            name: saved.name().clone(),
            seed: saved.seed(),
            generator_version: saved.generator_version(),
        }
    }
}
