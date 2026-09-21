//! The operator-given name of a universe.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

use crate::limits::MAX_UNIVERSE_NAME_CHARS;

/// A valid universe name: 1–48 characters after trimming, with no control characters.
///
/// Names are unique on a server without regard to case (plan 04, design note 16); the
/// comparison uses Unicode lower case, [`str::to_lowercase`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UniverseName(String);

impl UniverseName {
    /// The name as the operator gave it, trimmed.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The form in which names are compared for uniqueness.
    #[must_use]
    pub(crate) fn folded(&self) -> String {
        self.0.to_lowercase()
    }
}

impl FromStr for UniverseName {
    type Err = ParseUniverseNameError;

    /// Trims surrounding whitespace, then checks the length and the characters.
    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let name = raw.trim();
        let chars = name.chars().count();
        if chars == 0 {
            return Err(ParseUniverseNameError::Empty);
        }
        if chars > MAX_UNIVERSE_NAME_CHARS {
            return Err(ParseUniverseNameError::TooLong { chars });
        }
        if name.chars().any(char::is_control) {
            return Err(ParseUniverseNameError::ControlCharacter);
        }
        Ok(Self(name.to_owned()))
    }
}

impl fmt::Display for UniverseName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A universe name broke one of the rules of [`UniverseName`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseUniverseNameError {
    /// Nothing was left after trimming.
    Empty,
    /// Longer than [`MAX_UNIVERSE_NAME_CHARS`] characters after trimming.
    TooLong {
        /// The length after trimming, in characters.
        chars: usize,
    },
    /// The name contains a control character.
    ControlCharacter,
}

impl fmt::Display for ParseUniverseNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("the name is empty"),
            Self::TooLong { chars } => write!(
                f,
                "the name has {chars} characters, more than {MAX_UNIVERSE_NAME_CHARS}"
            ),
            Self::ControlCharacter => f.write_str("the name contains a control character"),
        }
    }
}

impl Error for ParseUniverseNameError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed() {
        let name: UniverseName = "  Kepler Reach \t".parse().unwrap();
        assert_eq!(name.as_str(), "Kepler Reach");
    }

    #[test]
    fn empty_and_blank_names_are_refused() {
        assert_eq!(
            "".parse::<UniverseName>(),
            Err(ParseUniverseNameError::Empty)
        );
        assert_eq!(
            " \t ".parse::<UniverseName>(),
            Err(ParseUniverseNameError::Empty)
        );
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        let longest = "é".repeat(MAX_UNIVERSE_NAME_CHARS);
        assert!(longest.parse::<UniverseName>().is_ok());
        let too_long = "a".repeat(MAX_UNIVERSE_NAME_CHARS + 1);
        assert_eq!(
            too_long.parse::<UniverseName>(),
            Err(ParseUniverseNameError::TooLong { chars: 49 })
        );
    }

    #[test]
    fn control_characters_are_refused() {
        assert_eq!(
            "Kepler\u{7}Reach".parse::<UniverseName>(),
            Err(ParseUniverseNameError::ControlCharacter)
        );
        assert_eq!(
            "Kepler\nReach".parse::<UniverseName>(),
            Err(ParseUniverseNameError::ControlCharacter)
        );
    }

    #[test]
    fn folding_ignores_case() {
        let upper: UniverseName = "KEPLER Reach".parse().unwrap();
        let lower: UniverseName = "kepler reach".parse().unwrap();
        assert_eq!(upper.folded(), lower.folded());
    }
}
