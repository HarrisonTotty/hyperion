//! The DAG on the procedural test meshes (plan R14, T5.a): group borders watertight at every cut,
//! errors monotone and spheres nested, and the same input giving the same bytes, pinned by each
//! mesh's SHA-256 (built on Linux; the same on macOS and Windows is the byte test).

mod common;

use common::{
    assert_cuts_watertight, assert_group_borders_kept, assert_monotone_and_nested, build_twice,
    open_points, sha256, summary,
};
use meshbuild::SourceMesh;
use meshbuild::procedural::{icosphere, terrain, torus_knot};

fn check(name: &str, source: &SourceMesh, expected_sha256: &str) {
    let built = build_twice(source);
    eprintln!("{name}: {}", summary(&built));
    let open = open_points(&built.file);
    assert_monotone_and_nested(&built.file);
    assert_group_borders_kept(&built.file, &open);
    assert_cuts_watertight(&built.file, &open, 48, 24);
    assert_eq!(
        sha256(&built.bytes),
        expected_sha256,
        "{name}'s pages moved"
    );
}

#[test]
fn an_icosphere_with_noise_builds_a_closed_watertight_monotone_dag_of_pinned_bytes() {
    let source = icosphere(6, 10.0, 0.05, 11);
    assert!(
        open_points(&build_twice(&source).file).is_empty(),
        "a closed mesh has no boundary"
    );
    check(
        "icosphere",
        &source,
        "b1ed9b957c51bb7f1a1edd27d408fb2bab83c085d0e2eb440443183965274df6",
    );
}

#[test]
fn a_torus_knot_with_uv_seams_builds_a_watertight_monotone_dag_of_pinned_bytes() {
    check(
        "torus knot",
        &torus_knot(2, 3, 512, 48, 4.0, 0.6),
        "b6d000cc9c6fbce743979d1ca6e5b1a4e30914d14cee60117745147ddb06f787",
    );
}

#[test]
fn a_terrain_patch_with_an_open_boundary_builds_a_watertight_monotone_dag_of_pinned_bytes() {
    check(
        "terrain",
        &terrain(160, 120, 1.0, 12.0, 5),
        "a0c7225fff2f01d8ceee400d5c307d32e39e03241fb1d12ce8ac87a66befb7aa",
    );
}
