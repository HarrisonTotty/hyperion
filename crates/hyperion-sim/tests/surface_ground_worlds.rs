//! The authoritative ground on the sample worlds (rendering plan R10, T3): `ground_at` is finite
//! at every sampled point of each, its normal a unit vector pointing out of the body, and at a
//! vertex it is the synthesis's height there, bit for bit.
//!
//! On each world the ground's `height()` and the point's own `point_height()` are finite at 64
//! random directions and three fixed ones (a cube corner `(1, 1, −1)`, a face edge's `(0, 1, 1)`
//! and the pole `(0, 0, 1)`), each at a height drawn from −500 m to 5 km. There the normal is a
//! unit vector to 10⁻¹⁴ in |n|² and points out of the body, against the figure's normal at the
//! point's foot. At one finest-level vertex the ground equals `Synthesiser::height_at` at the
//! finest level, bit for bit.
//!
//! The worlds are R09.T16's `sample_worlds()` (`tests/common/surface_worlds.rs`): the reference
//! Earth, Mars, Moon and Ceres, the icy moon `europa_like` and the locked `locked_earth_like`, each
//! run through the whole coarse pass under `reference::SEED`, behind the sim's `testing` feature.
//! Under nextest each test runs in a process of its own, so each test pays for the passes of the
//! worlds it reads. The four `Tier::Fast` worlds share one test, and each `Tier::Slow` world, the
//! reference Earth and the locked Earth, has its own, ignored. The slow two run with
//! `cargo nextest run -p hyperion-sim --features testing --test surface_ground_worlds
//! --run-ignored only`.
//!
//! A world's points come from a sampler seeded by its name ([`sampler`]), so they do not depend
//! on which test runs the world, the order the worlds run in or the world's place in the list.
#![cfg(feature = "testing")]

#[expect(
    dead_code,
    reason = "the ground's tests use only the sample worlds of the shared helpers"
)]
mod common;

use common::surface_worlds::{SampleWorld, Tier, sample_world, sample_worlds};
use hyperion_base::rng::DetailSeed;
use hyperion_surface::cube::{Face, PatchKey, unit_dir};
use hyperion_surface::field::FieldView;
use hyperion_surface::geometry::finest_level;
use hyperion_surface::ground::ground_at;
use hyperion_surface::synth::{BandLevel, SynthCache, Synthesiser};
use hyperion_testkit::float::bits;
use hyperion_testkit::lcg::Lcg;

/// The detail seed the sample worlds' ground is drawn with.
const DETAIL: DetailSeed = DetailSeed::new(0x6772_6f75_6e64_7266);

/// The seed of the sample points, into which each world's name is folded ([`sampler`]).
const SAMPLES: u64 = 0x6772_6f75_6e64_7277;

/// The FNV-1a 64 prime (Fowler, Noll and Vo), the testkit's `f64_digest`'s, which folds a world's
/// name into [`SAMPLES`].
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The reference Earth, a slow world with a test of its own.
const EARTH: &str = "earth_like";

/// The locked Earth, a slow world with a test of its own.
const LOCKED_EARTH: &str = "locked_earth_like";

/// The slow worlds, each with a test of its own.
const SLOW: [&str; 2] = [EARTH, LOCKED_EARTH];

/// The sampler of the world named `name`: an [`Lcg`] seeded with FNV-1a 64 over the name's bytes,
/// from [`SAMPLES`] in place of the offset basis.
fn sampler(name: &str) -> Lcg {
    Lcg::new(name.bytes().fold(SAMPLES, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(FNV_PRIME)
    }))
}

/// Asserts that the ground on `world` is finite at every sampled point, that its normal there is
/// an outward unit vector, and that at a finest-level vertex it is the synthesis's height there.
fn assert_the_ground_holds(world: &SampleWorld) {
    let name = world.name();
    let field = world.field();
    let figure = field.header().figure();
    let finest = finest_level(figure.equatorial_radius_m);
    let synth = Synthesiser::new(field, DETAIL);
    let mut cache = SynthCache::new();
    let mut rng = sampler(name);
    let mut dirs: Vec<[f64; 3]> = (0..64)
        .map(|_| unit_dir([0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0)))
        .collect();
    dirs.extend([
        unit_dir([1.0, 1.0, -1.0]),
        unit_dir([0.0, 1.0, 1.0]),
        [0.0, 0.0, 1.0],
    ]);
    for dir in dirs {
        let p = figure.surface_point(dir, rng.next_f64() * 5_500.0 - 500.0);
        let ground = ground_at(&synth, &mut cache, &p).unwrap();
        let n = ground.normal();
        assert!(ground.height().value().is_finite(), "{name} {dir:?}");
        assert!(ground.point_height().value().is_finite(), "{name} {dir:?}");
        let len2 = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
        assert!((len2 - 1.0).abs() < 1e-14, "{name} {dir:?}: |n|² = {len2}");
        let up = figure.normal(ground.dir());
        assert!(
            n[0] * up[0] + n[1] * up[1] + n[2] * up[2] > 0.0,
            "{name} {dir:?}: the normal points into the body"
        );
    }
    // At a vertex the ground is the synthesis's height there at the finest level.
    let key = PatchKey::new(Face::NegZ, finest, 1 << (finest - 2), 3).unwrap();
    let vertex = key.vertex_dir(40, 9);
    let expected = synth
        .height_at(&mut cache, vertex, BandLevel::new(finest).unwrap())
        .unwrap()
        .height_m;
    let ground = ground_at(&synth, &mut cache, &figure.surface_point(vertex, 120.0)).unwrap();
    assert_eq!(bits(ground.height().value()), bits(expected), "{name}");
}

/// The ground holds on Mars, the Moon, Ceres and the Europa; and every sample world runs in
/// exactly one test, these four here and each slow world in its own.
#[test]
fn the_ground_holds_on_every_fast_sample_world() {
    let fast: Vec<&SampleWorld> = sample_worlds()
        .iter()
        .filter(|world| world.tier() == Tier::Fast)
        .collect();
    assert_eq!(fast.len(), 4, "Mars, the Moon, Ceres and the Europa");
    let mut covered: Vec<&str> = fast
        .iter()
        .copied()
        .map(SampleWorld::name)
        .chain(SLOW)
        .collect();
    let mut names: Vec<&str> = sample_worlds().iter().map(SampleWorld::name).collect();
    covered.sort_unstable();
    names.sort_unstable();
    assert_eq!(
        covered, names,
        "every sample world runs in exactly one test: the fast ones here, each slow one in its own"
    );
    for world in fast {
        assert_the_ground_holds(world);
    }
}

#[test]
#[ignore = "slow: the reference Earth's coarse pass at level 8, about 100 s in the test profile"]
fn the_ground_holds_on_the_earth_like_sample_world() {
    let earth = sample_world(EARTH);
    assert_eq!(earth.tier(), Tier::Slow);
    assert_the_ground_holds(earth);
}

#[test]
#[ignore = "slow: the reference locked Earth's coarse pass at level 8, about 22 s in the test \
            profile"]
fn the_ground_holds_on_the_locked_earth_like_sample_world() {
    let locked = sample_world(LOCKED_EARTH);
    assert_eq!(locked.tier(), Tier::Slow);
    assert_the_ground_holds(locked);
}
