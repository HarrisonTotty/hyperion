//! The worker's golden files, blessed natively and compared on `wasm32-wasip1` and
//! `wasm32-unknown-unknown` through the testkit's embedded arm: `tests/golden/worker/moon_like.golden`
//! (plan R10, T6.b), a fixture field's payload blocks and what [`WorkerFields`] answers over them,
//! `tests/golden/worker/terrain_test.golden` (R10.T16.d; decision-r10-t16-contracts §4), the
//! payloads of the client's kept scene `TERRAIN TEST` with the answers R09.T19.b gives with them,
//! `tests/golden/worker/earth_like.golden` (R10.T12), an Earth-like survey whose field holds a
//! finest-level patch of three classes, with the worker's material at each of its vertices,
//! `tests/golden/worker/terrain_test_weathered.golden` (R10.T1.c's client half;
//! decision-r10-t1c-client-scope §3), the same scene's payloads over `TEST MOON` weathered, with its
//! maturity's digests, and `tests/golden/worker/maturity_layer.golden` (the same ruling's §4), the
//! maturity layer, the albedo nodes and the class map's maturity on two weathered worlds.
//!
//! The client's worker test (`apps/hyperion/.../workers/fieldsWasm.test.ts`) reads the first: it
//! posts the blocks to the client's module, through the worker's own answers, and asks the module's
//! entry points what this file pins, so that the module the client ships answers what the native
//! build answers, bit for bit (T6.b's "each reading entry point equals the native function on a
//! fixture field").
//!
//! The second is `TERRAIN TEST`'s field: the client's fixture link answers `surface_field` from its
//! payloads, its header lines and covers (`view/scenes/terrainGolden.ts`), and the scene takes
//! `TEST MOON`'s ID, detail seed and figure from its lines. **The server crate's test reads it
//! too** (`hyperion-server`'s `requests::surface` tests, "the terrain test golden"), holding its
//! header lines to `convert::surface::surface_header` and its covers to the cells
//! `requests::surface` sends, since the fixture stands in for the server's answers: moving or
//! renaming the file moves that test's path with it (decision-r10-t16-contracts §4).
//!
//! The answers are presentation, as R10.T6.a's bake goldens are: a change to them alone re-blesses
//! `worker/` at the version in force with no bump, and a change to R09's heights or closed forms,
//! or to the classifier, re-blesses it with theirs; the terrain test's file is re-blessed by the
//! same rule as the Moon-like's.
//!
//! # The Moon-like fixture's format
//!
//! After the header:
//!
//! - `body <system>.<index>` and `seed <seed>`, the body ID's wire form and the detail seed's, 16
//!   lowercase hexadecimal digits each;
//! - `block <payload> <index> <bytes>` for each block in the order posted, the first payload's (1)
//!   first, its bytes in lowercase hexadecimal;
//! - `change <ordinal> <runs>`, the [`SurveyChange`] of the block posted `ordinal`th from 0, each run
//!   `<first>-<past last>`, or `-` for none, then `held <ordinal> <runs>`, its
//!   [`held_runs`](SurveyChange::held_runs) alike (R10.T8.b);
//! - `level <when> <n> <bits> <bits> <bits> <bits>`, the level table's row n after the first block
//!   (`first`) and after the last (`last`);
//! - `level_sigma <when> <bits ×25>`, the body's σ by level ([`WorkerFields::level_sigma_m`],
//!   R10.T4.b) after the first block and after the last, alike;
//! - for each bake, `patch <name> <face> <level> <i> <j> <vertex path> <normals> <weights>`, then
//!   R10.T6.a's lines (`tests/golden/terrain_bake/`'s), then `unsurveyed <indices>` or
//!   `unsurveyed -`;
//! - for two of those bakes with the horizon map (R10.T8.b), `horizon <name> <face> <level> <i>
//!   <j>`, the digests `near` and `far` of the map's words (each as an `f64`), `horizon_complete
//!   <bool>`, and `refused <indices>` or `refused -`;
//! - for each reading, `reading <face> <u bits> <v bits> <baseline bits>`, then `none` or its
//!   values' bits and `resolved <bool>`;
//! - for each line of sight, `slant <origin bits ×3> <direction bits ×3> <reach bits>`, then
//!   `no_ground`, `not_surveyed`, or `ground`, its range's bits, its σ's bits or `sigma none`, and
//!   `resolved <bool>`, the hit's bits and the foot's `at <face> <u bits> <v bits>`;
//! - for each material, `material <face> <u bits> <v bits>`, then `none` or `class <code> entry
//!   <entry or ->` and, where the entry is named, its properties' figures.
//!
//! # The Earth-like fixture (plan R10, T12)
//!
//! The client's test that a finest-level bake's dominant weight at each vertex names the palette
//! slot of the worker's material there (`fieldsWasm.test.ts`; main's ruling for R10.T10.b) needs a
//! patch of at least three classes, which neither Moon-like file holds: every cell they survey is
//! highland, of regolith and bedrock of one entry alone. This one surveys the Earth-like world's
//! cell (0, 0) of face +x at level 8, at 50 m with its margin (one payload of 4,850 bytes in two
//! blocks), and pins its finest-level (19) patch (900, 668) of face +x, about 19 m across, centred
//! near 35.23° S, 44.87° W: soil (class 2), sand (3) and a channel's sediment (4), each of the
//! continental crust's granite, entry 1, where the arid bound's height contour crosses a channel
//! (66, 4,120 and 39 of its 4,225 vertices dominant, no tie).
//!
//! The spot was found by a deterministic search, recorded here so that it can be run again (it was,
//! 2026-10-20, to the same patch). A class is the form
//! [`material_at`](crate::material::material_at) answers over the world's whole field (over the
//! fixture's survey the search finds the same blocks and the same patch), and "columns in turn,
//! the rows within each" orders a grid by its first index, then its second.
//!
//! - The Earth-like `material` golden's points of bedrock, snow or sediment, in its order, and
//!   their cells at the field's level, 8, each once, in that order. The first, (0, 0) of face +x,
//!   holds the spot.
//! - On a cell (i, j), a 128 × 128 grid of equal squares of the cell in the cube's (s, t), before
//!   [`st_to_uv`](crate::cube::st_to_uv)'s warp, each 16 × 16 finest-level patches: square (a, b)
//!   read at its centre, s = (128 i + a + ½) ÷ 2¹⁵ and t = (128 j + b + ½) ÷ 2¹⁵.
//! - Every 2 × 2 block of squares, at every (a, b) from 0 to 126, so that the blocks overlap,
//!   columns in turn, the rows within each, that holds at least three classes: here (5, 22),
//!   (53, 41), (54, 41), then (55, 41).
//! - For each such block in turn, the 17 × 17 finest-level patches centred on the one whose corner
//!   of least s and t is the block's centre, (16 (128 i + a + 1), 16 (128 j + b + 1)) on the face,
//!   from 8 below it to 8 above it each way, columns in turn, the rows within each. Each patch is
//!   sampled at every eighth vertex, x and y of 0, 8, …, 64, 81 points (`vertex_point`).
//! - The first patch whose samples hold three classes: the first three blocks have none, and the
//!   fourth's patches about (896, 672) give (900, 668).
//!
//! After the header:
//!
//! - `body`, `seed` and `block 1 <index> <bytes>`, as the Moon-like file's;
//! - the patch's bake as the Moon-like file's bakes, under the name `three_classes`, at the low
//!   setting's options with the weights;
//! - `materials <runs>`: the worker's material at each vertex (x, y) in order of 65 y + x, as
//!   [`WorkerFields::material`] answers it at the vertex's point on its face (`vertex_point`), as
//!   runs `<code>:<entry or ->*<count>`.
//!
//! # The terrain test's format
//!
//! After the header, every float as `0x` and its 16 lowercase hexadecimal digits of bits, which is
//! what is pinned, a scalar's line ending `  # <decimal>` for the reader:
//!
//! - `body <system>.<index>` and `seed <seed>`, as the Moon-like file's;
//! - the field header as `SurfaceHeaderDto` carries it (R09.T19.a's `surface_header`), one
//!   `dto <member> <value>` line a member in its order: an integer in decimal, a name as the wire
//!   spells it, `-` for `null` (and, for `maturity`, `organic_cover` and `weathering_rate`, for a
//!   member not modelled, which the wire leaves out); each palette entry as `dto palette <index>
//!   <substance> <role> <B> <V> <R> <phase row> - <FeO_T>`, the `-` its liquid record, which no
//!   entry of the fixture's palette has (the writer refuses one), and `<FeO_T>` its `FeO_T` mass
//!   fraction (dimensionless, 0 to 1) or `-` where it is not modelled, which the wire leaves out
//!   (R09.T2.f; decision-p14-t51d-weathering §1.6). `maturity` is dimensionless, 0 for fresh
//!   ground to 1 for saturated weathering, `organic_cover` a covered fraction, 0 to 1, and
//!   `weathering_rate` the space-weathering dose rate relative to the Moon's, dimensionless, at
//!   least 0 with no upper bound, the Moon's 1 (§2.2);
//! - `survey <revision> <runs>`, the body's coverage at revisions 1 and 2 as the server's survey
//!   log folds it, each run `<first>-<past last>:<code>`;
//! - for each payload, `payload <n> <revision> <have>` (the body's revision, and the
//!   `have_revision` asked with, `-` for none), `cover <n> <runs>` (the answer's `coverage`: the
//!   cells its blocks hold, each with its code, margin cells at 0, `-` for none) and its `block
//!   <n> <index> <bytes>` lines. Payload 1 is the whole field at revision 1, 2 the delta from 1 to 2,
//!   3 the whole field at 2, and 4 the answer to an ask at the revision held, block 0 alone, the
//!   same bytes at either revision (a test holds it);
//! - `change <ordinal> <runs>` and `held <ordinal> <runs>` for each block of payloads 1 and 2
//!   posted in turn, as the Moon-like file's.
//!
//! Where the header carries a maturity, as `TEST MOON` weathered's does and `TERRAIN TEST`'s does
//! not, five lines follow, each over a worker holding payloads 1 and 2's blocks, every digest `0x`
//! and 16 lowercase hexadecimal digits (R10.T1.c's client half):
//!
//! - `crust_albedo <B> <V> <R>  # <b> <v> <r>`: the primary crust's regolith's drawn normal albedo
//!   at `m_body` at band level 0, its band-0 node K − 1 ([`WorkerFields::maturity_nodes`]), the
//!   ground the class map leaves to `elsewhere`, in B, V and R;
//! - three `maturity_patch <face> <level> <i> <j> <codes> <nodes>` lines, the patches holding the
//!   point (1, 0.99, 0.62) at band levels 8 and 13 and at the band limit's, each baked with the
//!   weights (whatever the vertex path and normals, which move neither digest), at the fixture's
//!   seed: `<codes>` the digest of the bake's `maturity()`, its unorm16 codes each as an `f64`
//!   (`code_digest`, as `byte_digest` takes the weights' bytes: TypeScript's
//!   `f64Digest(Float64Array.from(codes))`), and `<nodes>` the FNV-1a digest of
//!   `maturityNodes(system, body, palette, level)`'s `Float32Array` for the bake's own palette and
//!   level (`f32_digest`);
//! - `class_map_maturity <codes> <nodes>`: the class map's (at the fixture's seed) `maturity()`
//!   codes' digest, as the bake's codes', and its `nodes()` `Float32Array`'s.
//!
//! # The maturity layer's format
//!
//! After the header, every float as `0x` and its 16 lowercase hexadecimal digits of bits, for each
//! of two worlds, the 3.5 Gyr Moon-like world at a scale of 1 (`moon_like_3.5_gyr`) and `TEST MOON`
//! weathered (`terrain_test_weathered`), each over a worker holding a 2 × 2 survey about (0.3, 1,
//! 0.2) at 50 m with its margin:
//!
//! - `world <name>: m_body <bits>, scale <bits or ->, <n> octaves expose, band limit level <L>`;
//! - for each band level n from 0 to L, `mix <n> <octaves left out> <nodes> <atom> <mean deficit>`:
//!   the level's [`unresolved_mix`], the weight of its node at `m_body` (0 where it has none) and
//!   its mean deficit Σ w (`m_body` − μ);
//! - for the patches holding (0.3, 1, 0.2) at levels 10, 14 and L + 1, baked at the low setting
//!   with the weights: `patch <face> <level> <i> <j>: palette <slots>` (as the Moon-like file's
//!   palette line), `codes <digest>` (the layer's codes, as `code_digest` takes them), `row <y>
//!   <pairs>` for rows 0, 32 and 64, each vertex's codes at n and n − 1 as eight hexadecimal digits,
//!   four each,
//!   and for each named slot `slot <k> <class>:<entry>: level <digest> parent <digest>` (the
//!   `f64_digest`s of its nodes at n and at n − 1, node by node, B, V, R) with `node <j> <B> <V> <R>
//!   <B> <V> <R>` for nodes 0 and K − 1, at n then at n − 1;
//! - `class_map <texels> texels <classes> classes`, then `texel <cell index> <code>`, the code in four
//!   hexadecimal digits, and `class <class>:<entry>: <digest> <B> <V> <R>` (its band-0 nodes'
//!   `f64_digest` and node K − 1) or `class <class>:<entry>: none`.
//!
//! Then the TypeScript twin's cases, the 3.5 Gyr world's basalt regolith (`1:1`) at band levels 0
//! and 10: `twin moon_like_3.5_gyr 1:1 level <n>: m_body <bits>`, its K nodes as `node <j> <x>
//! <B> <V> <R>` (x = `m_j` ÷ `m_body`, [`maturity_node_share`]), and 33 `sample <x> <B> <V> <R> <B>
//! <V> <R>` lines at x = 1 − (1 − u³)², u = k ÷ 32: the nodes' own law T(m) = Σ w G(min(m, μ))
//! over the level's mix, then [`maturity_node_albedo`]'s interpolation, at m = x `m_body` (`m_body`
//! is 1, so m is x).

use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, f32_digest, f64_digest};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::tests::{
    MESH, TerrainFixture, WORLD, band_limit_level, cell, earth_fixture, earth_worker, face_uv,
    fixture, lit, middle_weathered_world, payload_cover, probes, sight_lines, surveyed_about,
    terrain_fixture, terrain_fixture_of, vertex_point, weathered_terrain_world,
};
use super::*;
use crate::bake::WeightsLayer;
use crate::field::{
    ClimateModelKind, CoarseField, Cover, FieldHeader, FieldHeaderParts, PaletteEntry, PaletteRole,
    PrecipitationSource,
};
use crate::material::{MaterialClass, MaturityNode, maturity_node_albedo, maturity_node_share};
use crate::patch::{NormalScale, VertexPath};
use crate::synth::test_support::{SEED, world};

/// A digest of `bytes` in the golden's form: each as an `f64`, which holds it exactly, as
/// R10.T6.a's goldens take it.
fn byte_digest(bytes: impl IntoIterator<Item = u8>) -> u64 {
    f64_digest(&bytes.into_iter().map(f64::from).collect::<Vec<_>>())
}

/// A digest of a maturity layer's unorm16 `codes` in the golden's form: each as an `f64`, which
/// holds it exactly, as [`byte_digest`] takes bytes.
fn code_digest(codes: impl IntoIterator<Item = u16>) -> u64 {
    f64_digest(&codes.into_iter().map(f64::from).collect::<Vec<_>>())
}

/// `values`' bits, each as 16 lowercase hexadecimal digits, spaced.
fn bits_of(values: &[f64]) -> String {
    values
        .iter()
        .map(|&v| format!("{:016x}", bits(v)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `bytes` in lowercase hexadecimal.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(2 * bytes.len()), |mut text, b| {
            write!(text, "{b:02x}").expect("writing to a String cannot fail");
            text
        })
}

/// Runs in the golden's form.
fn runs(runs: &[(u32, u32)]) -> String {
    if runs.is_empty() {
        "-".to_owned()
    } else {
        runs.iter()
            .map(|(a, b)| format!("{a}-{b}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Indices in the golden's form, `-` for none.
fn indices(cells: &[u32]) -> String {
    if cells.is_empty() {
        "-".to_owned()
    } else {
        cells
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Writes the horizon map of `bake`, the bake named `name`.
fn write_horizon(out: &mut GoldenWriter, name: &str, bake: &BodyBake) {
    let k = bake.terrain().patch().key;
    let map = bake.terrain().layers().horizon();
    out.line(&format!(
        "horizon {name} {} {} {} {}",
        k.face().index(),
        k.level(),
        k.i(),
        k.j()
    ));
    let digest =
        |words: &[u32]| f64_digest(&words.iter().copied().map(f64::from).collect::<Vec<_>>());
    out.u64_hex("near", digest(map.near_words()));
    out.u64_hex("far", digest(map.far_words()));
    out.line(&format!("horizon_complete {}", map.is_complete()));
    out.line(&format!("refused {}", indices(map.refused_cells())));
}

/// Writes the level table's rows and σ by level of `body` in `fields`, as of `when`.
fn write_levels(out: &mut GoldenWriter, when: &str, fields: &WorkerFields, body: BodyRef) {
    let table = fields.level_table(body).expect("held");
    for (n, row) in table.chunks(LEVEL_TABLE_STRIDE).enumerate() {
        out.line(&format!("level {when} {n} {}", bits_of(row)));
    }
    let sigma = fields.level_sigma_m(body).expect("held");
    out.line(&format!("level_sigma {when} {}", bits_of(&sigma)));
}

/// Writes `bake`'s lines, the bake named `name` of `opts`.
fn write_bake(out: &mut GoldenWriter, name: &str, opts: &TerrainBakeOptions, bake: &BodyBake) {
    let (p, l) = (bake.terrain().patch(), bake.terrain().layers());
    let k = p.key;
    let path = match opts.patch.vertex_path {
        VertexPath::BakedOffsets => "baked-offsets",
        VertexPath::FaceDifferences => "face-differences",
    };
    let normals = match opts.patch.normals {
        NormalScale::Mesh => "mesh",
        NormalScale::Double => "double",
    };
    let weights = match opts.weights {
        WeightsLayer::Baked => "baked",
        WeightsLayer::Skipped => "skipped",
    };
    out.line(&format!(
        "patch {name} {} {} {} {} {path} {normals} {weights}",
        k.face().index(),
        k.level(),
        k.i(),
        k.j()
    ));
    out.u64_hex("heights", f32_digest(&p.heights));
    out.u64_hex(
        "offsets",
        f32_digest(p.offsets.as_deref().unwrap_or_default()),
    );
    out.u64_hex("normals", f32_digest(&p.normals));
    for (axis, x) in ["x", "y", "z"].into_iter().zip(p.origin) {
        out.f64(&format!("origin_{axis}"), x);
    }
    out.f64("origin_height", p.origin_height_m);
    out.f64("lowest", f64::from(p.height_range_m.0));
    out.f64("highest", f64::from(p.height_range_m.1));
    out.f64("bounding_radius", p.bounding_radius_m);
    out.f64("skirt_depth", p.skirt_depth_m);
    out.line(&format!("palette {}", palette_words(&l.palette())));
    out.u64_hex(
        "weights",
        byte_digest(l.weights().iter().flatten().copied()),
    );
    out.u64_hex("mask", byte_digest(l.survey_mask().iter().copied()));
    out.f64("bound", l.bound().value());
    out.f64("sigma", l.sigma().value());
    out.line(&format!("complete {}", l.is_complete()));
    out.line(&format!("unsurveyed {}", indices(bake.unsurveyed_cells())));
}

/// A bake's palette in the golden's form: each slot `<class code>:<entry>` (`-` for no entry), or
/// `none`.
fn palette_words(palette: &[Option<GroundMaterial>]) -> String {
    let words: Vec<String> = palette
        .iter()
        .map(|slot| slot.map_or_else(|| "none".to_owned(), ground_word))
        .collect();
    words.join(" ")
}

/// A ground material in the golden's form: `<class code>:<entry>`, `-` for no entry.
fn ground_word(m: GroundMaterial) -> String {
    let entry = m.entry().map_or_else(|| "-".to_owned(), |e| e.to_string());
    format!("{}:{entry}", u8::from(m.class()))
}

/// The golden's points: two in the surveys, one in the margin and one not held.
fn points() -> [FaceUv; 4] {
    [
        face_uv(cell(22, 22), 0.3, 0.6),
        face_uv(cell(25, 23), 0.7, 0.2),
        face_uv(cell(20, 22), 0.5, 0.5),
        face_uv(cell(40, 40), 0.5, 0.5),
    ]
}

/// The Moon-like fixture's golden.
fn moon_text() -> String {
    let f = fixture();
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    out.line(&format!(
        "body {:016x}.{:04x}",
        f.body.raw_system_id(),
        f.body.body_index()
    ));
    out.line(&format!("seed {:016x}", SEED.get()));
    for (payload, blocks) in [(1, &f.first_blocks), (2, &f.second_blocks)] {
        for (index, block) in blocks.iter().enumerate() {
            out.line(&format!("block {payload} {index} {}", hex(block)));
        }
    }
    let mut fields = WorkerFields::new();
    for (ordinal, block) in f.blocks().enumerate() {
        let change = if ordinal == 0 {
            fields.init(f.body, block, SigmaSurvey::Surveyed)
        } else {
            fields.chunk(f.body, block)
        }
        .expect("the fixture's blocks are taken");
        out.line(&format!("change {ordinal} {}", runs(change.runs())));
        out.line(&format!("held {ordinal} {}", runs(change.held_runs())));
        if ordinal == 0 {
            write_levels(&mut out, "first", &fields, f.body);
        }
    }
    write_levels(&mut out, "last", &fields, f.body);
    for (name, key, opts) in probes() {
        let bake = fields.bake(f.body, SEED, key, &opts).expect("held");
        write_bake(&mut out, name, &opts, &bake);
    }
    for (name, key, opts) in probes()
        .into_iter()
        .filter(|(name, ..)| matches!(*name, "inside" | "astride"))
    {
        let bake = fields.bake(f.body, SEED, key, &lit(opts)).expect("held");
        write_horizon(&mut out, name, &bake);
    }
    write_readings(&mut out, &mut fields, f.body);
    write_sight_lines(&mut out, &mut fields, f.body);
    write_materials(&mut out, &mut fields, f.body);
    out.finish()
}

/// Writes the readings at [`points`], at two baselines each.
fn write_readings(out: &mut GoldenWriter, fields: &mut WorkerFields, body: BodyRef) {
    for at in points() {
        for baseline in [2.0, 100.0] {
            out.line(&format!(
                "reading {} {}",
                at.face.index(),
                bits_of(&[at.u, at.v, baseline])
            ));
            match fields
                .reading(body, SEED, at, Metres::new(baseline))
                .expect("held")
            {
                None => out.line("none"),
                Some(r) => {
                    out.f64("elevation", r.elevation().value());
                    out.f64("elevation_sigma", r.elevation_sigma().value());
                    out.f64("slope_rms", r.slope_rms().value());
                    out.f64("slope_sigma", r.slope_sigma().value());
                    out.f64("slope_baseline", r.slope_baseline().value());
                    out.f64("resolution", r.resolution().value());
                    out.line(&format!("resolved {}", r.is_resolved()));
                }
            }
        }
    }
}

/// Writes the lines of sight of [`sight_lines`], each within 10,000 km.
fn write_sight_lines(out: &mut GoldenWriter, fields: &mut WorkerFields, body: BodyRef) {
    let field = fields.field(body).expect("held").clone();
    let reach = 1e7;
    for (origin, direction) in sight_lines(&field) {
        let mut inputs = origin.to_vec();
        inputs.extend(direction);
        inputs.push(reach);
        out.line(&format!("slant {}", bits_of(&inputs)));
        match fields
            .slant_range(body, SEED, &origin, &direction, Metres::new(reach))
            .expect("held")
        {
            LineOfSight::NoGround => out.line("no_ground"),
            LineOfSight::NotSurveyed => out.line("not_surveyed"),
            LineOfSight::Ground(s) => {
                out.line("ground");
                out.f64("range", s.range().value());
                match s.sigma() {
                    Some(sigma) => out.f64("sigma", sigma.value()),
                    None => out.line("sigma none"),
                }
                out.line(&format!("resolved {}", s.is_resolved()));
                for (axis, x) in ["x", "y", "z"].into_iter().zip(s.hit_m()) {
                    out.f64(&format!("hit_{axis}"), x);
                }
                let at = s.at();
                out.line(&format!(
                    "at {} {}",
                    at.face.index(),
                    bits_of(&[at.u, at.v])
                ));
            }
        }
    }
}

/// Writes the materials at [`points`].
fn write_materials(out: &mut GoldenWriter, fields: &mut WorkerFields, body: BodyRef) {
    for at in points() {
        out.line(&format!(
            "material {} {}",
            at.face.index(),
            bits_of(&[at.u, at.v])
        ));
        match fields.material(body, SEED, at).expect("held") {
            None => out.line("none"),
            Some(m) => {
                let g = m.ground();
                let entry = g.entry().map_or_else(|| "-".to_owned(), |e| e.to_string());
                out.line(&format!("class {} entry {entry}", u8::from(g.class())));
                if let Some(p) = m.properties() {
                    for (band, a) in ["b", "v", "r"].into_iter().zip(p.normal_albedo_bvr()) {
                        out.f64(&format!("albedo_{band}"), a);
                    }
                    out.line(&format!("phase_law {}", u8::from(p.phase_law())));
                    out.f64("limb", p.limb_parameter());
                    out.f64("grain_density", p.grain_density().value());
                    out.f64("transition", p.transition().value());
                    out.line(&format!("mechanics {}", u8::from(p.mechanics())));
                }
            }
        }
    }
}

#[test]
fn worker_matches_its_golden() {
    golden!("worker/moon_like", &moon_text());
}

/// The Earth-like fixture's golden (the module documentation).
fn earth_text() -> String {
    let f = earth_fixture();
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    out.line(&format!(
        "body {:016x}.{:04x}",
        f.body.raw_system_id(),
        f.body.body_index()
    ));
    out.line(&format!("seed {:016x}", SEED.get()));
    for (index, block) in f.blocks.iter().enumerate() {
        out.line(&format!("block 1 {index} {}", hex(block)));
    }
    let mut fields = earth_worker(&f);
    let bake = fields.bake(f.body, SEED, f.patch, &MESH).expect("held");
    write_bake(&mut out, "three_classes", &MESH, &bake);
    // One synthesiser for the patch's 4,225 points, where `material` builds one a query: the same
    // answer (`surveyed_material`, after the same check), which the fixture's test holds at a vertex
    // of each class.
    let held = fields.field(f.body).expect("held");
    let synth = Synthesiser::new(held, SEED);
    let mut cache = SynthCache::new();
    let mut words: Vec<(String, u32)> = Vec::new();
    for y in 0..=PATCH_QUADS {
        for x in 0..=PATCH_QUADS {
            let at = vertex_point(f.patch, x, y);
            assert!(
                held.is_surveyed(field_cell(held, at)),
                "the patch is surveyed"
            );
            let ground = surveyed_material(&synth, &mut cache, at)
                .expect("a surveyed cell's margin is held")
                .ground();
            let entry = ground
                .entry()
                .map_or_else(|| "-".to_owned(), |e| e.to_string());
            let word = format!("{}:{entry}", u8::from(ground.class()));
            match words.last_mut() {
                Some((last, count)) if *last == word => *count += 1,
                _ => words.push((word, 1)),
            }
        }
    }
    let runs: Vec<String> = words
        .iter()
        .map(|(word, count)| format!("{word}*{count}"))
        .collect();
    out.line(&format!("materials {}", runs.join(" ")));
    out.finish()
}

#[test]
fn worker_matches_its_earth_like_golden() {
    golden!("worker/earth_like", &earth_text());
}

/// A palette role as the wire spells it (`PaletteRoleDto`).
fn role_name(role: PaletteRole) -> &'static str {
    match role {
        PaletteRole::PrimaryCrust => "primary_crust",
        PaletteRole::SecondaryCrust => "secondary_crust",
        PaletteRole::TertiaryCrust => "tertiary_crust",
        PaletteRole::Province => "province",
        PaletteRole::Ice => "ice",
        PaletteRole::Liquid => "liquid",
        PaletteRole::Cover => "cover",
        PaletteRole::Deposit => "deposit",
        PaletteRole::Bulk => "bulk",
    }
}

/// A climate model as the wire spells it (`ClimateModelDto`).
fn climate_name(kind: ClimateModelKind) -> &'static str {
    match kind {
        ClimateModelKind::EnergyBalance => "energy_balance",
        ClimateModelKind::LockedEnergyBalance => "locked_energy_balance",
        ClimateModelKind::RadiativeEquilibrium => "radiative_equilibrium",
        ClimateModelKind::Isothermal => "isothermal",
    }
}

/// A precipitation source as the wire spells it (`PrecipitationSourceDto`).
fn precipitation_name(source: PrecipitationSource) -> &'static str {
    match source {
        PrecipitationSource::Heuristic => "heuristic",
    }
}

/// A float in the terrain test's form: `0x` and its bits.
fn float(x: f64) -> String {
    format!("0x{:016x}", bits(x))
}

/// Writes `dto <key> <x>` with the decimal for the reader.
fn dto_f64(out: &mut GoldenWriter, key: &str, x: f64) {
    out.line(&format!("dto {key} {}  # {x:?}", float(x)));
}

/// Writes `dto <key>`, then `x` as [`dto_f64`] does, or `-` for none.
fn dto_option(out: &mut GoldenWriter, key: &str, x: Option<f64>) {
    match x {
        Some(x) => dto_f64(out, key, x),
        None => out.line(&format!("dto {key} -")),
    }
}

/// Writes the header's DTO lines, in `SurfaceHeaderDto`'s order.
///
/// The header's parts and each palette entry are taken apart whole, each member the wire does not
/// carry named and dropped, so that a member added to either is a compile error here until the
/// format takes it or says it does not (fixes-d D-A, after R09.T2.f added `FeO_T` and the
/// weathering rate unseen).
///
/// # Panics
///
/// If a palette entry carries a liquid record, which the format does not hold: the fixture's
/// Moon-like palette has none.
fn write_dto(out: &mut GoldenWriter, header: &FieldHeader) {
    let FieldHeaderParts {
        body,
        radius,
        figure,
        sea_level,
        lapse_rate_k_per_m,
        spectrum: _,
        craters: _,
        climate_model,
        precipitation,
        realised_sigma_h,
        realised_relief,
        months,
        season_eccentricity,
        reference_temperature,
        temperature_step: _,
        anomaly_step: _,
        surface_age,
        surface_pressure,
        albedo_scale,
        palette,
        crust_palette: _,
        main_liquid,
        maturity,
        organic_cover,
        weathering_rate,
    } = header.parts();
    out.line(&format!(
        "dto body {:016x}.{:04x}",
        body.raw_system_id(),
        body.body_index()
    ));
    out.line(&format!("dto level {}", header.level().get()));
    dto_f64(out, "radius_m", radius.value());
    dto_f64(out, "equatorial_radius_m", figure.equatorial_radius_m);
    dto_f64(out, "polar_radius_m", figure.polar_radius_m);
    dto_f64(
        out,
        "boundary_diameter_m",
        header.boundary_diameter().value(),
    );
    dto_f64(out, "sea_level_m", sea_level.value());
    dto_f64(out, "lapse_rate_k_per_m", *lapse_rate_k_per_m);
    dto_f64(out, "realised_sigma_h_m", realised_sigma_h.value());
    dto_f64(out, "realised_relief_m", realised_relief.value());
    out.line(&format!("dto months {months}"));
    dto_f64(out, "season_eccentricity", *season_eccentricity);
    dto_f64(
        out,
        "reference_temperature_k",
        reference_temperature.value(),
    );
    dto_f64(out, "surface_age_gyr", surface_age.value());
    dto_f64(out, "surface_pressure_pa", surface_pressure.value());
    dto_option(out, "albedo_scale", *albedo_scale);
    out.line(&format!(
        "dto climate_model {}",
        climate_name(*climate_model)
    ));
    out.line(&format!(
        "dto precipitation {}",
        precipitation_name(*precipitation)
    ));
    for (index, entry) in palette.entries().iter().enumerate() {
        let PaletteEntry {
            substance,
            role,
            normal_albedo_bvr,
            phase_row,
            grain_density_kg_m3: _,
            transition: _,
            mechanics: _,
            liquid,
            feo_mass_fraction,
        } = entry;
        assert!(
            liquid.is_none(),
            "the terrain test's format holds no liquid record"
        );
        let [b, v, r] = normal_albedo_bvr.map(float);
        let feo = feo_mass_fraction.map_or_else(|| "-".to_owned(), float);
        out.line(&format!(
            "dto palette {index} {} {} {b} {v} {r} {phase_row} - {feo}",
            substance.as_str(),
            role_name(*role),
        ));
    }
    out.line(&format!(
        "dto main_liquid {}",
        main_liquid.map_or_else(|| "-".to_owned(), |m| m.to_string())
    ));
    dto_option(out, "maturity", *maturity);
    dto_option(out, "organic_cover", *organic_cover);
    dto_option(out, "weathering_rate", *weathering_rate);
}

/// `cover`'s runs in the terrain test's form, each `<first>-<past last>:<code>`, `-` for none.
fn coded_runs(cover: &Cover) -> String {
    if cover.is_empty() {
        "-".to_owned()
    } else {
        cover
            .ranges()
            .iter()
            .map(|r| format!("{}-{}:{}", r.start(), r.end(), r.code().get()))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// `TERRAIN TEST`'s golden.
fn terrain_text() -> String {
    terrain_text_of(&terrain_fixture(), world(WORLD))
}

/// `TEST MOON` weathered's golden (decision-r10-t1c-client-scope §3): `TERRAIN TEST`'s surveys
/// and payloads over [`weathered_terrain_world`], with its maturity's lines.
fn terrain_weathered_text() -> String {
    let field = weathered_terrain_world();
    terrain_text_of(&terrain_fixture_of(field), field)
}

/// The terrain test's golden of the fixture `f` of `field`: its header, surveys, payloads and the
/// changes its blocks make in turn, and, where the header carries a maturity, its maturity's lines
/// ([`write_maturity`]).
fn terrain_text_of(f: &TerrainFixture, field: &CoarseField) -> String {
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    out.line(&format!(
        "body {:016x}.{:04x}",
        f.body.raw_system_id(),
        f.body.body_index()
    ));
    out.line(&format!("seed {:016x}", SEED.get()));
    write_dto(&mut out, field.header());
    for (revision, survey) in (1..).zip(&f.surveys) {
        out.line(&format!("survey {revision} {}", coded_runs(survey)));
    }
    for (n, payload) in (1..).zip(&f.payloads) {
        let have = payload
            .have
            .map_or_else(|| "-".to_owned(), |h| h.to_string());
        out.line(&format!("payload {n} {} {have}", payload.revision));
        out.line(&format!(
            "cover {n} {}",
            coded_runs(&payload_cover(&payload.blocks))
        ));
        for (index, block) in payload.blocks.iter().enumerate() {
            out.line(&format!("block {n} {index} {}", hex(block)));
        }
    }
    let mut fields = WorkerFields::new();
    for (ordinal, block) in f.first_then_delta().enumerate() {
        let change = if ordinal == 0 {
            fields.init(f.body, block, SigmaSurvey::Surveyed)
        } else {
            fields.chunk(f.body, block)
        }
        .expect("the fixture's blocks are taken");
        out.line(&format!("change {ordinal} {}", runs(change.runs())));
        out.line(&format!("held {ordinal} {}", runs(change.held_runs())));
    }
    if field.header().maturity().is_some() {
        write_maturity(&mut out, &mut fields, f.body, field);
    }
    out.finish()
}

#[test]
fn worker_matches_its_terrain_test_golden() {
    golden!("worker/terrain_test", &terrain_text());
}

#[test]
fn worker_matches_its_weathered_terrain_test_golden() {
    golden!("worker/terrain_test_weathered", &terrain_weathered_text());
}

/// The patches whose maturity layer and nodes `TEST MOON` weathered's golden pins: those holding
/// a point of the second survey, (1, 0.99, 0.62), some 0.3° from its centre, at band levels 8 and
/// 13 and at the band limit's.
fn weathered_patches(field: &CoarseField) -> [PatchKey; 3] {
    let dir = unit_dir([1.0, 0.99, 0.62]);
    [8, 13, band_limit_level(field)]
        .map(|level| PatchKey::containing(level, dir).expect("a band level is a quadtree level"))
}

/// `values` narrowed to `f32`, as the client module's exports narrow them.
fn narrowed(values: impl IntoIterator<Item = f64>) -> Vec<f32> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the exports' narrowing: a normal albedo of at most 1.35 narrows far inside f32"
    )]
    let narrow = |x: f64| x as f32;
    values.into_iter().map(narrow).collect()
}

/// What the client module's `maturityNodes` answers for `bake`'s palette at the bake's level on
/// `body`: each slot's nodes at the level, then at its parent's, each node's B, V and R, narrowed
/// to `f32`; all 0 for an empty slot or one whose palette names no substance.
fn export_nodes(fields: &mut WorkerFields, body: BodyRef, bake: &BodyBake) -> Vec<f32> {
    let level = band(bake.terrain().patch().key.level());
    let mut out = Vec::new();
    for slot in bake.terrain().layers().palette() {
        match slot.and_then(GroundMaterial::material) {
            Some(m) => {
                let nodes = fields.maturity_nodes(body, m, level).expect("held");
                let values = nodes.level().iter().chain(nodes.parent()).flatten();
                out.extend(narrowed(values.copied()));
            }
            None => out.extend(std::iter::repeat_n(0.0_f32, 2 * 3 * MATURITY_NODES)),
        }
    }
    out
}

/// What the client module's class map answers of its maturity: `maturity()`, each texel's code,
/// and `nodes()`, each class's band-0 nodes narrowed to `f32`, all 0 for an unnamed class.
fn export_class_map(map: &ClassMap) -> (Vec<u16>, Vec<f32>) {
    let codes = map
        .texels()
        .iter()
        .filter_map(ClassMapTexel::maturity)
        .collect();
    let mut nodes = Vec::new();
    for class in map.classes() {
        match class.nodes() {
            Some(n) => nodes.extend(narrowed(n.iter().flatten().copied())),
            None => nodes.extend(std::iter::repeat_n(0.0_f32, 3 * MATURITY_NODES)),
        }
    }
    (codes, nodes)
}

/// Writes `TEST MOON` weathered's maturity lines (the module documentation's terrain test format),
/// `fields` holding the blocks of payloads 1 and 2 of `field`, the body `body`'s.
fn write_maturity(
    out: &mut GoldenWriter,
    fields: &mut WorkerFields,
    body: BodyRef,
    field: &CoarseField,
) {
    let header = field.header();
    let crust = header
        .palette()
        .entries()
        .iter()
        .position(|entry| entry.role == PaletteRole::PrimaryCrust)
        .expect("the Moon-like palette has a primary crust");
    let regolith = Material::new(
        MaterialClass::Regolith,
        u8::try_from(crust).expect("a palette holds at most 15 entries"),
    )
    .expect("a palette's index is an entry");
    let nodes = fields
        .maturity_nodes(body, regolith, band(CLASS_MAP_BAND))
        .expect("held");
    let [b, v, r] = nodes.level()[MATURITY_NODES - 1];
    out.line(&format!(
        "crust_albedo {} {} {}  # {b:?} {v:?} {r:?}",
        float(b),
        float(v),
        float(r)
    ));
    for key in weathered_patches(field) {
        let bake = fields.bake(body, SEED, key, &MESH).expect("held");
        let codes = code_digest(bake.terrain().layers().maturity().iter().flatten().copied());
        let nodes = f32_digest(&export_nodes(fields, body, &bake));
        out.line(&format!(
            "maturity_patch {} {} {} {} 0x{codes:016x} 0x{nodes:016x}",
            key.face().index(),
            key.level(),
            key.i(),
            key.j()
        ));
    }
    let map = fields.class_map(body, SEED).expect("held");
    let (codes, nodes) = export_class_map(&map);
    out.line(&format!(
        "class_map_maturity 0x{:016x} 0x{:016x}",
        code_digest(codes),
        f32_digest(&nodes)
    ));
}

/// The layer golden's worlds (decision-r10-t1c-client-scope §4): the 3.5 Gyr Moon-like world at a
/// scale of 1, and `TEST MOON` weathered, on its 1 Gyr surface at its haloed scale.
fn layer_worlds() -> [(&'static str, &'static CoarseField); 2] {
    [
        ("moon_like_3.5_gyr", middle_weathered_world()),
        ("terrain_test_weathered", weathered_terrain_world()),
    ]
}

/// The nodes' own law at `m` of a material of drawn albedo `g` over `mix`: Σ w G(min(m, μ)), band
/// by band in node order from +0, as [`maturity_nodes`] sums each node.
fn node_law(mix: &crate::material::UnresolvedMix, g: impl Fn(f64) -> [f64; 3], m: f64) -> [f64; 3] {
    let mut sum = [0.0; 3];
    for node in mix.nodes() {
        let at = if m < node.maturity() {
            m
        } else {
            node.maturity()
        };
        for (total, band) in sum.iter_mut().zip(g(at)) {
            *total += node.weight() * band;
        }
    }
    sum
}

/// Bits of B, V and R, spaced, in the terrain test's float form.
fn bvr_floats(bvr: [f64; 3]) -> String {
    bvr.map(float).join(" ")
}

/// The maturity layer's golden (decision-r10-t1c-client-scope §4; the module documentation).
fn maturity_layer_text() -> String {
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    for (name, field) in layer_worlds() {
        write_layer_world(&mut out, name, field);
    }
    write_twin(&mut out);
    out.finish()
}

/// Writes the layer golden's lines of the world `field`, named `name`: its levels' mixes, three
/// patches' layers and nodes, and its class map's maturity, over a 2 × 2 survey.
fn write_layer_world(out: &mut GoldenWriter, name: &str, field: &CoarseField) {
    let header = field.header();
    let body = header.body();
    let m_body = header.maturity().expect("the world carries a maturity");
    let terms = unresolved_terms(header).expect("the world carries a maturity and a rate");
    let limit = band_limit_level(field);
    let scale = header.albedo_scale().map_or_else(|| "-".to_owned(), float);
    out.line(&format!(
        "world {name}: m_body {}, scale {scale}, {} octaves expose, band limit level {limit}",
        float(m_body),
        terms.octaves().len()
    ));
    for level in 0..=limit {
        let mix = unresolved_mix(&terms, band(level));
        let atom = mix
            .nodes()
            .first()
            .filter(|node| node.maturity().total_cmp(&m_body).is_eq())
            .map_or(0.0, MaturityNode::weight);
        let mean = mix.nodes().iter().fold(0.0, |sum, node| {
            sum + node.weight() * (m_body - node.maturity())
        });
        out.line(&format!(
            "mix {level} {} {} {} {}",
            mix.unresolved_octaves(),
            mix.nodes().len(),
            float(atom),
            float(mean)
        ));
    }
    let dir = unit_dir([0.3, 1.0, 0.2]);
    let (mut fields, _) = surveyed_about(field, dir);
    for level in [10, 14, limit + 1] {
        let key = PatchKey::containing(level, dir).expect("a band level is a quadtree level");
        write_layer_patch(out, &mut fields, body, key);
    }
    let map = fields.class_map(body, SEED).expect("held");
    out.line(&format!(
        "class_map {} texels {} classes",
        map.texels().len(),
        map.classes().len()
    ));
    for texel in map.texels() {
        let code = texel.maturity().expect("the world carries a maturity");
        out.line(&format!("texel {} {code:04x}", cell_index(texel.cell())));
    }
    for class in map.classes() {
        match class.nodes() {
            Some(nodes) => out.line(&format!(
                "class {}: 0x{:016x} {}",
                ground_word(class.material()),
                node_digest(nodes),
                bvr_floats(nodes[MATURITY_NODES - 1])
            )),
            None => out.line(&format!("class {}: none", ground_word(class.material()))),
        }
    }
}

/// The `f64_digest` of `nodes`, node by node, each its B, V and R.
fn node_digest(nodes: &[[f64; 3]; MATURITY_NODES]) -> u64 {
    f64_digest(&nodes.iter().flatten().copied().collect::<Vec<_>>())
}

/// Writes the layer golden's lines of the patch `key` of `body`, baked at the low setting with the
/// weights through `fields`: its palette, its codes' digest and three rows, and each named slot's
/// nodes at its level and its parent's.
fn write_layer_patch(
    out: &mut GoldenWriter,
    fields: &mut WorkerFields,
    body: BodyRef,
    key: PatchKey,
) {
    let bake = fields.bake(body, SEED, key, &MESH).expect("held");
    let layers = bake.terrain().layers();
    out.line(&format!(
        "patch {} {} {} {}: palette {}",
        key.face().index(),
        key.level(),
        key.i(),
        key.j(),
        palette_words(&layers.palette())
    ));
    let codes = layers.maturity();
    out.line(&format!(
        "codes 0x{:016x}",
        code_digest(codes.iter().flatten().copied())
    ));
    for y in [0_usize, 32, 64] {
        let row: Vec<String> = codes[65 * y..65 * (y + 1)]
            .iter()
            .map(|[n, p]| format!("{n:04x}{p:04x}"))
            .collect();
        out.line(&format!("row {y} {}", row.join(" ")));
    }
    for (k, slot) in layers.palette().iter().enumerate() {
        let Some(ground) = *slot else {
            continue;
        };
        let Some(m) = ground.material() else {
            continue;
        };
        let nodes = fields
            .maturity_nodes(body, m, band(key.level()))
            .expect("held");
        out.line(&format!(
            "slot {k} {}: level 0x{:016x} parent 0x{:016x}",
            ground_word(ground),
            node_digest(nodes.level()),
            node_digest(nodes.parent())
        ));
        for j in [0, MATURITY_NODES - 1] {
            out.line(&format!(
                "node {j} {} {}",
                bvr_floats(nodes.level()[j]),
                bvr_floats(nodes.parent()[j])
            ));
        }
    }
}

/// Writes the TypeScript twin's cases: the 3.5 Gyr world's basalt regolith's nodes at band levels
/// 0 and 10, and the nodes' own law and the interpolation at 33 maturities graded as the nodes are.
fn write_twin(out: &mut GoldenWriter) {
    let field = middle_weathered_world();
    let header = field.header();
    let m_body = header.maturity().expect("the world carries a maturity");
    let terms = unresolved_terms(header).expect("the world carries a maturity and a rate");
    let basalt = Material::new(MaterialClass::Regolith, 1).expect("entry 1 is a palette index");
    let g = |m: f64| {
        let exposed = exposed_properties(
            basalt,
            header.palette(),
            GroundExposure::of_header(header).with_maturity(m),
        )
        .expect("entry 1 is the palette's");
        scaled_normal_albedo(&exposed, header.albedo_scale().unwrap_or(1.0))
    };
    for level in [0, 10] {
        let mix = unresolved_mix(&terms, band(level));
        let nodes = maturity_nodes(&mix, g);
        out.line(&format!(
            "twin moon_like_3.5_gyr {} level {level}: m_body {}",
            ground_word(GroundMaterial::of(MaterialClass::Regolith, Some(1))),
            float(m_body)
        ));
        for (j, node) in nodes.iter().enumerate() {
            out.line(&format!(
                "node {j} {} {}",
                float(maturity_node_share(j)),
                bvr_floats(*node)
            ));
        }
        for k in 0..=32_u32 {
            let u = f64::from(k) / 32.0;
            let fresh = 1.0 - u * u * u;
            let x = 1.0 - fresh * fresh;
            let m = x * m_body;
            out.line(&format!(
                "sample {} {} {}",
                float(x),
                bvr_floats(node_law(&mix, g, m)),
                bvr_floats(maturity_node_albedo(&nodes, m_body, m))
            ));
        }
    }
}

#[test]
fn worker_matches_its_maturity_layer_golden() {
    golden!("worker/maturity_layer", &maturity_layer_text());
}

/// Block 0 of the terrain test's first payload carries the Moon-like world's header, which its
/// `dto` lines state for the client; that those lines are the header the server sends is held by
/// `hyperion-server`'s `requests::surface` tests (`terrain_test_golden`).
#[test]
fn worker_terrain_test_block_zero_carries_the_world_s_header() {
    let f = terrain_fixture();
    let blocks = decode_payload_of(&f.payloads[0].blocks);
    assert_eq!(blocks[0].header(), Some(world(WORLD).header()));
}

/// The decoded blocks of a payload's.
fn decode_payload_of(blocks: &[Vec<u8>]) -> Vec<crate::wire::DecodedBlock> {
    crate::wire::decode_payload(&blocks.concat()).expect("the fixture's payloads decode")
}

/// The tests that read files, which the browser target cannot (plan R04, Design note 12).
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    /// The files of `tests/golden/worker/`, each one a test writes and compares
    /// (decision-r10-t16-contracts §4): a stray file fails the test below, as before.
    const WORKER_GOLDENS: [&str; 5] = [
        "earth_like.golden",
        "maturity_layer.golden",
        "moon_like.golden",
        "terrain_test.golden",
        "terrain_test_weathered.golden",
    ];

    /// The largest the terrain test's golden may be, bytes: `cube_sphere.golden`'s, the crate's
    /// largest when it was written, the plan's "about 360 kB" (decision-r10-t16-contracts §4), since
    /// the client's kept scene ships it.
    const TERRAIN_TEST_MAX_BYTES: u64 = 361_605;

    /// The worker's golden directory.
    fn worker_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("worker")
    }

    /// Every golden file of `tests/golden/worker/` carries `GENERATOR_VERSION`, and they are the
    /// five the tests write.
    #[test]
    fn every_worker_golden_carries_the_generator_version() {
        let expected = format!("# generator_version = {}", crate::generator_version());
        let mut names = BTreeSet::new();
        for entry in
            std::fs::read_dir(worker_dir()).expect("the worker's golden directory is readable")
        {
            let path = entry.expect("a directory entry is readable").path();
            let text = std::fs::read_to_string(&path).expect("a golden file is readable");
            assert_eq!(
                text.lines().next(),
                Some(expected.as_str()),
                "{}",
                path.display()
            );
            names.insert(
                path.file_name()
                    .expect("a directory entry has a name")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        assert_eq!(
            names,
            WORKER_GOLDENS
                .iter()
                .map(|&name| name.to_owned())
                .collect::<BTreeSet<_>>()
        );
    }

    /// The terrain test's goldens, `TERRAIN TEST`'s and `TEST MOON` weathered's, are each within
    /// their cap, the crate's largest golden of their writing, since the client's kept scene ships
    /// them.
    #[test]
    fn the_terrain_test_goldens_are_within_their_size_cap() {
        for name in ["terrain_test.golden", "terrain_test_weathered.golden"] {
            let bytes = std::fs::metadata(worker_dir().join(name))
                .expect("the terrain test's golden is blessed")
                .len();
            eprintln!("{name}: {bytes} B of {TERRAIN_TEST_MAX_BYTES} B");
            assert!(
                bytes <= TERRAIN_TEST_MAX_BYTES,
                "{name}: {bytes} B, past {TERRAIN_TEST_MAX_BYTES} B"
            );
        }
    }
}
