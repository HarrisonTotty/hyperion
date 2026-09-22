//! The generator version, half of what identifies a universe.
//!
//! A universe is `(seed, generator_version)`. Any change that alters generated output bumps
//! [`GENERATOR_VERSION`], and the golden files are regenerated with `just bless` in the same
//! commit. Before the first release bumps are free; afterwards old versions either remain runnable
//! or saves made under them are declared incompatible. A saved game records the version it was
//! created with, and the golden tests make an accidental change visible: a change that moves a
//! pinned star fails CI until the bump is deliberate.
//!
//! What the version covers is listed in plan 01 under "Generator version": the block function,
//! the key and counter layout, the tag hash, the word-to-float rules, each sampler's algorithm and
//! word consumption, the threshold rule, the event-key convention, the pinned `libm` version, H
//! and L, and the ID layouts. Text forms and designations are not part of it.

use std::fmt;

/// The version of the generator a universe was created with.
///
/// Only the current version, [`GENERATOR_VERSION`], is supported until a release freezes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GeneratorVersion(u32);

/// The version of the generator in this build. Starts at 1.
pub const GENERATOR_VERSION: GeneratorVersion = GeneratorVersion::new(7);

impl GeneratorVersion {
    /// Wraps a version number, for instance one read from a save.
    #[must_use]
    pub const fn new(version: u32) -> Self {
        Self(version)
    }

    /// The version number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Whether this build can generate universes of this version: true only for
    /// [`GENERATOR_VERSION`].
    #[must_use]
    pub fn is_supported(self) -> bool {
        self == GENERATOR_VERSION
    }
}

impl fmt::Display for GeneratorVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_version_is_supported_and_others_are_not() {
        assert!(GENERATOR_VERSION.is_supported());
        assert_eq!(GENERATOR_VERSION.get(), 7);
        assert!(!GeneratorVersion::new(0).is_supported());
        assert!(!GeneratorVersion::new(GENERATOR_VERSION.get() + 1).is_supported());
    }

    #[test]
    fn versions_order_and_display_as_numbers() {
        assert!(GeneratorVersion::new(2) > GeneratorVersion::new(1));
        assert_eq!(GeneratorVersion::new(12).to_string(), "12");
    }
}
