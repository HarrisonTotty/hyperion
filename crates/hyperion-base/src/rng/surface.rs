//! The surface seeds: the two numbers a body's surface is drawn from, each opening its own scope.
//!
//! A body's [`SurfaceSeed`] (plan 14, P14.T23) is word 0 of its `body.surface` stream, and its
//! [`DetailSeed`] (plan R09) word 0 of its `body.surface.detail` stream, both in the sim, so they
//! share nothing but the universe seed. The surface seed stays on the server and keys the coarse
//! pass, whose tags have scope [`TagScope::SurfaceCoarse`]; the detail seed is sent to clients and
//! keys the local synthesis, whose tags have scope [`TagScope::SurfaceDetail`]. Each opens its own
//! scope's tags and nothing else, and [`Stream::open`] refuses both scopes, so no path through
//! these types seeds the synthesis from the universe seed or the coarse pass from the detail seed
//! (plan R09, Design note 2). That is a discipline for an honest client, not a security boundary:
//! `new` is public, for the wire and for the sim's `hooks::surface_seed` (the rendering
//! brainstorm's "Knowledge, and the surface seed").
//!
//! # Keying
//!
//! | Word           | Contents                                                                        |
//! | -------------- | ------------------------------------------------------------------------------- |
//! | Key word 0     | The surface or detail seed, in the universe seed's place                        |
//! | Key word 1     | The domain tag's hash                                                           |
//! | Counter word 0 | The key's word: a surface cell's face, level, `i` and `j`, or a surface item's number |
//! | Counter word 1 | `sub << 48 \| block`: a surface cell's instance, or 0, and the 48-bit block number |
//!
//! The body is in the seed, so it is not in the key ([`ObjectKey::surface_cell`],
//! [`ObjectKey::surface_item`]).

use std::fmt;

use super::{DomainTag, ObjectKey, Stream, TagScope};

/// A body's surface seed (P14.T23): 64 bits, from which the server's coarse surface pass draws.
///
/// It is word 0 of the body's `body.surface` stream, a function of the universe seed and the
/// body's ID alone, built by the sim's `planetary::hooks::surface_seed`, which re-exports the type
/// at `planetary::hooks::SurfaceSeed`. It opens [`TagScope::SurfaceCoarse`] tags and nothing else,
/// and no wire type carries it: a client is sent the body's [`DetailSeed`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SurfaceSeed(u64);

impl SurfaceSeed {
    /// The surface seed of these 64 bits.
    ///
    /// The sim's `hooks::surface_seed` is its one caller outside tests.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The seed's 64 bits.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Opens the stream of `object` under `tag`, keyed by this seed in the universe seed's place,
    /// at word 0.
    ///
    /// # Panics
    ///
    /// If `tag`'s scope is not [`TagScope::SurfaceCoarse`], in release builds too: the surface
    /// seed keys the coarse pass and nothing else. The key's own scope is not read (see
    /// [`ObjectKey`]).
    #[must_use]
    pub fn stream(self, tag: DomainTag, object: ObjectKey) -> Stream {
        open(self.0, TagScope::SurfaceCoarse, "surface", tag, object)
    }
}

impl fmt::Display for SurfaceSeed {
    /// Sixteen lowercase hexadecimal digits, as the crate prints other 64-bit identifiers.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// A body's detail seed (plan R09): 64 bits, from which the local surface synthesis draws, on the
/// server and on every client.
///
/// It is word 0 of the body's `body.surface.detail` stream in the sim, which shares nothing with
/// the [`SurfaceSeed`] but the universe seed, and it opens [`TagScope::SurfaceDetail`] tags and
/// nothing else, so a client that holds it can elaborate the surveyed coarse field but cannot
/// regenerate the field itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DetailSeed(u64);

impl DetailSeed {
    /// The detail seed of these 64 bits: the sim's, or the wire's.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The seed's 64 bits, as the wire carries them.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Opens the stream of `object` under `tag`, keyed by this seed in the universe seed's place,
    /// at word 0.
    ///
    /// # Panics
    ///
    /// If `tag`'s scope is not [`TagScope::SurfaceDetail`], in release builds too: the detail
    /// seed keys the local synthesis and nothing else. The key's own scope is not read (see
    /// [`ObjectKey`]).
    #[must_use]
    pub fn stream(self, tag: DomainTag, object: ObjectKey) -> Stream {
        open(self.0, TagScope::SurfaceDetail, "detail", tag, object)
    }
}

impl fmt::Display for DetailSeed {
    /// Sixteen lowercase hexadecimal digits, as the wire's `DetailSeedHex` is.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// The stream of `object` under `tag`, keyed by `seed`, once `tag` is shown to be of the seed's
/// own scope `own`; `kind` names the seed in the panic.
#[must_use]
fn open(seed: u64, own: TagScope, kind: &str, tag: DomainTag, object: ObjectKey) -> Stream {
    assert!(
        tag.scope() == own,
        "domain tag {} has scope {:?}, but a {kind} seed opens only {own:?} tags",
        tag.name(),
        tag.scope(),
    );
    Stream::from_words([seed, tag.hash()], object.word(), object.sub())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::rng::threefry2x64_20;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// Tags of the two surface scopes, minted for these tests: the surface crate's registry holds
    /// the real ones, which this crate cannot see.
    const COARSE_TAG: DomainTag =
        DomainTag::registered("selftest.surface_coarse", TagScope::SurfaceCoarse);
    const DETAIL_TAG: DomainTag =
        DomainTag::registered("selftest.surface_detail", TagScope::SurfaceDetail);

    const VALUE: u64 = 0x5eed_0000_0009_0001;

    fn cell() -> ObjectKey {
        ObjectKey::surface_cell(2, 8, 131, 77, 5).unwrap()
    }

    /// Each seed's stream is the block function's words under (seed, tag hash), at the key's
    /// word and `sub`, the universe seed's layout with the seed in its place.
    #[test]
    fn each_seed_opens_its_own_scope_in_the_universe_seed_s_place() {
        let key = cell();
        let sub = u64::from(key.sub()) << 48;
        let mut coarse = SurfaceSeed::new(VALUE).stream(COARSE_TAG, key);
        let mut detail = DetailSeed::new(VALUE).stream(DETAIL_TAG, key);
        for block in 0..3 {
            let counter = [key.word(), sub | block];
            assert_eq!(
                [coarse.next_u64(), coarse.next_u64()],
                threefry2x64_20([VALUE, COARSE_TAG.hash()], counter)
            );
            assert_eq!(
                [detail.next_u64(), detail.next_u64()],
                threefry2x64_20([VALUE, DETAIL_TAG.hash()], counter)
            );
        }
        let item = SurfaceSeed::new(VALUE).stream(COARSE_TAG, ObjectKey::surface_item(9));
        assert_eq!(
            item.word_at(0),
            threefry2x64_20([VALUE, COARSE_TAG.hash()], [9, 0])[0]
        );
    }

    /// A seed of the same value as a universe seed opens a stream unlike any the universe seed
    /// opens, since no other tag has a surface scope's hash: the tags differ.
    #[test]
    fn a_surface_stream_differs_from_the_universe_seed_s_self_test_stream() {
        let key = ObjectKey::surface_item(0);
        let universe = Stream::open(Seed::new(VALUE), crate::rng::tags::SELFTEST_STREAM, key);
        let coarse = SurfaceSeed::new(VALUE).stream(COARSE_TAG, key);
        let detail = DetailSeed::new(VALUE).stream(DETAIL_TAG, key);
        assert_ne!(universe.word_at(0), coarse.word_at(0));
        assert_ne!(universe.word_at(0), detail.word_at(0));
        assert_ne!(coarse.word_at(0), detail.word_at(0));
    }

    #[test]
    fn seeds_keep_their_bits_and_print_as_sixteen_hex_digits() {
        assert_eq!(SurfaceSeed::new(VALUE).get(), VALUE);
        assert_eq!(DetailSeed::new(VALUE).get(), VALUE);
        assert_eq!(SurfaceSeed::new(0xab).to_string(), "00000000000000ab");
        assert_eq!(DetailSeed::new(0xab).to_string(), "00000000000000ab");
    }
}
