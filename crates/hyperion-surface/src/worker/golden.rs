//! The worker's golden files, blessed natively and compared on `wasm32-wasip1` and
//! `wasm32-unknown-unknown` through the testkit's embedded arm: `tests/golden/worker/moon_like.golden`
//! (plan R10, T6.b), a fixture field's payload blocks and what [`WorkerFields`] answers over them,
//! and `tests/golden/worker/terrain_test.golden` (R10.T16.d; decision-r10-t16-contracts §4), the
//! payloads of the client's kept scene `TERRAIN TEST` with the answers R09.T19.b gives with them.
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

use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, f32_digest, f64_digest};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::tests::{
    WORLD, cell, face_uv, fixture, lit, payload_cover, probes, sight_lines, terrain_fixture,
};
use super::*;
use crate::bake::WeightsLayer;
use crate::field::{
    ClimateModelKind, Cover, FieldHeader, FieldHeaderParts, PaletteEntry, PaletteRole,
    PrecipitationSource,
};
use crate::patch::{NormalScale, VertexPath};
use crate::synth::test_support::{SEED, world};

/// A digest of `bytes` in the golden's form: each as an `f64`, which holds it exactly, as
/// R10.T6.a's goldens take it.
fn byte_digest(bytes: impl IntoIterator<Item = u8>) -> u64 {
    f64_digest(&bytes.into_iter().map(f64::from).collect::<Vec<_>>())
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

/// Writes `table`'s rows, as of `when`.
fn write_levels(out: &mut GoldenWriter, when: &str, table: &[f64]) {
    for (n, row) in table.chunks(LEVEL_TABLE_STRIDE).enumerate() {
        out.line(&format!("level {when} {n} {}", bits_of(row)));
    }
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
    let palette: Vec<String> = l
        .palette()
        .iter()
        .map(|slot| {
            slot.map_or_else(
                || "none".to_owned(),
                |m| {
                    let entry = m.entry().map_or_else(|| "-".to_owned(), |e| e.to_string());
                    format!("{}:{entry}", u8::from(m.class()))
                },
            )
        })
        .collect();
    out.line(&format!("palette {}", palette.join(" ")));
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
            fields.init(f.body, block)
        } else {
            fields.chunk(f.body, block)
        }
        .expect("the fixture's blocks are taken");
        out.line(&format!("change {ordinal} {}", runs(change.runs())));
        out.line(&format!("held {ordinal} {}", runs(change.held_runs())));
        if ordinal == 0 {
            write_levels(
                &mut out,
                "first",
                &fields.level_table(f.body).expect("held"),
            );
        }
    }
    write_levels(&mut out, "last", &fields.level_table(f.body).expect("held"));
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
    let f = terrain_fixture();
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    out.line(&format!(
        "body {:016x}.{:04x}",
        f.body.raw_system_id(),
        f.body.body_index()
    ));
    out.line(&format!("seed {:016x}", SEED.get()));
    write_dto(&mut out, world(WORLD).header());
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
            fields.init(f.body, block)
        } else {
            fields.chunk(f.body, block)
        }
        .expect("the fixture's blocks are taken");
        out.line(&format!("change {ordinal} {}", runs(change.runs())));
        out.line(&format!("held {ordinal} {}", runs(change.held_runs())));
    }
    out.finish()
}

#[test]
fn worker_matches_its_terrain_test_golden() {
    golden!("worker/terrain_test", &terrain_text());
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
    const WORKER_GOLDENS: [&str; 2] = ["moon_like.golden", "terrain_test.golden"];

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
    /// two the tests write.
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

    /// The terrain test's golden is within its cap, the crate's largest golden of its writing.
    #[test]
    fn the_terrain_test_golden_is_within_its_size_cap() {
        let bytes = std::fs::metadata(worker_dir().join("terrain_test.golden"))
            .expect("the terrain test's golden is blessed")
            .len();
        assert!(
            bytes <= TERRAIN_TEST_MAX_BYTES,
            "{bytes} B, past {TERRAIN_TEST_MAX_BYTES} B"
        );
    }
}
