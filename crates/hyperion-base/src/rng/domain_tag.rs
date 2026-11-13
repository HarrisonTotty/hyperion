//! Domain tags: the names of the property groups that random streams are keyed by.

/// What a domain tag's counter word names: the kind of object whose streams it keys.
///
/// A tag is declared with one scope, and every stream opened under it names an object of that
/// scope. That is what makes it safe for a cell's word (an ID with its index zeroed) to equal the
/// ID of the cell's candidate 0, and for body index 0 to share its system's word: the two are
/// never opened under the same tag.
///
/// The two surface scopes are different: their tags are opened from a body's surface or detail
/// seed, never from the universe seed, so [`Stream::open`](super::Stream::open) refuses them, and
/// only [`SurfaceSeed::stream`](super::SurfaceSeed::stream) and
/// [`DetailSeed::stream`](super::DetailSeed::stream) open them (plan R09, Design note 2).
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
    /// The server's coarse surface pass: opened only from a body's
    /// [`SurfaceSeed`](super::SurfaceSeed), on a cube-sphere cell or a numbered item.
    SurfaceCoarse,
    /// The local surface synthesis, which clients run too: opened only from a body's
    /// [`DetailSeed`](super::DetailSeed), on a cube-sphere cell or a numbered item.
    SurfaceDetail,
}

/// A domain tag: a registered name, its 64-bit hash, and the scope of the objects it keys.
///
/// Tags exist only as the constants of a registry declared with
/// [`domain_tags!`](crate::domain_tags): this crate's [`tags`](super::tags), the surface crate's
/// and the sim's. The constructor is for that macro alone (it is public, and hidden, only because
/// the macro expands in other crates), so every tag a generator opens is in one registry's `ALL`,
/// and the sim's assertion that the three are disjoint ([`assert_registries_disjoint`]) covers it.
/// The one exception is a test that needs a tag outside every registry, to vary the tag, which
/// mints one directly and never opens a generator's stream with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DomainTag {
    name: &'static str,
    hash: u64,
    scope: TagScope,
}

impl DomainTag {
    /// A registry entry. Only [`domain_tags!`](crate::domain_tags) calls this.
    ///
    /// It is public because the macro's expansion calls it from the crates that declare a
    /// registry, and hidden from the documentation because nothing else may: a tag minted outside
    /// a registry would escape the collision checks. Tests that need a tag outside every registry,
    /// to vary it, are the one exception, and never open a generator's stream with it.
    #[doc(hidden)]
    #[must_use]
    pub const fn registered(name: &'static str, scope: TagScope) -> Self {
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
/// use hyperion_base::rng::hash_tag_name;
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
/// const _: () = hyperion_base::rng::assert_tag_names(&["star.mass", "planet.orbits"]);
/// ```
///
/// ```compile_fail,E0080
/// // A duplicate name.
/// const _: () = hyperion_base::rng::assert_tag_names(&["star.mass", "star.mass"]);
/// ```
///
/// ```compile_fail,E0080
/// // A malformed name: upper case.
/// const _: () = hyperion_base::rng::assert_tag_names(&["Star.mass"]);
/// ```
///
/// ```compile_fail,E0080
/// // A malformed name: one segment.
/// const _: () = hyperion_base::rng::assert_tag_names(&["mass"]);
/// ```
///
/// Each name is hashed once and compared only with the earlier names in its hash's bucket, about
/// one, so the check's steps grow linearly with the registry. That keeps it within rustc's limit
/// on a `const`'s evaluation (the deny-by-default `long_running_const_eval`), which a check that
/// hashed every earlier name again for each name passed at about 190 names.
///
/// # Panics
///
/// On the first malformed name, duplicate name or hash collision.
pub const fn assert_tag_names<const N: usize>(names: &[&str; N]) {
    let mut hashes = [0_u64; N];
    let mut i = 0;
    while i < N {
        hashes[i] = hash_tag_name(names[i]);
        i += 1;
    }
    if let Some(fault) = first_name_fault(names, &hashes) {
        panic!("{}", fault.message());
    }
}

/// Why [`assert_tag_names`] refuses a registry's names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NameFault {
    /// A name that is not well-formed ([`is_valid_tag_name`]).
    Malformed,
    /// A name equal to an earlier one.
    Duplicate,
    /// A name whose hash an earlier, different name has.
    Collision,
}

impl NameFault {
    /// The panic message of [`assert_tag_names`] for this fault.
    #[must_use]
    const fn message(self) -> &'static str {
        match self {
            Self::Malformed => "a domain tag name must match [a-z][a-z0-9_]*(.[a-z][a-z0-9_]*)+",
            Self::Duplicate => "duplicate domain tag name",
            Self::Collision => "domain tag hash collision",
        }
    }
}

/// The first fault of a registry's names, given each name's hash, or `None` if there is none.
///
/// Names are taken in order. Name `i` is checked first for its form, then against the earlier
/// name with its hash, if there is one. Every earlier name passed, so no two of them share a hash,
/// and that name is the only earlier one equal to name `i` or sharing its hash: the fault found is
/// the one that comparing each name with every earlier name, in order, would find first. Equal
/// hashes share a bucket, so that name is found by walking name `i`'s bucket, which holds about
/// one name with `N` buckets.
///
/// `hashes[i]` is the hash of `names[i]`, and equal names have equal hashes; the hashes are a
/// parameter so that tests can give names colliding ones.
#[must_use]
const fn first_name_fault<const N: usize>(
    names: &[&str; N],
    hashes: &[u64; N],
) -> Option<NameFault> {
    // Separate chaining: `head[b]` is the latest name in bucket `b` and `next[i]` the one before
    // name `i` in its bucket, the index `N` ending a chain.
    let mut head = [N; N];
    let mut next = [N; N];
    let mut i = 0;
    while i < N {
        if !is_valid_tag_name(names[i]) {
            return Some(NameFault::Malformed);
        }
        let bucket = bucket_of(hashes[i], N);
        let mut j = head[bucket];
        while j < N {
            if hashes[j] == hashes[i] {
                return Some(if same_str(names[i], names[j]) {
                    NameFault::Duplicate
                } else {
                    NameFault::Collision
                });
            }
            j = next[j];
        }
        next[i] = head[bucket];
        head[bucket] = i;
        i += 1;
    }
    None
}

/// The bucket of a hash among `buckets` buckets, `buckets` > 0: the hash modulo `buckets`.
///
/// It is the same on every target, since the remainder is taken in `u64` and is below `buckets`,
/// and it decides only which names are compared, never the check's outcome.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "a usize widens to u64 losslessly, and the remainder is below `buckets`, a usize"
)]
const fn bucket_of(hash: u64, buckets: usize) -> usize {
    (hash % buckets as u64) as usize
}

/// Panics if two registries share a tag name or a tag hash.
///
/// Each registry asserts its own names with [`assert_tag_names`]; this covers the pairs across
/// them, so that tags declared in different registries (the foundation's, the surface crate's and
/// the sim's) still never alias a stream. Called in a `const`, a clash fails compilation:
///
/// ```
/// use hyperion_base::rng::{assert_registries_disjoint, tags};
/// const _: () = assert_registries_disjoint(&[&[tags::SELFTEST_STREAM], &[]]);
/// ```
///
/// ```compile_fail,E0080
/// use hyperion_base::rng::{assert_registries_disjoint, tags};
/// const _: () = assert_registries_disjoint(&[&[tags::SELFTEST_STREAM], &[tags::SELFTEST_STREAM]]);
/// ```
///
/// # Panics
///
/// On the first name or hash found in two registries.
pub const fn assert_registries_disjoint(registries: &[&[DomainTag]]) {
    let mut a = 0;
    while a < registries.len() {
        let mut b = a + 1;
        while b < registries.len() {
            let mut i = 0;
            while i < registries[a].len() {
                let tag = registries[a][i];
                let mut j = 0;
                while j < registries[b].len() {
                    let other = registries[b][j];
                    assert!(
                        !same_str(tag.name, other.name),
                        "a domain tag name is in two registries"
                    );
                    assert!(
                        tag.hash != other.hash,
                        "domain tag hash collision across registries"
                    );
                    j += 1;
                }
                i += 1;
            }
            b += 1;
        }
        a += 1;
    }
}

/// Declares a registry of domain tags, inside a module of its own.
///
/// Each entry reads `CONST_NAME: Scope = "tag.name";` and becomes a `pub const` [`DomainTag`].
/// The macro also emits `ALL`, every tag in registry order, and a `const` assertion that fails
/// compilation on a malformed name, a duplicate name or a hash collision within it. There are
/// three registries, one per crate that opens streams: `hyperion_base::rng::tags`,
/// `hyperion_surface::tags` and the sim's `rng::tags`; the sim asserts them disjoint with
/// [`assert_registries_disjoint`](crate::rng::assert_registries_disjoint).
#[macro_export]
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

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
    fn each_fault_keeps_its_panic_message() {
        assert_eq!(
            NameFault::Malformed.message(),
            "a domain tag name must match [a-z][a-z0-9_]*(.[a-z][a-z0-9_]*)+"
        );
        assert_eq!(NameFault::Duplicate.message(), "duplicate domain tag name");
        assert_eq!(NameFault::Collision.message(), "domain tag hash collision");
    }

    #[test]
    fn each_fault_is_found() {
        assert_eq!(first_name_fault(&[], &[]), None);
        assert_eq!(
            first_name_fault(&["star.mass", "moon.count"], &[1, 2]),
            None
        );
        assert_eq!(
            first_name_fault(&["star.mass", "Moon.count"], &[1, 2]),
            Some(NameFault::Malformed)
        );
        assert_eq!(
            first_name_fault(&["star.mass", "moon.count", "star.mass"], &[1, 2, 1]),
            Some(NameFault::Duplicate)
        );
        // No two real names are known to share a 64-bit FNV-1a hash, so this one is given.
        assert_eq!(
            first_name_fault(&["star.mass", "moon.count", "planet.orbits"], &[1, 2, 1]),
            Some(NameFault::Collision)
        );
    }

    /// Names of every fault: two pairs share [`last_byte_mod_3`] (`star.mass` and
    /// `planet.orbits`, `moon.count` and `a.b`), and two are malformed.
    const POOL: [&str; 6] = [
        "star.mass",
        "moon.count",
        "planet.orbits",
        "a.b",
        "Star.mass",
        "mass",
    ];

    /// A hash under which many of [`POOL`]'s names collide.
    fn last_byte_mod_3(name: &str) -> u64 {
        u64::from(name.as_bytes().last().copied().unwrap_or(0) % 3)
    }

    /// [`last_byte_mod_3`] times 60, a multiple of every `N` up to 5: every name falls in bucket
    /// 0, so a chain is walked past names of other hashes.
    fn last_byte_mod_3_in_one_bucket(name: &str) -> u64 {
        last_byte_mod_3(name) * 60
    }

    /// The check as it was before it hashed each name once: each name, in order, for its form and
    /// then against every earlier name, in order.
    fn first_fault_pairwise(names: &[&str], hash: fn(&str) -> u64) -> Option<NameFault> {
        for (i, &name) in names.iter().enumerate() {
            if !is_valid_tag_name(name) {
                return Some(NameFault::Malformed);
            }
            for &earlier in &names[..i] {
                if name == earlier {
                    return Some(NameFault::Duplicate);
                }
                if hash(name) == hash(earlier) {
                    return Some(NameFault::Collision);
                }
            }
        }
        None
    }

    /// Asserts that [`first_name_fault`] finds the fault [`first_fault_pairwise`] finds, for every
    /// list of `N` names drawn from [`POOL`] with repetition.
    fn assert_agrees_with_pairwise<const N: usize>(hash: fn(&str) -> u64) {
        let mut digits = [0_usize; N];
        loop {
            let names: [&str; N] = std::array::from_fn(|k| POOL[digits[k]]);
            assert_eq!(
                first_name_fault(&names, &names.map(hash)),
                first_fault_pairwise(&names, hash),
                "{names:?}"
            );
            // The next list, counting in base `POOL.len()`.
            let mut k = 0;
            loop {
                if k == N {
                    return;
                }
                digits[k] += 1;
                if digits[k] < POOL.len() {
                    break;
                }
                digits[k] = 0;
                k += 1;
            }
        }
    }

    #[test]
    fn the_first_fault_is_the_pairwise_check_s() {
        for hash in [
            hash_tag_name as fn(&str) -> u64,
            last_byte_mod_3,
            last_byte_mod_3_in_one_bucket,
        ] {
            assert_agrees_with_pairwise::<0>(hash);
            assert_agrees_with_pairwise::<1>(hash);
            assert_agrees_with_pairwise::<2>(hash);
            assert_agrees_with_pairwise::<3>(hash);
            assert_agrees_with_pairwise::<4>(hash);
            assert_agrees_with_pairwise::<5>(hash);
        }
    }

    /// The size of [`SCALE_NAMES`], about five times the sim's registry of 193 tags.
    const SCALE: usize = 1_000;

    /// The bytes of [`SCALE_NAMES`]: `selftest.scale.` and three letters, `aaa` for name 0
    /// counting up in base 26.
    static SCALE_BYTES: [[u8; 18]; SCALE] = {
        const LETTERS: &[u8; 26] = b"abcdefghijklmnopqrstuvwxyz";
        let mut bytes = [*b"selftest.scale.aaa"; SCALE];
        let mut i = 0;
        while i < SCALE {
            bytes[i][15] = LETTERS[i / 676];
            bytes[i][16] = LETTERS[i / 26 % 26];
            bytes[i][17] = LETTERS[i % 26];
            i += 1;
        }
        bytes
    };

    /// A registry of [`SCALE`] distinct well-formed names.
    const SCALE_NAMES: [&str; SCALE] = {
        let mut names = [""; SCALE];
        let mut i = 0;
        while i < SCALE {
            names[i] = match core::str::from_utf8(&SCALE_BYTES[i]) {
                Ok(name) => name,
                Err(_) => panic!("the names are ASCII"),
            };
            i += 1;
        }
        names
    };

    // A registry of a thousand names passes the check within rustc's limit on a `const`'s
    // evaluation (`long_running_const_eval`, deny by default): the check is linear in the
    // registry, where one that hashed every earlier name again for each name failed at about 190.
    const _: () = assert_tag_names(&SCALE_NAMES);

    #[test]
    fn disjoint_registries_pass() {
        let a = DomainTag::registered("selftest.a", TagScope::SelfTest);
        let b = DomainTag::registered("selftest.b", TagScope::SelfTest);
        assert_registries_disjoint(&[&[a], &[b], &[]]);
    }
}
