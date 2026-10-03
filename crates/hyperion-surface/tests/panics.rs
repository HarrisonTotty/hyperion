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
