//! The authoritative ground on the reference worlds (rendering plan R10, T3): `ground_at` is
//! finite at every sampled point of each, its normal a unit vector pointing out of the body, and
//! at a vertex it is the synthesis's height there, bit for bit.
//!
//! The worlds are R09's reference Earth, Mars, Moon and Ceres
//! (`planetary::surface::reference`), each run through the coarse pass once per binary, under the
//! sim's `testing` feature. Until R09.T13.e–T16 fill the rest of the pass's steps they come out as
//! the steps built so far leave them, the plates, the coarse elevation and the coarse craters of
//! R09.T11–T12.d, scaled to their `σ_h` with the reference Earth's sea placed by R09.T12.e, under
//! R09.T13.a–d's climate, with the Earth's and Mars's ices placed by the climate step, which
//! exercises the whole function, its finest level and the point's foot on each figure. R09.T16's
//! `sample_worlds()` (`tests/common/surface_worlds.rs`, with its icy moon and locked world) takes
//! this list's place when it lands, under the same task ID.
#![cfg(feature = "testing")]

use std::sync::OnceLock;

use hyperion_base::rng::DetailSeed;
use hyperion_sim::planetary::surface::{CoarseInputs, coarse_pass, reference};
use hyperion_surface::cube::{Face, PatchKey, unit_dir};
use hyperion_surface::field::{CoarseField, FieldView};
use hyperion_surface::geometry::finest_level;
use hyperion_surface::ground::ground_at;
use hyperion_surface::synth::{BandLevel, SynthCache, Synthesiser};
use hyperion_testkit::float::bits;
use hyperion_testkit::lcg::Lcg;

/// The detail seed the reference worlds' ground is drawn with.
const DETAIL: DetailSeed = DetailSeed::new(0x6772_6f75_6e64_7266);

/// A reference world's name and inputs.
type World = (&'static str, fn() -> CoarseInputs);

/// The reference worlds, in the order R09's records list them.
const WORLDS: [World; 4] = [
    ("Earth-like", reference::earth_like),
    ("Mars-like", reference::mars_like),
    ("Moon-like", reference::moon_like),
    ("Ceres-like", reference::ceres_like),
];

/// The coarse field of world `n`, run through the coarse pass once per binary.
fn field(n: usize) -> &'static CoarseField {
    static FIELDS: [OnceLock<CoarseField>; 4] = [const { OnceLock::new() }; 4];
    FIELDS[n].get_or_init(|| coarse_pass(reference::SEED, &(WORLDS[n].1)()))
}

#[test]
fn the_ground_is_finite_on_every_reference_world() {
    let mut rng = Lcg::new(0x6772_6f75_6e64_7277);
    for (n, (name, _)) in WORLDS.iter().enumerate() {
        let field = field(n);
        let figure = field.header().figure();
        let finest = finest_level(figure.equatorial_radius_m);
        let synth = Synthesiser::new(field, DETAIL);
        let mut cache = SynthCache::new();
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
}
