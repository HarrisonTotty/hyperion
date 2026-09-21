//! The key that names a generated galaxy in every cache.

use hyperion_sim::GeneratorVersion;

/// What identifies a generated galaxy: `(seed, generator_version)`.
///
/// Part of every cache key (plan 04, design note 23), so that two saves of one seed share their
/// entries and nothing generated under one version is served for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GalaxyKey {
    seed: u64,
    generator_version: GeneratorVersion,
}

impl GalaxyKey {
    /// The galaxy generated from `seed` by `generator_version`.
    #[must_use]
    pub const fn new(seed: u64, generator_version: GeneratorVersion) -> Self {
        Self {
            seed,
            generator_version,
        }
    }

    /// The seed, as the server keeps it; wrap it in [`hyperion_sim::Seed`] to call the sim.
    #[must_use]
    pub const fn seed(self) -> u64 {
        self.seed
    }

    /// The generator version.
    #[must_use]
    pub const fn generator_version(self) -> GeneratorVersion {
        self.generator_version
    }
}
