//! The DAG on the procedural test meshes (plan R14, T5.a): group borders watertight at every cut,
//! errors monotone and spheres nested, and the same input giving the same bytes, pinned by each
//! mesh's SHA-256 (built on Linux; the same on macOS and Windows is the byte test).

mod common;

use common::{
    assert_cuts_watertight, assert_group_borders_kept, assert_group_table,
    assert_monotone_and_nested, assert_root_pages, build_twice, open_points, sha256, summary,
};
use meshbuild::SourceMesh;
use meshbuild::procedural::TestMesh;

fn check(name: &str, source: &SourceMesh, expected_sha256: &str) {
    let built = build_twice(source);
    eprintln!("{name}: {}", summary(&built));
    let open = open_points(&built.file);
    assert_monotone_and_nested(&built.file);
    assert_group_table(&built.file);
    assert_root_pages(&built.file);
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
    let source = TestMesh::Icosphere.source();
    assert!(
        open_points(&build_twice(&source).file).is_empty(),
        "a closed mesh has no boundary"
    );
    check(
        "icosphere",
        &source,
        "c3c366d48325247707d5cb2eb4aa31177fd508e55fa81c68229ec86316ffbec2",
    );
}

#[test]
fn a_torus_knot_with_uv_seams_builds_a_watertight_monotone_dag_of_pinned_bytes() {
    check(
        "torus knot",
        &TestMesh::TorusKnot.source(),
        "05ec4ce530e1df080214ad928c527c04d4dd52a24d05520f656fabb935668265",
    );
}

#[test]
fn a_terrain_patch_with_an_open_boundary_builds_a_watertight_monotone_dag_of_pinned_bytes() {
    check(
        "terrain",
        &TestMesh::Terrain.source(),
        "2d49bb07d29002da826c27e273dc70035c5ecf8c79965b303ef3bd4ead5d9ae2",
    );
}
