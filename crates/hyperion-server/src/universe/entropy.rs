//! Where the server's randomness comes from: seeds the operator leaves out, and universe IDs.
//!
//! Randomness stays out of the sim (plan 04, design note 19). The server draws it through the
//! [`Entropy`] trait: [`OsEntropy`] in production, [`SequenceEntropy`] in tests, so that a test
//! knows every seed and ID the server will draw.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::sync::{Mutex, PoisonError};

/// A source of random 64-bit values.
pub trait Entropy: fmt::Debug + Send + Sync {
    /// Draws the next value.
    ///
    /// # Errors
    ///
    /// [`DrawEntropyError`] when the source cannot produce a value.
    fn draw_u64(&self) -> Result<u64, DrawEntropyError>;
}

/// The operating system's random number generator, through the `getrandom` crate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct OsEntropy;

impl Entropy for OsEntropy {
    fn draw_u64(&self) -> Result<u64, DrawEntropyError> {
        getrandom::u64().map_err(|error| DrawEntropyError::Os(OsRandomError(error)))
    }
}

/// A test double that hands out a fixed sequence of values, in order, then fails.
///
/// Integration tests use only the public API, so this lives here rather than behind
/// `#[cfg(test)]`. Production code never constructs one.
#[derive(Debug, Default)]
pub struct SequenceEntropy {
    values: Mutex<VecDeque<u64>>,
}

impl SequenceEntropy {
    /// A source that returns `values` in order and then [`DrawEntropyError::Exhausted`].
    #[must_use]
    pub fn new(values: impl IntoIterator<Item = u64>) -> Self {
        Self {
            values: Mutex::new(values.into_iter().collect()),
        }
    }

    /// How many values are left.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.lock().len()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, VecDeque<u64>> {
        // A queue pop cannot panic part-way, so a poisoned lock still guards a whole queue.
        self.values.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Entropy for SequenceEntropy {
    fn draw_u64(&self) -> Result<u64, DrawEntropyError> {
        self.lock().pop_front().ok_or(DrawEntropyError::Exhausted)
    }
}

/// A random value could not be drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawEntropyError {
    /// The operating system's random number generator failed.
    Os(OsRandomError),
    /// A [`SequenceEntropy`] has handed out every value it was given.
    Exhausted,
}

impl fmt::Display for DrawEntropyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Os(_) => f.write_str("the operating system's random number generator failed"),
            Self::Exhausted => f.write_str("the entropy sequence is exhausted"),
        }
    }
}

impl Error for DrawEntropyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Os(error) => Some(error),
            Self::Exhausted => None,
        }
    }
}

/// The operating system's error from a failed draw.
///
/// Opaque, so that the `getrandom` crate's API stays behind [`OsEntropy`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct OsRandomError(getrandom::Error);

impl fmt::Debug for OsRandomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for OsRandomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl Error for OsRandomError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_entropy_returns_two_different_values() {
        // Two equal draws from a working generator happen with probability 2⁻⁶⁴.
        let first = OsEntropy.draw_u64().unwrap();
        let second = OsEntropy.draw_u64().unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn sequence_entropy_returns_its_values_in_order_then_fails() {
        let entropy = SequenceEntropy::new([3, 1, 2]);
        assert_eq!(entropy.remaining(), 3);
        assert_eq!(entropy.draw_u64(), Ok(3));
        assert_eq!(entropy.draw_u64(), Ok(1));
        assert_eq!(entropy.draw_u64(), Ok(2));
        assert_eq!(entropy.draw_u64(), Err(DrawEntropyError::Exhausted));
        assert_eq!(entropy.remaining(), 0);
    }
}
