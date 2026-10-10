//! The cube sphere's golden file, `tests/golden/cube_sphere.golden` (plan R05, T2).
//!
//! It pins the warp, the vertex directions of 50 fixed patches at levels 0 to 24 on all six faces,
//! their edge and corner neighbours and the finest level of 20 radii, as hexadecimal bits, on
//! native, `wasm32-wasip1` and `wasm32-unknown-unknown`. The client's TypeScript mirror
//! (`view/terrain/cube.ts` and `patchKey.ts`) reads the same file and asserts every value bit for
//! bit (Design note 1).
//!
//! The header carries [`TEST_PLANET_VERSION`], not the generator version: the cube sphere belongs
//! to no universe until R09 (Design note 13). Where this test fails with the testkit's hint to
//! "bump `GENERATOR_VERSION`", read "bump `TEST_PLANET_VERSION`" in `src/lib.rs`, then
//! `just bless`.
//!
//! # Format
//!
//! After the header, one record a line, its fields separated by single spaces, every float as
//! `0x` and its 16 hexadecimal digits of IEEE 754 bits:
//!
//! - `warp <k> <s> <u> <st>`, for k from 0 to 999: s = k ÷ 999, u = `st_to_uv(s)` and
//!   st = `uv_to_st(u)`.
//! - `patch <n> <face> <level> <i> <j> <word>`, for n from 0 to 49: the key's fields in decimal
//!   and its `to_u64` word in hexadecimal; then, for that patch,
//!   - `vertex <x> <y> <dx> <dy> <dz>`, `vertex_dir(x, y)`, for x and y each in 0, 1, 8, 16, 32, 48,
//!     56, 63 and 64, y outer and x inner;
//!   - `digest <hash>`: `hyperion_testkit::golden::f64_digest` over every vertex's direction, all
//!     65 × 65 with y outer and x inner, its x, y and z in turn;
//!   - `edge <edge> <face> <level> <i> <j> <back>`, for the edges `UMin`, `UMax`, `VMin` and
//!     `VMax` in turn: `edge_neighbour_and_back`;
//!   - `corner <c> <face> <level> <i> <j>` or `corner <c> none`, for c from 0 to 3:
//!     `corner_neighbours`.
//! - `table <face> <level> <i> <j> <edge> <face> <level> <i> <j> <back>`: for each face and
//!   each of its edges in turn, a level-3 patch on that edge at an off-centre cell, its neighbour
//!   across the edge and the edge back, so that all 24 face crossings are pinned.
//! - `finest <radius> <level> <max_spacing>`: `finest_level(radius)` in decimal and
//!   `vertex_spacing(radius, level).max_m`, for 20 radii from 100 km to 70,000 km.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_base::math;
use hyperion_surface::TEST_PLANET_VERSION;
use hyperion_surface::cube::{Edge, Face, MAX_LEVEL, PATCH_QUADS, PatchKey, st_to_uv, uv_to_st};
use hyperion_surface::geometry::{finest_level, vertex_spacing};
use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, f64_digest};
use hyperion_testkit::lcg::Lcg;

/// The vertices printed in full, along each axis of a patch.
const PRINTED: [u8; 9] = [0, 1, 8, 16, 32, 48, 56, 63, 64];

/// The 50 patches: two at each level from 0 to 24, on faces in turn, at a cube corner, along a
/// face edge or inside the face in turn, the free indices drawn by a fixed test generator.
fn patches() -> Vec<PatchKey> {
    let mut rng = Lcg::new(0x6375_6265);
    (0_u8..50)
        .map(|n| {
            let level = n % (MAX_LEVEL + 1);
            let face = Face::from_index(n % 6).expect("n % 6 is a face");
            let last = (1_u32 << level) - 1;
            let mut free =
                || u32::try_from(rng.next_below(u64::from(last) + 1)).expect("an index fits a u32");
            let (i, j) = match n % 4 {
                0 => (last, 0),
                1 => (last, free()),
                2 => (free(), 0),
                _ => (free(), free()),
            };
            PatchKey::new(face, level, i, j).expect("the indices are within the level")
        })
        .collect()
}

fn hex(value: f64) -> String {
    format!("0x{:016x}", bits(value))
}

fn key_fields(key: PatchKey) -> String {
    format!(
        "{} {} {} {}",
        key.face().index(),
        key.level(),
        key.i(),
        key.j()
    )
}

fn edge_name(edge: Edge) -> &'static str {
    match edge {
        Edge::UMin => "UMin",
        Edge::UMax => "UMax",
        Edge::VMin => "VMin",
        Edge::VMax => "VMax",
    }
}

#[test]
fn the_cube_sphere_matches_its_golden() {
    let mut w = GoldenWriter::new();
    w.header(TEST_PLANET_VERSION);

    for k in 0_u32..1000 {
        let s = f64::from(k) / 999.0;
        let u = st_to_uv(s);
        w.line(&format!(
            "warp {k} {} {} {}",
            hex(s),
            hex(u),
            hex(uv_to_st(u))
        ));
    }

    let quads = u8::try_from(PATCH_QUADS).expect("64 fits a u8");
    for (n, key) in (0_u32..).zip(patches()) {
        w.line(&format!(
            "patch {n} {} 0x{:016x}",
            key_fields(key),
            key.to_u64()
        ));
        for &y in &PRINTED {
            for &x in &PRINTED {
                let [dx, dy, dz] = key.vertex_dir(x, y);
                w.line(&format!(
                    "vertex {x} {y} {} {} {}",
                    hex(dx),
                    hex(dy),
                    hex(dz)
                ));
            }
        }
        let all: Vec<f64> = (0..=quads)
            .flat_map(|y| (0..=quads).flat_map(move |x| key.vertex_dir(x, y)))
            .collect();
        w.line(&format!("digest 0x{:016x}", f64_digest(&all)));
        for edge in Edge::ALL {
            let (neighbour, back) = key.edge_neighbour_and_back(edge);
            w.line(&format!(
                "edge {} {} {}",
                edge_name(edge),
                key_fields(neighbour),
                edge_name(back)
            ));
        }
        for (c, corner) in (0_u32..).zip(key.corner_neighbours()) {
            match corner {
                Some(neighbour) => w.line(&format!("corner {c} {}", key_fields(neighbour))),
                None => w.line(&format!("corner {c} none")),
            }
        }
    }

    // Every face's four edges, at level 3 from an off-centre cell on each, so that all 24
    // directed face crossings and their orientations are pinned.
    let last = 7;
    for face in Face::ALL {
        for (edge, i, j) in [
            (Edge::UMin, 0, 1),
            (Edge::UMax, last, 1),
            (Edge::VMin, 1, 0),
            (Edge::VMax, 1, last),
        ] {
            let key = PatchKey::new(face, 3, i, j).expect("a level-3 cell");
            let (neighbour, back) = key.edge_neighbour_and_back(edge);
            w.line(&format!(
                "table {} {} {} {}",
                key_fields(key),
                edge_name(edge),
                key_fields(neighbour),
                edge_name(back)
            ));
        }
    }

    for k in 0_u32..20 {
        let radius = 1.0e5 * math::powf(700.0, f64::from(k) / 19.0);
        let level = finest_level(radius);
        let spacing = vertex_spacing(radius, level).max_m;
        w.line(&format!("finest {} {level} {}", hex(radius), hex(spacing)));
    }

    golden!("cube_sphere", &w.finish());
}

/// What reads the golden directory, which only a native run can (plan R04, Design note 12).
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::path::Path;

    use hyperion_surface::TEST_PLANET_VERSION;

    /// Every golden file directly in the crate's golden directory carries
    /// [`TEST_PLANET_VERSION`] in its header, whichever test writes it; R09's, in its
    /// subdirectories, carry `GENERATOR_VERSION` and are not read here.
    #[test]
    fn every_golden_file_carries_the_test_planet_version() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden");
        let expected = format!("# generator_version = {TEST_PLANET_VERSION}");
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("the golden directory is readable") {
            let path = entry.expect("a directory entry is readable").path();
            if path.extension().is_some_and(|e| e == "golden") {
                let text = std::fs::read_to_string(&path).expect("a golden file is readable");
                assert_eq!(
                    text.lines().next(),
                    Some(expected.as_str()),
                    "{}",
                    path.display()
                );
                seen += 1;
            }
        }
        assert!(seen > 0, "no golden files in {}", dir.display());
    }
}
