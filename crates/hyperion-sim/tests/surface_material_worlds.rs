//! The authoritative material on the reference worlds (rendering plan R10, T1.b): `material_at`
//! answers at every sampled point of each, its entry is one of the body's palette, and the airless
//! Moon's and Ceres's ground is their crusts' regolith, or bedrock where a small crater's wall is
//! steeper than the cliff slope; and the surface crate's copy of the sim's thin-air threshold is
//! the sim's.
//!
//! The worlds are R09's reference Earth, Mars, Moon and Ceres
//! (`planetary::surface::reference`), each run through the coarse pass once per binary, under the
//! sim's `testing` feature. Until R09.T13–T16 fill the rest of the pass's steps they come out as
//! the steps built so far leave them, the plates, the coarse elevation and the coarse craters of
//! R09.T11–T12.d, scaled to their `σ_h` with the reference Earth's sea placed by R09.T12.e, with no
//! ice, on which the synthesis draws its own relief and small craters, which exercises the
//! classifier's reads of every record, the reference Earth's sea among them, but not its ice. The
//! plan's two cases on R09.T16's `sample_worlds()` (`tests/common/surface_worlds.rs`, with its icy
//! moon and locked world), that a warm pole has no ice cap and that a locked world's ice lies where
//! R09's field puts it, follow R09.T16 under the same task ID, and that list takes this one's
//! place. Until then the classifier's rules are held on the surface crate's synthetic worlds and
//! built fields (`material::classify`'s tests).
#![cfg(feature = "testing")]

use std::sync::OnceLock;

use hyperion_base::rng::DetailSeed;
use hyperion_sim::planetary::derive::photometry;
use hyperion_sim::planetary::surface::{CoarseInputs, coarse_pass, reference};
use hyperion_surface::cube::{unit_dir, xyz_to_face_uv};
use hyperion_surface::field::{CoarseField, FieldView};
use hyperion_surface::material::{MaterialClass, THIN_ATMOSPHERE_PRESSURE, material_at};
use hyperion_surface::synth::{SynthCache, Synthesiser};
use hyperion_testkit::float::bits;
use hyperion_testkit::lcg::Lcg;

/// The detail seed the reference worlds' ground is drawn with.
const DETAIL: DetailSeed = DetailSeed::new(0x6d61_7465_7269_616c);

/// A reference world's name, its inputs and whether it is airless.
type World = (&'static str, fn() -> CoarseInputs, bool);

/// The reference worlds, in the order R09's records list them.
const WORLDS: [World; 4] = [
    ("Earth-like", reference::earth_like, false),
    ("Mars-like", reference::mars_like, false),
    ("Moon-like", reference::moon_like, true),
    ("Ceres-like", reference::ceres_like, true),
];

/// The coarse field of world `n`, run through the coarse pass once per binary.
fn field(n: usize) -> &'static CoarseField {
    static FIELDS: [OnceLock<CoarseField>; 4] = [const { OnceLock::new() }; 4];
    FIELDS[n].get_or_init(|| coarse_pass(reference::SEED, &(WORLDS[n].1)()))
}

#[test]
fn the_material_is_named_on_every_reference_world() {
    let mut rng = Lcg::new(0x6d61_7465_7269_6177);
    for (n, (name, _, airless)) in WORLDS.iter().enumerate() {
        let field = field(n);
        let header = field.header();
        let palette = header.palette();
        assert_eq!(
            header.surface_pressure().value() <= 0.0,
            *airless,
            "{name}'s air"
        );
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
            let ground = material_at(&synth, &mut cache, xyz_to_face_uv(dir))
                .unwrap_or_else(|e| panic!("{name} {dir:?}: {e}"));
            if *airless {
                assert!(
                    matches!(
                        ground.class(),
                        MaterialClass::Regolith | MaterialClass::Bedrock
                    ),
                    "{name} {dir:?}: {ground:?} on an airless body's dry ground"
                );
            }
            let Some(entry) = ground.entry() else {
                assert!(
                    palette.is_empty(),
                    "{name} {dir:?}: no entry on a modelled body"
                );
                continue;
            };
            let entry = palette
                .get(entry)
                .unwrap_or_else(|| panic!("{name} {dir:?}: {ground:?} past the palette"));
            if *airless {
                assert!(entry.role.is_crust(), "{name} {dir:?}: {entry:?}");
            }
        }
    }
}

/// The surface crate repeats the sim's thin-air threshold, which it cannot see; the two are one.
#[test]
fn the_thin_air_threshold_is_the_sim_s() {
    assert_eq!(
        bits(THIN_ATMOSPHERE_PRESSURE.value()),
        bits(photometry::THIN_ATMOSPHERE_PRESSURE.value())
    );
}
