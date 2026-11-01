//! `TERRAIN TEST`'s fields and their text (plan R10, R10.T16.d; decision-r10-t16-contracts §4):
//! the Moon-like world's two surveys about the edge of faces +x and +y near their corner with +z,
//! the whole body's surveys beside them (R10.T14.c's whole-body run; research-leans-20 §2), the
//! payloads R09.T19.b answers over either, and their text.
//!
//! The text is `tests/golden/worker/terrain_test.golden`'s format, which `worker::golden` blesses
//! for the patch and the client's `terrainGolden.ts` reads. The views check's whole-body run reads
//! the whole body's, which `examples/whole_terrain_test.rs` writes uncommitted, at about 4.5 MB, under
//! `target/views-check/terrain/`. One field gives both, cut twice: the whole body's cells over the
//! patch's survey are the patch's to the bit (`worker::golden`'s test of the two).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use hyperion_base::rng::DetailSeed;
use hyperion_base::units::Metres;

use crate::cube::{FaceUv, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir};
use crate::field::{
    BodyRef, ClimateModelKind, CoarseField, Cover, CoverRange, FieldHeader, FieldHeaderParts,
    FieldView, PaletteEntry, PaletteRole, PrecipitationSource, ResolutionCode,
    SYNTHESIS_MARGIN_CELLS, cell_at_index, cell_index,
};
use crate::wire::{decode_payload, encode_within};
use crate::worker::{SigmaSurvey, WorkerFields};

/// The detail seed `TERRAIN TEST`'s moon draws its detail from: R09's whole-function tests' (the
/// crate's `synth::test_support::SEED`), which its golden's `seed` line names.
pub const TERRAIN_TEST_SEED: DetailSeed = DetailSeed::new(0x6173_7365_6d62_6c79);

/// The fixture's largest block, bytes: 4 KiB, so that each payload is several blocks, as the
/// client's chunked answers are.
#[cfg(test)]
const TERRAIN_TEST_BLOCK_BYTES: usize = 4096;

/// The whole body's largest block in a whole answer, bytes: 16 KiB, so that its 1.7 MB come in 68
/// blocks, each a chunk of the client's request, under its 257 (`bulk.ts`' `MAX_BULK_CHUNKS`).
const WHOLE_TERRAIN_TEST_BLOCK_BYTES: usize = 16_384;

/// The whole body's largest block in a delta, bytes: 3 KiB, so that its revision's rise, the
/// patch's second survey's cells alone at a finer code (3.5 kB in one block of 4 KiB), comes in two,
/// for the views check's forced loss mid-stream; a cell with the craters reaching it (cell 3934's)
/// overflows a block of 2 KiB.
const WHOLE_TERRAIN_TEST_DELTA_BLOCK_BYTES: usize = 3072;

/// The first survey's centre, not normalised: on the edge between faces +x and +y, at v = 0.8 of
/// both, some five cells of level 6 from the corner they share with face +z, so that the survey
/// straddles the edge and its margin reaches round the corner (T8.b's third place, "a face edge
/// near a cube corner").
#[cfg(test)]
const FIRST_CENTRE: [f64; 3] = [1.0, 1.0, 0.8];

/// The second survey's centre, along the same edge away from the corner, at v = 0.62: some four
/// cells beyond the first's, so that the two meet and the second's finer code wins where they do.
const SECOND_CENTRE: [f64; 3] = [1.0, 1.0, 0.62];

/// The cosine of each survey's angular radius about its centre, which a surveyed cell's centre is
/// within: 0.9985, about 3.1°, some two cells of level 6 there.
const SURVEY_DOT: f64 = 0.9985;

/// The first survey's resolution and the second's, m: R09's orbital 50 m, and 20 m.
const FIRST_RESOLUTION_M: f64 = 50.0;
const SECOND_RESOLUTION_M: f64 = 20.0;

/// One of `TERRAIN TEST`'s payloads: R09.T19.b's answer, at the body's revision `revision`, to a
/// `surface_field` with `have_revision` `have`, cut into the fixture's blocks.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerrainTestPayload {
    revision: u32,
    have: Option<u32>,
    blocks: Vec<Vec<u8>>,
}

impl TerrainTestPayload {
    /// The body's coverage revision the payload reflects, from 1.
    #[must_use]
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// The revision the asker held, `None` for the whole field.
    #[must_use]
    pub fn have(&self) -> Option<u32> {
        self.have
    }

    /// The payload's blocks, in order.
    #[must_use]
    pub fn blocks(&self) -> &[Vec<u8>] {
        &self.blocks
    }
}

/// `TERRAIN TEST`'s field over a Moon-like world: its coverage at revisions 1 and 2, as the
/// server's survey log folds it (the finer code winning), and the payloads R09.T19.b answers over
/// them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerrainTestFixture {
    body: BodyRef,
    surveys: [Cover; 2],
    payloads: [TerrainTestPayload; 4],
}

impl TerrainTestFixture {
    /// The world's body.
    #[must_use]
    pub fn body(&self) -> BodyRef {
        self.body
    }

    /// The coverage at revisions 1 and 2.
    #[must_use]
    pub fn surveys(&self) -> &[Cover; 2] {
        &self.surveys
    }

    /// The first payload (revision 1, whole); the delta from it to revision 2; the whole field at
    /// revision 2; and the answer to an ask at the revision held, block 0 alone.
    #[must_use]
    pub fn payloads(&self) -> &[TerrainTestPayload; 4] {
        &self.payloads
    }

    /// The blocks a session posts in turn: the first payload's, then the delta's.
    pub fn first_then_delta(&self) -> impl Iterator<Item = &[u8]> {
        self.payloads[0]
            .blocks
            .iter()
            .chain(&self.payloads[1].blocks)
            .map(Vec::as_slice)
    }
}

/// A resolution code of `metres`.
#[must_use]
fn code(metres: f64) -> ResolutionCode {
    ResolutionCode::from_resolution(Metres::new(metres))
        .expect("the fixture's resolutions are finite and above 0")
}

/// The unit direction of `cell`'s centre.
#[must_use]
fn centre(cell: PatchKey) -> [f64; 3] {
    let side = f64::from(1_u32 << cell.level());
    unit_dir(face_uv_to_xyz(FaceUv {
        face: cell.face(),
        u: st_to_uv((f64::from(cell.i()) + 0.5) / side),
        v: st_to_uv((f64::from(cell.j()) + 0.5) / side),
    }))
}

/// Every cell of `field`'s level.
#[must_use]
fn every_cell(field: &CoarseField) -> BTreeSet<PatchKey> {
    let level = field.header().level();
    (0..level.cell_count())
        .map(|index| cell_at_index(level.get(), index).expect("an index below the count"))
        .collect()
}

/// The cells of `field`'s level whose centres lie within the angle whose cosine is `dot` of the
/// direction `towards`, not normalised.
#[must_use]
fn cells_about(field: &CoarseField, towards: [f64; 3], dot: f64) -> BTreeSet<PatchKey> {
    let towards = unit_dir(towards);
    every_cell(field)
        .into_iter()
        .filter(|&cell| {
            let p = centre(cell);
            p[0] * towards[0] + p[1] * towards[1] + p[2] * towards[2] >= dot
        })
        .collect()
}

/// The cover of `cells` at the code `code_of` gives each.
#[must_use]
fn cover(cells: &BTreeSet<PatchKey>, code_of: impl Fn(PatchKey) -> ResolutionCode) -> Cover {
    let mut indices: Vec<(u32, PatchKey)> = cells.iter().map(|&c| (cell_index(c), c)).collect();
    indices.sort_unstable();
    Cover::from_ranges(indices.iter().map(|&(i, c)| {
        CoverRange::new(i, i + 1, code_of(c)).expect("a cell's run holds one index")
    }))
    .expect("the cells' runs rise and do not overlap")
}

/// `bytes` cut at each block's length, the `u32` at offset 6 of its frame.
#[must_use]
fn split(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut blocks = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let length = u32::from_le_bytes(
            rest[6..10]
                .try_into()
                .expect("a block's frame holds its length"),
        );
        let (block, after) =
            rest.split_at(usize::try_from(length).expect("a block's length fits a usize"));
        blocks.push(block.to_vec());
        rest = after;
    }
    blocks
}

/// The fixture over `field` of the surveys `first` and `both` (revision 1's and 2's), with the
/// payloads R09.T19.b answers over them, a whole answer's in blocks of at most `whole_block` bytes
/// and every other's of at most `delta_block`.
#[must_use]
fn fixture_of(
    field: &CoarseField,
    first: Cover,
    both: Cover,
    whole_block: usize,
    delta_block: usize,
) -> TerrainTestFixture {
    let encode = |c: &Cover, since: Option<&Cover>| {
        let max_block = if since.is_none() {
            whole_block
        } else {
            delta_block
        };
        split(&encode_within(
            field,
            c,
            since,
            SYNTHESIS_MARGIN_CELLS,
            max_block,
        ))
    };
    let payloads = [
        TerrainTestPayload {
            revision: 1,
            have: None,
            blocks: encode(&first, None),
        },
        TerrainTestPayload {
            revision: 2,
            have: Some(1),
            blocks: encode(&both, Some(&first)),
        },
        TerrainTestPayload {
            revision: 2,
            have: None,
            blocks: encode(&both, None),
        },
        TerrainTestPayload {
            revision: 2,
            have: Some(2),
            blocks: encode(&both, Some(&both)),
        },
    ];
    TerrainTestFixture {
        body: field.header().body(),
        surveys: [first, both],
        payloads,
    }
}

/// `TERRAIN TEST`'s patch over `field`, a Moon-like world of the fixture's level (`TEST MOON`'s,
/// or `TEST MOON` weathered's): the first survey at 50 m, then the second's cells at 20 m with it.
#[cfg(test)]
#[must_use]
pub(crate) fn terrain_test_fixture(field: &CoarseField) -> TerrainTestFixture {
    let first = cells_about(field, FIRST_CENTRE, SURVEY_DOT);
    let second = cells_about(field, SECOND_CENTRE, SURVEY_DOT);
    let both: BTreeSet<PatchKey> = first.union(&second).copied().collect();
    fixture_of(
        field,
        cover(&first, |_| code(FIRST_RESOLUTION_M)),
        cover(&both, |c| {
            code(if second.contains(&c) {
                SECOND_RESOLUTION_M
            } else {
                FIRST_RESOLUTION_M
            })
        }),
        TERRAIN_TEST_BLOCK_BYTES,
        TERRAIN_TEST_BLOCK_BYTES,
    )
}

/// `TERRAIN TEST`'s whole body over `field`: every cell surveyed at 50 m, as `just place-ship
/// --survey orbital 50` surveys a served world's, then the patch's second survey's cells at 20 m
/// with them, so that the revision rises over the same cells as the patch's; its whole answers in
/// blocks of 16 KiB and its deltas of 3 KiB.
#[must_use]
pub fn whole_terrain_test_fixture(field: &CoarseField) -> TerrainTestFixture {
    let every = every_cell(field);
    let second = cells_about(field, SECOND_CENTRE, SURVEY_DOT);
    fixture_of(
        field,
        cover(&every, |_| code(FIRST_RESOLUTION_M)),
        cover(&every, |c| {
            code(if second.contains(&c) {
                SECOND_RESOLUTION_M
            } else {
                FIRST_RESOLUTION_M
            })
        }),
        WHOLE_TERRAIN_TEST_BLOCK_BYTES,
        WHOLE_TERRAIN_TEST_DELTA_BLOCK_BYTES,
    )
}

/// The cover the blocks of a payload hold, every block's cells with their codes in turn.
///
/// # Panics
///
/// If the blocks do not decode, or hold their cells out of index order.
#[must_use]
pub(crate) fn payload_cover(blocks: &[Vec<u8>]) -> Cover {
    let decoded = decode_payload(&blocks.concat()).expect("the fixture's payloads decode");
    Cover::from_ranges(
        decoded
            .iter()
            .flat_map(|block| block.cover().ranges().iter().copied()),
    )
    .expect("a payload's blocks hold its cells in index order")
}

/// `bytes` in lowercase hexadecimal.
#[must_use]
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(2 * bytes.len()), |mut text, b| {
            write!(text, "{b:02x}").expect("writing to a String cannot fail");
            text
        })
}

/// Runs in the golden's form, `-` for none.
#[must_use]
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

/// `cover`'s runs in the terrain test's form, each `<first>-<past last>:<code>`, `-` for none.
#[must_use]
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

/// A palette role as the wire spells it (`PaletteRoleDto`).
#[must_use]
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
#[must_use]
fn climate_name(kind: ClimateModelKind) -> &'static str {
    match kind {
        ClimateModelKind::EnergyBalance => "energy_balance",
        ClimateModelKind::LockedEnergyBalance => "locked_energy_balance",
        ClimateModelKind::RadiativeEquilibrium => "radiative_equilibrium",
        ClimateModelKind::Isothermal => "isothermal",
    }
}

/// A precipitation source as the wire spells it (`PrecipitationSourceDto`).
#[must_use]
fn precipitation_name(source: PrecipitationSource) -> &'static str {
    match source {
        PrecipitationSource::Heuristic => "heuristic",
    }
}

/// A float in the terrain test's form: `0x` and its bits.
///
/// # Panics
///
/// If `x` is a NaN, whose bits are unspecified.
#[expect(
    clippy::disallowed_methods,
    reason = "printed into the terrain golden's text, never hashed: the testkit's `bits`, which the \
              library may not depend on, so that the views check's tool writes the format without it"
)]
#[must_use]
fn float(x: f64) -> String {
    assert!(!x.is_nan(), "the bits of a NaN are unspecified");
    format!("0x{:016x}", x.to_bits())
}

/// The line `dto <key> <x>` with the decimal for the reader.
#[must_use]
fn dto_f64(key: &str, x: f64) -> String {
    format!("dto {key} {}  # {x:?}", float(x))
}

/// The line `dto <key>`, then `x` as [`dto_f64`] gives it, or `-` for none.
#[must_use]
fn dto_option(key: &str, x: Option<f64>) -> String {
    match x {
        Some(x) => dto_f64(key, x),
        None => format!("dto {key} -"),
    }
}

/// The header's DTO lines, in `SurfaceHeaderDto`'s order.
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
#[must_use]
fn dto_lines(header: &FieldHeader) -> Vec<String> {
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
    let mut lines = vec![
        format!(
            "dto body {:016x}.{:04x}",
            body.raw_system_id(),
            body.body_index()
        ),
        format!("dto level {}", header.level().get()),
        dto_f64("radius_m", radius.value()),
        dto_f64("equatorial_radius_m", figure.equatorial_radius_m),
        dto_f64("polar_radius_m", figure.polar_radius_m),
        dto_f64("boundary_diameter_m", header.boundary_diameter().value()),
        dto_f64("sea_level_m", sea_level.value()),
        dto_f64("lapse_rate_k_per_m", *lapse_rate_k_per_m),
        dto_f64("realised_sigma_h_m", realised_sigma_h.value()),
        dto_f64("realised_relief_m", realised_relief.value()),
        format!("dto months {months}"),
        dto_f64("season_eccentricity", *season_eccentricity),
        dto_f64("reference_temperature_k", reference_temperature.value()),
        dto_f64("surface_age_gyr", surface_age.value()),
        dto_f64("surface_pressure_pa", surface_pressure.value()),
        dto_option("albedo_scale", *albedo_scale),
        format!("dto climate_model {}", climate_name(*climate_model)),
        format!("dto precipitation {}", precipitation_name(*precipitation)),
    ];
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
        lines.push(format!(
            "dto palette {index} {} {} {b} {v} {r} {phase_row} - {feo}",
            substance.as_str(),
            role_name(*role),
        ));
    }
    lines.push(format!(
        "dto main_liquid {}",
        main_liquid.map_or_else(|| "-".to_owned(), |m| m.to_string())
    ));
    lines.push(dto_option("maturity", *maturity));
    lines.push(dto_option("organic_cover", *organic_cover));
    lines.push(dto_option("weathering_rate", *weathering_rate));
    lines
}

/// The terrain test's whole text of `fixture` over `field`, its detail seed `seed`: the golden's
/// header line, `# generator_version = <the crate's>`, as the testkit's `GoldenWriter` writes it,
/// then [`terrain_test_lines`], each ended by a line feed. The views check's tool writes it; for
/// the patch it is `terrain_test.golden`'s text, which has no maturity's lines.
///
/// # Panics
///
/// As [`terrain_test_lines`] does.
#[must_use]
pub fn terrain_test_text(
    fixture: &TerrainTestFixture,
    field: &CoarseField,
    seed: DetailSeed,
) -> String {
    let mut text = format!("# generator_version = {}\n", crate::generator_version());
    for line in terrain_test_lines(fixture, field, seed) {
        text.push_str(&line);
        text.push('\n');
    }
    text
}

/// The terrain test's text of `fixture` over `field`, its detail seed `seed`, after the golden's
/// header line: its body and seed, its header's DTO lines, its surveys, its payloads with their
/// covers and blocks, and the change each block posted in turn makes to a client's fields.
///
/// # Panics
///
/// If a palette entry of `field` carries a liquid record, or the fixture's blocks are not taken.
#[must_use]
pub fn terrain_test_lines(
    fixture: &TerrainTestFixture,
    field: &CoarseField,
    seed: DetailSeed,
) -> Vec<String> {
    let mut lines = vec![
        format!(
            "body {:016x}.{:04x}",
            fixture.body.raw_system_id(),
            fixture.body.body_index()
        ),
        format!("seed {:016x}", seed.get()),
    ];
    lines.extend(dto_lines(field.header()));
    for (revision, survey) in (1..).zip(&fixture.surveys) {
        lines.push(format!("survey {revision} {}", coded_runs(survey)));
    }
    for (n, payload) in (1..).zip(&fixture.payloads) {
        let have = payload
            .have
            .map_or_else(|| "-".to_owned(), |h| h.to_string());
        lines.push(format!("payload {n} {} {have}", payload.revision));
        lines.push(format!(
            "cover {n} {}",
            coded_runs(&payload_cover(&payload.blocks))
        ));
        for (index, block) in payload.blocks.iter().enumerate() {
            lines.push(format!("block {n} {index} {}", hex(block)));
        }
    }
    let mut fields = WorkerFields::new();
    for (ordinal, block) in fixture.first_then_delta().enumerate() {
        let change = if ordinal == 0 {
            fields.init(fixture.body, block, SigmaSurvey::Surveyed)
        } else {
            fields.chunk(fixture.body, block)
        }
        .expect("the fixture's blocks are taken");
        lines.push(format!("change {ordinal} {}", runs(change.runs())));
        lines.push(format!("held {ordinal} {}", runs(change.held_runs())));
    }
    lines
}
