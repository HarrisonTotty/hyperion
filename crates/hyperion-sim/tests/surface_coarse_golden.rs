//! R10's sample worlds are the fields R09.T16's coarse goldens pin (plan R09, T16): each world of
//! `common::surface_worlds::sample_worlds()` gives the payload digest of its golden,
//! `tests/golden/planetary/surface/<world>.golden`, which the sim's library tests write
//! (`planetary::surface::coarse_golden`, whose documentation says why they live there).
//!
//! The digest is computed here as the library computes it: the whole field's payload, every cell
//! at the finest code, each block's generator version (bytes 10 to 13) zeroed, and FNV-1a 64 over
//! its bytes as `f64`s (the testkit's `f64_digest`, through a table of the 256 bytes' bits). The
//! library's writer is `cfg(test)`, which an integration test cannot reach, so the few lines are
//! repeated, and a change to either breaks this test at once.
#![cfg(feature = "testing")]

#[expect(
    dead_code,
    reason = "the coarse goldens use only the sample worlds of the shared helpers"
)]
mod common;

use std::collections::BTreeSet;

use common::surface_worlds::{SampleWorld, Tier, sample_worlds};
use hyperion_surface::field::{CoarseField, Cover, CoverRange, FieldView, ResolutionCode};
use hyperion_surface::wire::encode_payload;
use hyperion_testkit::float::bits;

/// The bytes of a payload block before its generator version: its magic, its format and its
/// length (the surface crate's `wire` frame).
const VERSION_AT: usize = 10;

/// The FNV-1a 64 offset basis, the testkit's `f64_digest`'s.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// The FNV-1a 64 prime, the testkit's `f64_digest`'s.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The goldens, by world name.
const GOLDENS: [(&str, &str); 6] = [
    (
        "earth_like",
        include_str!("golden/planetary/surface/earth_like.golden"),
    ),
    (
        "mars_like",
        include_str!("golden/planetary/surface/mars_like.golden"),
    ),
    (
        "moon_like",
        include_str!("golden/planetary/surface/moon_like.golden"),
    ),
    (
        "ceres_like",
        include_str!("golden/planetary/surface/ceres_like.golden"),
    ),
    (
        "europa_like",
        include_str!("golden/planetary/surface/europa_like.golden"),
    ),
    (
        "locked_earth_like",
        include_str!("golden/planetary/surface/locked_earth_like.golden"),
    ),
];

/// The payload digest of `field` as the coarse goldens write it.
fn payload_digest(field: &CoarseField) -> u64 {
    let count = field.header().level().cell_count();
    let cover =
        Cover::from_ranges([CoverRange::new(0, count, ResolutionCode::FINEST).unwrap()]).unwrap();
    let mut bytes = encode_payload(field, &cover, None);
    let mut at = 0;
    while at < bytes.len() {
        let length = u32::from_le_bytes(bytes[at + 6..at + 10].try_into().unwrap());
        bytes[at + VERSION_AT..at + VERSION_AT + 4].fill(0);
        at += usize::try_from(length).unwrap();
    }
    assert_eq!(at, bytes.len(), "the blocks tile the payload");
    let table: Vec<[u8; 8]> = (0..=u8::MAX)
        .map(|b| bits(f64::from(b)).to_le_bytes())
        .collect();
    bytes.iter().fold(FNV_OFFSET, |hash, &b| {
        table[usize::from(b)]
            .iter()
            .fold(hash, |h, &x| (h ^ u64::from(x)).wrapping_mul(FNV_PRIME))
    })
}

/// The `payload digest` line of world `name`'s golden.
fn golden_digest(name: &str) -> u64 {
    let (_, text) = GOLDENS
        .iter()
        .find(|(world, _)| *world == name)
        .unwrap_or_else(|| panic!("{name} has a coarse golden"));
    let hex = text
        .lines()
        .find_map(|line| line.strip_prefix("payload digest = 0x"))
        .unwrap_or_else(|| panic!("{name}'s golden has a payload digest"));
    u64::from_str_radix(hex, 16).unwrap()
}

/// The sample world named `name`.
fn world(name: &str) -> &'static SampleWorld {
    sample_worlds()
        .iter()
        .find(|world| world.name() == name)
        .unwrap_or_else(|| panic!("{name} is a sample world"))
}

/// Asserts that the sample world `world` gives its golden's payload digest.
fn assert_pinned(world: &SampleWorld) {
    assert_eq!(
        payload_digest(world.field()),
        golden_digest(world.name()),
        "{}'s field is its golden's",
        world.name()
    );
}

/// The worlds' names are unique, each has a golden, and every fast world gives its golden's
/// payload digest.
#[test]
fn every_fast_sample_world_is_the_one_its_golden_pins() {
    let names: BTreeSet<&str> = sample_worlds().iter().map(SampleWorld::name).collect();
    assert_eq!(names.len(), sample_worlds().len(), "the names are unique");
    assert_eq!(
        names,
        GOLDENS.iter().map(|(name, _)| *name).collect(),
        "each world has a golden"
    );
    let fast: Vec<&SampleWorld> = sample_worlds()
        .iter()
        .filter(|world| world.tier() == Tier::Fast)
        .collect();
    assert_eq!(fast.len(), 4, "Mars, the Moon, Ceres and Europa");
    for world in fast {
        assert_pinned(world);
    }
}

#[test]
#[ignore = "slow: the reference Earth's coarse pass at level 8, about 100 s in the test profile"]
fn the_earth_like_sample_world_is_the_one_its_golden_pins() {
    let earth = world("earth_like");
    assert_eq!(earth.tier(), Tier::Slow);
    assert_pinned(earth);
}

#[test]
#[ignore = "slow: the reference locked Earth's coarse pass at level 8, about 22 s in the test \
            profile"]
fn the_locked_earth_like_sample_world_is_the_one_its_golden_pins() {
    let locked = world("locked_earth_like");
    assert_eq!(locked.tier(), Tier::Slow);
    assert_pinned(locked);
}
