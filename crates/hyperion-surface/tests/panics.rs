//! Every test of `hyperion-surface` that expects a panic, in a test binary of its own.
//!
//! On `wasm32-unknown-unknown` a panic is a trap, after which every later test of the same binary
//! runs in a best-effort state, so these tests are kept apart from the goldens and the unit tests
//! (plan R04, Design note 12). Each states `expected`, because on that target a bare
//! `should_panic` passes on any trap.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_surface::cube::{Face, MAX_LEVEL, PatchKey, unit_dir, xyz_to_face_uv};
use hyperion_surface::geometry::{finest_level, vertex_spacing};
use hyperion_surface::noise::{LatticeCache, Octave, gradient_noise};
use hyperion_surface::num;
use hyperion_surface::patch::vertex::{PatchTerms, face_difference_morph_f32};
use hyperion_surface::patch::{BakeOptions, NormalScale, VertexPath, bake_patch};
use hyperion_surface::spheroid::Spheroid;
use hyperion_surface::test_planet::{TEST_PLANET, octaves};

const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn octave() -> Octave {
    Octave::new(3, 100.0, IDENTITY, [0.0; 3], hyperion_base::Seed::new(1))
}

#[test]
#[should_panic(expected = "a direction must be finite and non-zero")]
fn the_zero_vector_has_no_face() {
    let _ = xyz_to_face_uv([0.0, -0.0, 0.0]);
}

#[test]
#[should_panic(expected = "a direction must be finite and non-zero")]
fn a_nan_vector_has_no_face() {
    let _ = xyz_to_face_uv([f64::NAN, 1.0, 0.0]);
}

#[test]
#[should_panic(expected = "a direction must be finite and non-zero")]
fn the_zero_vector_has_no_unit_direction() {
    let _ = unit_dir([0.0, 0.0, 0.0]);
}

#[test]
#[should_panic(expected = "has no children")]
fn a_patch_at_the_maximum_level_has_no_children() {
    let _ = PatchKey::new(Face::PosZ, MAX_LEVEL, 0, 0)
        .unwrap()
        .children();
}

#[test]
#[should_panic(expected = "is outside a patch of 64 quads")]
fn a_vertex_beyond_the_patch_is_refused() {
    let _ = PatchKey::root(Face::PosX).vertex_dir(65, 0);
}

#[test]
#[should_panic(expected = "a body's radius must be finite and positive")]
fn a_zero_radius_has_no_finest_level() {
    let _ = finest_level(0.0);
}

#[test]
#[should_panic(expected = "is above the maximum")]
fn spacing_above_the_maximum_level_is_refused() {
    let _ = vertex_spacing(6.371e6, MAX_LEVEL + 1);
}

#[test]
#[should_panic(expected = "min of a NaN")]
fn min_refuses_a_nan() {
    let _ = num::min(f64::NAN, 1.0);
}

#[test]
#[should_panic(expected = "max of a NaN")]
fn max_refuses_a_nan() {
    let _ = num::max(1.0, f64::NAN);
}

#[test]
#[should_panic(expected = "a height must be finite")]
fn assert_finite_refuses_infinity() {
    let _ = num::assert_finite(f64::INFINITY);
}

#[test]
#[should_panic(expected = "an octave index is 0 to 31")]
fn an_octave_index_above_31_is_refused() {
    let _ = Octave::new(32, 1.0, IDENTITY, [0.0; 3], hyperion_base::Seed::new(1));
}

#[test]
#[should_panic(expected = "an octave's rotation must be orthonormal")]
fn a_rotation_that_is_not_orthonormal_is_refused() {
    let stretched = [[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let _ = Octave::new(3, 1.0, stretched, [0.0; 3], hyperion_base::Seed::new(1));
}

#[test]
#[should_panic(expected = "a covered ball must be finite")]
fn a_cover_of_an_infinite_ball_is_refused() {
    LatticeCache::new().cover(&octave(), [0.0; 3], f64::INFINITY);
}

#[test]
#[should_panic(expected = "beyond the key's 32 bits")]
fn a_point_beyond_the_keys_reach_is_refused() {
    let _ = gradient_noise([1e15, 0.0, 0.0], &octave(), &mut LatticeCache::new());
}

#[test]
#[should_panic(expected = "is beyond the finest")]
fn an_octave_beyond_the_finest_has_no_spacing() {
    let _ = octaves::spacing_m(octaves::FINEST_OCTAVE + 1);
}

#[test]
#[should_panic(expected = "a skirt margin must be finite and non-negative")]
fn a_negative_skirt_margin_is_refused() {
    let opts = BakeOptions {
        vertex_path: VertexPath::BakedOffsets,
        normals: NormalScale::Mesh,
        skirt_m: -1.0,
    };
    let _ = bake_patch(
        &TEST_PLANET,
        PatchKey::root(Face::PosX),
        &opts,
        &mut LatticeCache::new(),
    );
}

#[test]
#[should_panic(expected = "is outside a patch")]
fn a_morph_target_beyond_the_patch_is_refused() {
    let terms = PatchTerms::new(PatchKey::root(Face::PosX), &Spheroid::WGS84, 0.0).narrow();
    let _ = face_difference_morph_f32(&terms, 65, 1, |_, _| 0.0);
}

#[test]
#[should_panic(expected = "a sphere's radius must be finite and positive")]
fn a_sphere_of_no_radius_is_refused() {
    let _ = Spheroid::sphere(0.0);
}
