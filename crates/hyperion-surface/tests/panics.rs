//! Every test of `hyperion-surface` that expects a panic, in a test binary of its own.
//!
//! On `wasm32-unknown-unknown` a panic is a trap, after which every later test of the same binary
//! runs in a best-effort state, so these tests are kept apart from the goldens and the unit tests
//! (plan R04, Design note 12). Each states `expected`, because on that target a bare
//! `should_panic` passes on any trap.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_surface::cube::{unit_dir, xyz_to_face_uv};

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
