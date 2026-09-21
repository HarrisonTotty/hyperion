//! Domain tags: the names of the property groups that random streams are keyed by.

/// What a domain tag's counter word names: the kind of object whose streams it keys.
///
/// A tag is declared with one scope, and every stream opened under it names an object of that
/// scope. That is what makes it safe for a cell's word (an ID with its index zeroed) to equal the
/// ID of the cell's candidate 0, and for body index 0 to share its system's word: the two are
/// never opened under the same tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TagScope {
    /// The galaxy as a whole, or a numbered item of a galaxy-wide list.
    Galaxy,
    /// A generation cell, named by the ID of its candidate 0 with the index zeroed.
    Cell,
    /// A feature, named by its object word.
    Feature,
    /// A system, named by its ID.
    System,
    /// A body, named by its system's ID and its body index.
    Body,
    /// An event tag's key derivation; never opened as an ordinary stream.
    Event,
    /// Tests and golden files only; never opened by a generator.
    SelfTest,
}

/// A domain tag: a registered name, its 64-bit hash, and the scope of the objects it keys.
///
/// Tags exist only as the constants of the single registry, [`tags`](super::tags); there is no
/// public constructor, so every tag in use is in [`tags::ALL`](super::tags::ALL) and the collision
/// check covers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DomainTag {
    name: &'static str,
    hash: u64,
    scope: TagScope,
}

impl DomainTag {
    /// A registry entry. Only `domain_tags!` calls this, and it is visible only inside `rng`, so
    /// no other module of the crate can mint a tag that bypasses the registry.
    #[must_use]
    pub(super) const fn registered(name: &'static str, scope: TagScope) -> Self {
        Self {
            name,
            hash: hash_tag_name(name),
            scope,
        }
    }

    /// The tag's name, such as `"star.mass"`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// The FNV-1a hash of the name, [`hash_tag_name`]: the stream key's second word.
    #[must_use]
    pub const fn hash(self) -> u64 {
        self.hash
    }

    /// The scope of the objects whose streams this tag keys.
    #[must_use]
    pub const fn scope(self) -> TagScope {
        self.scope
    }
}

/// FNV-1a offset basis, 64-bit.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a prime, 64-bit.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The 64-bit FNV-1a hash of a tag name's UTF-8 bytes (Fowler, Noll and Vo; offset basis
/// `0xcbf29ce484222325`, prime `0x100000001b3`).
///
/// The hash only has to be injective over the registry, which [`assert_tag_names`] proves at
/// compile time; the block function does the mixing.
///
/// # Examples
///
/// ```
/// use hyperion_sim::rng::hash_tag_name;
///
/// assert_eq!(hash_tag_name("star.mass"), 0x76f0_46fe_b1fe_aee9);
/// ```
#[must_use]
pub const fn hash_tag_name(name: &str) -> u64 {
    let bytes = name.as_bytes();
    let mut hash = FNV_OFFSET_BASIS;
    let mut i = 0;
    while i < bytes.len() {
        // `u64::from` is not `const`; a byte widens to `u64` losslessly.
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

/// Whether `name` is a well-formed tag name: `[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+`, that is two
/// or more dot-separated segments, each a lower-case letter followed by lower-case letters,
/// digits and underscores.
#[must_use]
pub const fn is_valid_tag_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    let mut segments = 0;
    let mut at_segment_start = true;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if at_segment_start {
            if !b.is_ascii_lowercase() {
                return false;
            }
            segments += 1;
            at_segment_start = false;
        } else if b == b'.' {
            at_segment_start = true;
        } else if !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
            return false;
        }
        i += 1;
    }
    !at_segment_start && segments >= 2
}

/// Byte equality of two strings, usable in `const`.
const fn same_str(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Panics unless every name is well-formed ([`is_valid_tag_name`]) and no two names are equal or
/// share a hash.
///
/// The registry calls this in a `const`, so a duplicate, a collision or a malformed name fails
/// compilation:
///
/// ```
/// const _: () = hyperion_sim::rng::assert_tag_names(&["star.mass", "planet.orbits"]);
/// ```
///
/// ```compile_fail,E0080
/// // A duplicate name.
/// const _: () = hyperion_sim::rng::assert_tag_names(&["star.mass", "star.mass"]);
/// ```
///
/// ```compile_fail,E0080
/// // A malformed name: upper case.
/// const _: () = hyperion_sim::rng::assert_tag_names(&["Star.mass"]);
/// ```
///
/// ```compile_fail,E0080
/// // A malformed name: one segment.
/// const _: () = hyperion_sim::rng::assert_tag_names(&["mass"]);
/// ```
///
/// # Panics
///
/// On the first malformed name, duplicate name or hash collision.
pub const fn assert_tag_names(names: &[&str]) {
    let mut i = 0;
    while i < names.len() {
        assert!(
            is_valid_tag_name(names[i]),
            "a domain tag name must match [a-z][a-z0-9_]*(.[a-z][a-z0-9_]*)+"
        );
        let hash = hash_tag_name(names[i]);
        let mut j = 0;
        while j < i {
            assert!(!same_str(names[i], names[j]), "duplicate domain tag name");
            assert!(hash != hash_tag_name(names[j]), "domain tag hash collision");
            j += 1;
        }
        i += 1;
    }
}

/// Declares the registry of domain tags. Used once, in `rng/tags.rs`.
///
/// Each entry reads `CONST_NAME: Scope = "tag.name";` and becomes a `pub const` [`DomainTag`].
/// The macro also emits `ALL`, every tag in registry order, and a `const` assertion that fails
/// compilation on a malformed name, a duplicate name or a hash collision.
macro_rules! domain_tags {
    ($( $(#[$meta:meta])* $name:ident : $scope:ident = $text:literal ; )*) => {
        $(
            $(#[$meta])*
            pub const $name: $crate::rng::DomainTag =
                $crate::rng::DomainTag::registered($text, $crate::rng::TagScope::$scope);
        )*

        /// Every registered domain tag, in registry order.
        pub const ALL: &[$crate::rng::DomainTag] = &[$($name),*];

        const _: () = $crate::rng::assert_tag_names(&[$($text),*]);
    };
}

pub(super) use domain_tags;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_1a_matches_independent_values() {
        // Computed independently with a Python FNV-1a implementation.
        assert_eq!(hash_tag_name("star.mass"), 0x76f0_46fe_b1fe_aee9);
        assert_eq!(hash_tag_name("planet.orbits"), 0x00b5_17d6_2a08_cb6e);
        assert_eq!(hash_tag_name("moon.count"), 0xa00d_d982_cc04_3ce1);
        assert_eq!(hash_tag_name(""), FNV_OFFSET_BASIS);
    }

    #[test]
    fn tag_names_follow_the_grammar() {
        for good in [
            "star.mass",
            "a.b",
            "galaxy.cell.candidates",
            "x9_.y_2",
            "event.selftest",
        ] {
            assert!(is_valid_tag_name(good), "{good}");
        }
        for bad in [
            "",
            "star",
            "star.",
            ".mass",
            "star..mass",
            "Star.mass",
            "star.Mass",
            "9star.mass",
            "star._mass",
            "star.mass ",
            "star-mass.x",
            "star.9",
        ] {
            assert!(!is_valid_tag_name(bad), "{bad}");
        }
    }

    #[test]
    #[should_panic(expected = "duplicate domain tag name")]
    fn a_duplicate_name_panics() {
        assert_tag_names(&["star.mass", "moon.count", "star.mass"]);
    }

    #[test]
    #[should_panic(expected = "must match")]
    fn a_malformed_name_panics() {
        assert_tag_names(&["star.mass", "Moon.count"]);
    }
}
