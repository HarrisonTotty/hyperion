//! Random streams, not a random sequence.
//!
//! Each value the generator draws comes from a stream keyed by what it is for:
//! `(universe seed, domain tag; object)`. Adding a draw for a new property then moves nothing
//! already generated, which is what makes lazy, out-of-order and parallel generation safe
//! (brainstorm, "Random streams, not a random sequence"; Salmon et al. 2011).
//!
//! A [`DomainTag`] names the property group (`"star.mass"`, `"planet.orbits"`). Its name is hashed
//! to a `u64` at compile time with [`hash_tag_name`], and it carries a [`TagScope`] fixing what
//! kind of object its streams name. Every tag is an entry of a registry declared with
//! [`domain_tags!`](crate::domain_tags), whose `const` assertion rejects a malformed name, a
//! duplicate or a hash collision at compile time. There are three registries: this crate's
//! [`tags`], for the tags the foundation itself opens; `hyperion_surface::tags`, for the
//! `surface.*` tags; and the sim's `rng::tags`, for every other stage's. The sim asserts all three
//! disjoint with [`assert_registries_disjoint`], so no two tags anywhere share a name or a hash.
//!
//! # Keying
//!
//! A [`Stream`] is opened with [`Stream::open`]`(`[`Seed`]`, `[`DomainTag`]`, `[`ObjectKey`]`)`,
//! and draws its words from the block function [`threefry2x64_20`]:
//!
//! | Word           | Contents                                                                                                                         |
//! | -------------- | -------------------------------------------------------------------------------------------------------------------------------- |
//! | Key word 0     | The universe seed, or for a surface tag the body's surface or detail seed                                                        |
//! | Key word 1     | The domain tag's hash                                                                                                            |
//! | Counter word 0 | The object's word: a system's raw ID, a body's system's raw ID, a cell word (an ID with its index zeroed), a feature word, 0 or an item number for the galaxy, a surface cell's packed face, level, `i` and `j`, or a surface item's number |
//! | Counter word 1 | `sub << 48 \| block`: `sub` is 0, or the body index for a body, or a surface cell's instance; `block` is the 48-bit draw number   |
//!
//! Word `n` of a stream is output `n & 1` of block `n >> 1`. The generator version is not in the
//! key.
//!
//! A surface tag, of scope [`TagScope::SurfaceCoarse`] or [`TagScope::SurfaceDetail`], is opened
//! by [`SurfaceSeed::stream`] or [`DetailSeed::stream`], each its own scope's alone, and never by
//! [`Stream::open`], so the body's seed takes the universe seed's place and the key names a cell
//! or an item of that body's surface ([`ObjectKey::surface_cell`], [`ObjectKey::surface_item`];
//! plan R09, Design note 2).
//!
//! # Samplers
//!
//! The samplers are hand-written, because a dependency's distributions may change their output in
//! a minor release and a dependency bump must never move a star. Each is a fixed sequence of IEEE
//! operations and [`math`](crate::math) calls on a documented number of words, so its output is
//! part of the generator version:
//!
//! | Sampler                                                   | Words                    |
//! | --------------------------------------------------------- | ------------------------ |
//! | [`uniform`](Stream::uniform) and its open forms, [`uniform_in`](Stream::uniform_in) | 1 |
//! | [`below`](Stream::below)                                  | 1 per attempt, under 2 on average |
//! | [`standard_normal`](Stream::standard_normal), [`standard_normal_pair`](Stream::standard_normal_pair), [`normal`](Stream::normal), [`log_normal`](Stream::log_normal), [`log_normal_dex`](Stream::log_normal_dex) | 2 |
//! | [`poisson`](Stream::poisson) below [`POISSON_PTRS_MIN_MEAN`] | 1 (0 at a mean of 0)  |
//! | [`poisson`](Stream::poisson) from [`POISSON_PTRS_MIN_MEAN`] | 2 per attempt         |
//! | [`power_law`](Stream::power_law)                          | 1                        |
//! | [`PiecewisePowerLaw::sample`], [`PiecewiseLinear::sample`] | 2                       |
//! | [`mark`](Stream::mark), [`decide`](Stream::decide), [`pick`](Stream::pick) | 1       |
//!
//! # Decisions
//!
//! A random decision compares a [`Mark`], the top 53 bits of one word, against a 53-bit
//! [`Threshold`], `ceil(p × 2⁵³)`: exactly `uniform() < p` on the same word, with `p = 1` always
//! accepting. [`Thresholds`] and [`Mark::pick_weighted`] pick a class, or reject, with one mark.
//! This is a convention, not a second line of defence: a threshold is computed from floats, so
//! reproducibility still rests on the pinned `libm`.
//!
//! # Events
//!
//! A system's or body's events in time are keyed in two steps, which [`RawEventKey`] does over
//! plain words: one block under a domain tag of scope [`TagScope::Event`], whose output keys
//! ordinary streams. The sim's `EventKey` forms the words from its ID types.

mod decide;
mod domain_tag;
mod key;
mod raw_event;
mod sample;
mod stream;
mod surface;
pub mod tags;
mod threefry;

pub use decide::{Mark, Threshold, Thresholds};
pub use domain_tag::{
    DomainTag, TagScope, assert_registries_disjoint, assert_tag_names, hash_tag_name,
    is_valid_tag_name,
};
pub use key::{ObjectKey, ParseSeedError, Seed, SurfaceCellKeyError};
pub use raw_event::RawEventKey;
pub use sample::{
    BuildPiecewiseError, BuildPowerLawError, POISSON_MAX_MEAN, POISSON_PTRS_MIN_MEAN,
    PiecewiseLinear, PiecewisePowerLaw, PowerLaw,
};
pub use stream::Stream;
pub use surface::{DetailSeed, SurfaceSeed};
pub use threefry::threefry2x64_20;
