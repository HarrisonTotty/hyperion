//! Streams: the words drawn for one (seed, domain tag, object).

use super::threefry::threefry2x64_20;
use super::{DomainTag, ObjectKey, Seed, TagScope};

/// Blocks in a stream: `block` is the low 48 bits of counter word 1.
const BLOCKS: u64 = 1 << 48;

/// Bit position of `sub` in counter word 1.
const SUB_SHIFT: u32 = 48;

/// A random stream: the words drawn for one object under one domain tag, in a universe.
///
/// A stream is a window onto the Threefry2x64-20 block function ([`threefry2x64_20`]) with its key
/// and counter fixed by what it is for:
///
/// | Word           | Contents                                                          |
/// | -------------- | ----------------------------------------------------------------- |
/// | Key word 0     | The universe [`Seed`]                                             |
/// | Key word 1     | The domain tag's hash, [`DomainTag::hash`]                        |
/// | Counter word 0 | The object's word, [`ObjectKey::word`]                            |
/// | Counter word 1 | `sub << 48 \| block`: [`ObjectKey::sub`] and the 48-bit block number |
///
/// Word `n` of the stream is output `n & 1` of block `n >> 1`, so a stream holds 2⁴⁹ words
/// ([`Stream::WORDS`]). Because the block function is a bijection on the counter for a fixed key,
/// two streams that differ in the object's word or `sub` never share a block, and streams under
/// different seeds or tags are unrelated. The generator version is not in the key, so a version
/// bump moves only what its code change moves.
///
/// The words are what the samplers consume: [`uniform`](Self::uniform), [`normal`](Self::normal),
/// [`poisson`](Self::poisson) and the rest each document how many they take. A stream is a plain
/// value, its position and nothing else of consequence, so cloning one forks it: both copies draw
/// the same words from there on.
///
/// # Examples
///
/// Random access and sequential draws agree:
///
/// ```
/// use hyperion_base::Seed;
/// use hyperion_base::rng::{ObjectKey, Stream, tags};
///
/// let mut stream = Stream::open(Seed::new(42), tags::SELFTEST_STREAM, ObjectKey::galaxy());
/// let third = stream.word_at(2);
/// stream.seek(2);
/// assert_eq!(stream.next_u64(), third);
/// assert_eq!(stream.position(), 3);
/// ```
///
/// The central promise, that a new property under a new tag moves nothing already drawn, is shown
/// in the sim's `rng` module docs, since it needs an event key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Stream {
    /// `(seed, tag hash)`.
    key: [u64; 2],
    /// Counter word 0: the object's word.
    object: u64,
    /// The high bits of counter word 1: `sub << 48`.
    sub_bits: u64,
    /// The number of the next word `next_u64` returns.
    position: u64,
    /// When `position` is odd, the second word of block `position >> 1`, which the previous call
    /// computed; otherwise 0, so that two streams in the same place compare equal.
    pending: u64,
}

impl Stream {
    /// The number of words in a stream: 2⁴⁸ blocks of two.
    pub const WORDS: u64 = 1 << 49;

    /// Opens the stream of `object` under `tag` in the universe of `seed`, at word 0.
    ///
    /// # Panics
    ///
    /// If the tag's scope is [`TagScope::SurfaceCoarse`] or [`TagScope::SurfaceDetail`]: a
    /// surface tag is opened from a body's surface or detail seed, by
    /// [`SurfaceSeed::stream`](super::SurfaceSeed::stream) or
    /// [`DetailSeed::stream`](super::DetailSeed::stream), never from the universe seed (plan R09,
    /// Design note 2).
    ///
    /// If the key's scope is not the tag's (Design note 3 of the determinism plan): a tag fixes
    /// what its counter word names, which is what lets a cell's word equal its candidate 0's ID
    /// and body 0 share its system's word. A tag of scope [`TagScope::SelfTest`] accepts a key of
    /// any scope, because tests exercise every kind of key; no key has scope [`TagScope::Event`],
    /// so an event tag is never opened here. The checks hold in release builds too: a mismatch
    /// would silently alias two objects' streams and change a galaxy, and they cost a few byte
    /// comparisons.
    #[must_use]
    pub fn open(seed: Seed, tag: DomainTag, object: ObjectKey) -> Self {
        assert!(
            !matches!(
                tag.scope(),
                TagScope::SurfaceCoarse | TagScope::SurfaceDetail
            ),
            "domain tag {} has scope {:?}, which only a body's surface or detail seed opens",
            tag.name(),
            tag.scope(),
        );
        assert!(
            tag.scope() == object.scope() || tag.scope() == TagScope::SelfTest,
            "domain tag {} has scope {:?}, but the object key has scope {:?}",
            tag.name(),
            tag.scope(),
            object.scope(),
        );
        Self::from_words([seed.get(), tag.hash()], object.word(), object.sub())
    }

    /// A stream from its raw key words, counter word 0 and `sub`, at word 0.
    ///
    /// [`open`](Self::open) keys a stream by seed and domain tag; an event's streams are keyed by
    /// its subject's event key instead (the sim's `EventKey`, through `RawEventKey`), which is why
    /// this exists.
    #[must_use]
    pub(super) fn from_words(key: [u64; 2], object: u64, sub: u16) -> Self {
        Self {
            key,
            object,
            sub_bits: u64::from(sub) << SUB_SHIFT,
            position: 0,
            pending: 0,
        }
    }

    /// The counter of block `block`: `(object word, sub << 48 | block)`.
    ///
    /// # Panics
    ///
    /// If `block` is 2⁴⁸ or more, where it would run into the next `sub`.
    #[inline]
    fn counter(&self, block: u64) -> [u64; 2] {
        assert!(
            block < BLOCKS,
            "a stream holds 2^49 words; block {block} is past its end"
        );
        [self.object, self.sub_bits | block]
    }

    /// The two words of block `block`.
    #[inline]
    fn block(&self, block: u64) -> [u64; 2] {
        threefry2x64_20(self.key, self.counter(block))
    }

    /// The next word, advancing the stream by one.
    ///
    /// A call at an even position computes a block and keeps its second word for the next call.
    ///
    /// # Panics
    ///
    /// If the stream is exhausted, after 2⁴⁹ words. That is unreachable in practice: at a
    /// nanosecond a word it takes six days.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let n = self.position;
        let word = if n & 1 == 0 {
            let [first, second] = self.block(n >> 1);
            self.pending = second;
            first
        } else {
            std::mem::take(&mut self.pending)
        };
        // `block` rejected n ≥ 2⁴⁹ above, and an odd n is below 2⁴⁹ too, so this cannot overflow.
        self.position = n + 1;
        word
    }

    /// Word `n` of the stream, without moving it.
    ///
    /// # Panics
    ///
    /// If `n` is [`Stream::WORDS`] or more.
    #[inline]
    #[must_use]
    pub fn word_at(&self, n: u64) -> u64 {
        let [first, second] = self.block(n >> 1);
        if n & 1 == 0 { first } else { second }
    }

    /// The number of the word the next draw returns: how many words have been drawn, unless the
    /// stream was moved with [`seek`](Self::seek).
    #[must_use]
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Moves the stream so that the next draw returns word `n`.
    ///
    /// Seeking is what lets a draw be addressed by number: a property drawn from words `4k` to
    /// `4k + 3` can be read for any `k` without drawing the ones before it.
    ///
    /// # Panics
    ///
    /// If `n` is above [`Stream::WORDS`]. Seeking to exactly `WORDS` is allowed; drawing there
    /// panics.
    pub fn seek(&mut self, n: u64) {
        assert!(
            n <= Self::WORDS,
            "a stream holds 2^49 words; cannot seek to word {n}"
        );
        self.position = n;
        self.pending = if n & 1 == 0 { 0 } else { self.block(n >> 1)[1] };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::tags;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    const SEED: Seed = Seed::new(0x5eed_0000_0000_0001);

    fn first_words(stream: &Stream, n: u64) -> Vec<u64> {
        let mut stream = stream.clone();
        (0..n).map(|_| stream.next_u64()).collect()
    }

    fn open(object: ObjectKey) -> Stream {
        Stream::open(SEED, tags::SELFTEST_STREAM, object)
    }

    /// A system's raw ID, standing in for one: the stream sees only the word.
    const SYSTEM: u64 = 0x0200_0800_2000_0000;

    #[test]
    fn word_at_equals_the_sequential_draws() {
        let stream = open(ObjectKey::galaxy_item(3));
        let mut sequential = stream.clone();
        for n in 0..=1_000 {
            assert_eq!(sequential.position(), n);
            assert_eq!(stream.word_at(n), sequential.next_u64(), "word {n}");
        }
    }

    #[test]
    fn words_are_the_block_outputs_in_order() {
        let mut stream = open(ObjectKey::cell(0xabc));
        for block in 0..4 {
            let expected =
                threefry2x64_20([SEED.get(), tags::SELFTEST_STREAM.hash()], [0xabc, block]);
            assert_eq!([stream.next_u64(), stream.next_u64()], expected);
        }
    }

    #[test]
    fn seek_lands_on_the_word_asked_for_at_either_parity() {
        let stream = open(ObjectKey::feature(99));
        let words = first_words(&stream, 41);
        for target in [0, 1, 2, 17, 38, 39] {
            let mut moved = open(ObjectKey::feature(99));
            moved.next_u64();
            moved.seek(target);
            assert_eq!(moved.position(), target);
            let n = usize::try_from(target).unwrap();
            assert_eq!(moved.next_u64(), words[n], "seek to {target}");
            assert_eq!(moved.next_u64(), words[n + 1], "after seek to {target}");
        }
    }

    #[test]
    fn two_streams_at_the_same_word_are_equal() {
        let mut a = open(ObjectKey::galaxy());
        let mut b = a.clone();
        a.next_u64();
        a.next_u64();
        b.seek(2);
        assert_eq!(a, b);
        a.next_u64();
        b.seek(3);
        assert_eq!(a, b);
    }

    /// Body 1's first block and body 0's last block are neighbours in counter word 1, one apart;
    /// they are different counters, and body 0 cannot draw past its last block into body 1's.
    #[test]
    fn sub_and_block_do_not_alias() {
        let body0 = open(ObjectKey::body(SYSTEM, 0));
        let body1 = open(ObjectKey::body(SYSTEM, 1));
        let last = body0.counter(BLOCKS - 1);
        let first = body1.counter(0);
        assert_eq!(last, [SYSTEM, 0x0000_ffff_ffff_ffff]);
        assert_eq!(first, [SYSTEM, 0x0001_0000_0000_0000]);
        assert_ne!(last, first);
        assert_ne!(
            body0.word_at(Stream::WORDS - 1),
            body1.word_at(0),
            "the last word of body 0 is not body 1's first"
        );
    }

    #[test]
    fn keys_hold_seed_and_tag_and_counters_hold_the_object() {
        let stream = open(ObjectKey::body(SYSTEM, 0x0102));
        assert_eq!(stream.key, [SEED.get(), tags::SELFTEST_STREAM.hash()]);
        assert_eq!(stream.counter(7), [SYSTEM, (0x0102 << 48) | 7]);
        assert_eq!(open(ObjectKey::system(SYSTEM)).counter(7), [SYSTEM, 7]);
    }

    #[test]
    fn a_matching_scope_opens() {
        const CELL_TAG: DomainTag = DomainTag::registered("selftest.cell", TagScope::Cell);
        let mut stream = Stream::open(SEED, CELL_TAG, ObjectKey::cell(8));
        assert_eq!(stream.next_u64(), stream.word_at(0));
    }
}
