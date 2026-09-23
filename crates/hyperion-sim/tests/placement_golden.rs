//! The first generated stars, pinned: whole cells and single IDs (plan 03, P03.T8.d).
//!
//! Two galaxies, three golden files. `placement/cells` holds eight whole cells of each galaxy — the
//! candidate count, which candidates the thinning kept, and each system's ID, light-year, offsets,
//! primary initial mass, age and picked component — `placement/digests` a digest of every record of
//! the same cells, and `placement/ids` ten IDs taken through `resolve`, eight of which name a system
//! and two of which do not. Between them they pin every draw placement makes: the Poisson count on
//! the cell's stream, the three position words, the acceptance mark and the component it picks, and
//! the mass and age marks.
//!
//! Every candidate's outcome is pinned for every cell, as a list of the kept indices. The marks are
//! pinned for the first [`FULL_RECORDS`] systems of a cell, which is all of them except in the two
//! bulge cells of layer E. That is a deviation from the task's "every record in full", and it is
//! there because a 128 ly cell anywhere the bulge is the bulge holds thousands of systems: 1,700 at
//! [`BULGE_LY`], ten thousand at 1,000 ly and a quarter of a million at the centre. Writing them all
//! out would make one file several times the size of every golden in the repository together. The
//! first sixty-four do not pin the rest, though: a mark drawn for the wrong candidate only past some
//! index, or an age taken from the wrong component there, moves no early system. So
//! `placement/digests` pins every record of every pinned cell as one FNV-1a digest a cell.
//!
//! # Why these cells
//!
//! - **One cell of each layer at the Sun-like point.** This is the density every figure in the
//!   brainstorm and the plan is quoted at, and the five layers differ there by a factor of 4,096 in
//!   cell volume and by 42 in systems per cell, so one cell each pins the whole layer table: the
//!   cell sizes, the mass bands, the index widths and the share matrix's five rows.
//! - **A layer-A and a layer-E cell in the outer bulge.** An old, dense place where the bulge and the
//!   long bar contribute as much as the discs, so the component pick, and with it the age distribution
//!   a system inherits, is exercised as the solar neighbourhood cannot exercise it. Layer A because
//!   its 16-bit index field is the tightest, layer E because a 128 ly cell spans the largest range of
//!   density of any cell here, which is what the thinning's bound has to cover.
//!
//!   It is the *outer* bulge, at [`BULGE_LY`] — where the bulge and the long bar still supply about
//!   half of the density, and a layer-E cell holds some 1,700 systems against ten thousand at
//!   1,000 ly — and not the centre, for three reasons. A golden file is a list a person
//!   has to be able to read, and it is the cell's size that decides that. The centre's layer-A cells
//!   run towards the index capacity, which is [`check_index_headroom`]'s subject and is tested
//!   directly in `placement::headroom`, not something a golden should re-assert. And the closer to
//!   the centre a cell sits, the more of its candidate count comes from the nuclear disc, whose
//!   parameters plan 02 may still tune: a golden there would be re-blessed by work that moves no
//!   star at the Sun.
//! - **A layer-E cell on a young arm ridge outside the bar's end.** The cell whose bound is tightest
//!   (brainstorm, "Exact placement by thinning": the true maximum beat a corner estimate by 12% for
//!   a sharp arm in a 128 ly cell near the bar), so a change to the arm factor's bound shows up here
//!   as a changed candidate count before it shows up anywhere else.
//!
//! # Why these galaxies
//!
//! One is `GalaxyParams::milky_way_like()`, the fixture the brainstorm's comparisons use, so these
//! records are the ones the plan's figures describe. The other is drawn from a seed, which pins the
//! path through the parameter draw as well: a galaxy whose bar, arms and nuclear disc are wherever
//! its seed put them.
//!
//! This is the task that adds the first generated stars, so it carries this plan's one
//! `GENERATOR_VERSION` bump, 7 → 8.

#[expect(
    dead_code,
    reason = "the golden cells use the Sun-like point of tests/common alone"
)]
mod common;

use common::sunlike_point;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
#[cfg(doc)]
use hyperion_sim::galaxy::placement::check_index_headroom;
use hyperion_sim::galaxy::placement::{
    CandidateOutcome, CellKey, ResolveSystemError, STELLAR_LAYERS, SystemRecord, candidate_count,
    evaluate_candidate, generate_cell, resolve,
};
use hyperion_sim::id::{Layer, SystemId, SystemIdKind};
use hyperion_sim::math;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The seed of the pinned fixture galaxy.
const FIXTURE_SEED: u64 = 0x0308_d000_0000_0000;

/// The seed of the pinned drawn galaxy.
const DRAWN_SEED: u64 = 0x0308_d000_5eed_0001;

/// Where the outer-bulge cells sit: in the plane, at 45° to both axes (module documentation).
const BULGE_LY: f64 = 3_200.0;

/// How many of a cell's systems have their marks written out. Every cell here but the two dense
/// bulge ones holds fewer than this.
const FULL_RECORDS: usize = 64;

/// The most systems a pinned cell may hold: past this even a list of kept candidate indices is more
/// than a golden file should carry, and the cell has been put somewhere too dense.
const MOST_SYSTEMS: usize = 2_500;

/// The two pinned galaxies, with the tag their labels carry in the golden files.
fn galaxies() -> [(&'static str, Galaxy); 2] {
    [
        (
            "mw",
            Galaxy::from_params(Seed::new(FIXTURE_SEED), GalaxyParams::milky_way_like()),
        ),
        ("drawn", Galaxy::new(Seed::new(DRAWN_SEED))),
    ]
}

/// A point in the plane at 45° to both axes, `r` light-years from the centre.
fn diagonal(r: f64) -> GalacticPosition {
    let half = 0.5_f64.sqrt();
    GalacticPosition::from_light_years([r * half, r * half, 0.0])
        .expect("a point a few thousand light-years out is inside the cube")
}

/// A point on the young disc's first arm ridge, just outside the bar's end.
fn arm_ridge(galaxy: &Galaxy) -> GalacticPosition {
    let arms = galaxy.fields().arms();
    let r = 1.15 * arms.bar_half_length().value();
    let (sin, cos) = math::sin_cos(arms.ridge_azimuth(r, 0));
    GalacticPosition::from_light_years([r * cos, r * sin, 0.0])
        .expect("a point on a ridge is inside the cube")
}

/// The eight cells of one galaxy that are pinned in full, with the label each carries.
fn pinned_cells(galaxy: &Galaxy) -> Vec<(String, &'static str, CellKey)> {
    let sun = sunlike_point(galaxy);
    let mut cells = Vec::new();
    let mut add = |label: String, what: &'static str, layer: Layer, at: &GalacticPosition| {
        let key = CellKey::containing(layer, at).expect("a stellar layer at a point in the cube");
        cells.push((label, what, key));
    };
    for spec in STELLAR_LAYERS {
        let layer = spec.layer();
        add(
            format!("sun.{}", layer.letter()),
            "the Sun-like point",
            layer,
            &sun,
        );
    }
    let bulge = diagonal(BULGE_LY);
    add("bulge.A".to_owned(), "the outer bulge", Layer::A, &bulge);
    add("bulge.E".to_owned(), "the outer bulge", Layer::E, &bulge);
    add(
        "arm.E".to_owned(),
        "a young arm ridge outside the bar",
        Layer::E,
        &arm_ridge(galaxy),
    );
    cells
}

/// Writes one system: its ID, where it is at the epoch, its marks and the component it was picked
/// for.
fn write_record(w: &mut GoldenWriter, label: &str, record: &SystemRecord) {
    let component = record
        .component()
        .expect("every record here was placed by the grid");
    let ly = record.epoch_position().cell().to_array();
    let offsets = record.epoch_position().offset_metres();
    w.u64_hex(&format!("{label}.id"), record.id().raw());
    w.line(&format!(
        "{label}.ly = {} {} {}, component {} ({})",
        ly[0],
        ly[1],
        ly[2],
        component.index(),
        record.population().name()
    ));
    for (axis, offset) in ["x", "y", "z"].into_iter().zip(offsets) {
        w.f64(&format!("{label}.offset_{axis}_m"), offset);
    }
    w.f64(
        &format!("{label}.mass_msun"),
        record.primary_initial_mass().value(),
    );
    w.f64(&format!("{label}.age_yr"), record.age_at_epoch().value());
}

/// Writes one whole cell: its candidate count, which of its candidates the thinning kept, and the
/// systems themselves in candidate-index order.
///
/// Every candidate's outcome is pinned, whatever the cell holds: the count comes from the cell's own
/// stream and the index list is the acceptance mark's verdict on each candidate in turn, so together
/// they pin the thinning of the whole cell in a line per sixteen candidates. The marks — position,
/// mass, age and the component picked — are written for the first [`FULL_RECORDS`] systems, which is
/// every system of every cell here but the two dense ones (see the module documentation).
fn write_cell(w: &mut GoldenWriter, galaxy: &Galaxy, label: &str, what: &str, key: CellKey) {
    let candidates = candidate_count(galaxy, key);
    let mut cell = Vec::new();
    generate_cell(galaxy, key, &mut cell);
    assert!(
        cell.len() <= MOST_SYSTEMS,
        "{label} ({what}) holds {} systems of {candidates} candidates, which is no size for a \
         golden file even as an index list: move the cell",
        cell.len()
    );
    println!("{label}: {candidates} candidates, {} systems", cell.len());
    let [x, y, z] = key.gen_cell().to_array();
    w.line(&format!(
        "# {label}: {what}, layer {} cell ({x}, {y}, {z}), {} ly",
        key.layer().letter(),
        key.size_ly()
    ));
    w.u64_hex(&format!("{label}.cell_word"), key.cell_word());
    w.line(&format!("{label}.candidates = {candidates}"));
    w.line(&format!("{label}.systems = {}", cell.len()));
    // Which candidates survived, sixteen to a line: the whole cell's thinning, compactly.
    let indices: Vec<u32> = cell
        .iter()
        .map(|record| match record.id().kind() {
            SystemIdKind::Grid(grid) => grid.index(),
            other => panic!("{label}: a placed system has a grid ID, not {other:?}"),
        })
        .collect();
    for (chunk, kept) in indices.chunks(16).enumerate() {
        let list: Vec<String> = kept.iter().map(u32::to_string).collect();
        w.line(&format!(
            "{label}.kept[{}] = {}",
            chunk * 16,
            list.join(" ")
        ));
    }
    for (index, record) in cell.iter().take(FULL_RECORDS).enumerate() {
        write_record(w, &format!("{label}.s{index}"), record);
    }
}

/// Eight whole cells of each of two galaxies, pinned (P03.T8.d).
#[test]
fn placed_cells_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (tag, galaxy) in galaxies() {
        w.line(&format!("# galaxy {tag}, {}", galaxy.seed()));
        for (label, what, key) in pinned_cells(&galaxy) {
            write_cell(&mut w, &galaxy, &format!("{tag}.{label}"), what, key);
        }
    }
    golden!("placement/cells", w.as_str());
}

/// The FNV-1a offset basis and prime, 64-bit (Fowler, Noll and Vo): a digest that any changed bit
/// of any record moves.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Folds one record into an FNV-1a digest, every field bit for bit: the ID, the light-year and the
/// offsets of the epoch position, the primary initial mass, the age and the picked component.
fn digest_record(hash: &mut u64, record: &SystemRecord) {
    let component = record
        .component()
        .expect("every record here was placed by the grid");
    let ly = record.epoch_position().cell().to_array();
    let offsets = record.epoch_position().offset_metres();
    let words = [
        record.id().raw(),
        i64::from(ly[0]).cast_unsigned(),
        i64::from(ly[1]).cast_unsigned(),
        i64::from(ly[2]).cast_unsigned(),
        bits(offsets[0]),
        bits(offsets[1]),
        bits(offsets[2]),
        bits(record.primary_initial_mass().value()),
        bits(record.age_at_epoch().value()),
        u64::try_from(component.index()).expect("a component index is small"),
    ];
    for word in words {
        for byte in word.to_le_bytes() {
            *hash = (*hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
        }
    }
}

/// Every record of every pinned cell, as one digest a cell (P03.T8.d, completed in validation).
///
/// `placement/cells` writes the marks of a cell's first [`FULL_RECORDS`] systems only, so on its
/// own it pins nothing past them: in the two bulge cells of layer E a mark drawn for the wrong
/// candidate past the 64th, or an age taken from the wrong component, left every golden file and
/// every other test unchanged (found by perturbing the generator in validation). This file closes
/// that gap in a line or two a cell: each cell's system count and an FNV-1a digest of all its
/// records, in index order, every field bit for bit.
#[test]
fn every_record_of_the_pinned_cells_is_digested() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let mut cell = Vec::new();
    for (tag, galaxy) in galaxies() {
        w.line(&format!("# galaxy {tag}, {}", galaxy.seed()));
        for (label, _, key) in pinned_cells(&galaxy) {
            generate_cell(&galaxy, key, &mut cell);
            let mut hash = FNV_OFFSET;
            for record in &cell {
                digest_record(&mut hash, record);
            }
            w.line(&format!("{tag}.{label}.systems = {}", cell.len()));
            w.u64_hex(&format!("{tag}.{label}.digest"), hash);
        }
    }
    golden!("placement/digests", w.as_str());
}

/// The ten pinned IDs: where each comes from, as a label, which galaxy it belongs to and the
/// candidate it names.
///
/// Eight name a system: the first system of the pinned cells that hold one, across four layers and
/// both galaxies. A layer-A or layer-B cell at the Sun-like point expects about one candidate and
/// often keeps none, so the IDs are taken from the coarser cells there and from the bulge's layer-A
/// cell, which is dense enough to be certain.
///
/// Two do not name a system: a candidate the thinning rejected, and the index one past a cell's
/// candidate count, which is the first index that cell never draws. Both are the failures the
/// brainstorm's "A well-formed ID does not always name a system" describes, and both are what an ID
/// read from a save can turn out to be.
fn pinned_ids(galaxies: &[(&'static str, Galaxy); 2]) -> Vec<(String, usize, SystemId)> {
    let (_, fixture) = &galaxies[0];
    let (_, drawn) = &galaxies[1];
    let mut ids = Vec::new();
    let mut cell = Vec::new();
    let mut first_system = |label: &str, which: usize, key: CellKey| {
        let galaxy = &galaxies[which].1;
        generate_cell(galaxy, key, &mut cell);
        let id = cell
            .first()
            .unwrap_or_else(|| {
                panic!("the pinned cell {label} holds no system, so no ID can be taken from it")
            })
            .id();
        ids.push((label.to_owned(), which, id));
    };
    for layer in [Layer::C, Layer::D, Layer::E] {
        let key = CellKey::containing(layer, &sunlike_point(fixture))
            .expect("a stellar layer at the Sun-like point");
        first_system(&format!("mw.sun.{}.first", layer.letter()), 0, key);
    }
    let bulge_a = CellKey::containing(Layer::A, &diagonal(BULGE_LY)).expect("a cell in the bulge");
    first_system("mw.bulge.A.first", 0, bulge_a);
    let sun_c = CellKey::containing(Layer::C, &sunlike_point(drawn))
        .expect("a layer-C cell at the Sun-like point");
    first_system("drawn.sun.C.first", 1, sun_c);
    for (label, layer, at) in [
        ("bulge.A", Layer::A, diagonal(BULGE_LY)),
        ("bulge.E", Layer::E, diagonal(BULGE_LY)),
        ("arm.E", Layer::E, arm_ridge(drawn)),
    ] {
        let key = CellKey::containing(layer, &at).expect("a stellar layer inside the cube");
        first_system(&format!("drawn.{label}.first"), 1, key);
    }

    // A candidate the thinning rejected: the first in the fixture's layer-E cell in the bulge, a
    // cell whose density falls by a factor across it, so that some hundred of its candidates are
    // rejected. A cell at the Sun-like point can keep every candidate it draws, since the density
    // there barely varies over 128 ly.
    let thinned_in =
        CellKey::containing(Layer::E, &diagonal(BULGE_LY)).expect("a cell in the bulge");
    let thinned = (0..candidate_count(fixture, thinned_in))
        .find(|&index| evaluate_candidate(fixture, thinned_in, index) == CandidateOutcome::Thinned)
        .and_then(|index| thinned_in.candidate_id(index))
        .expect("a layer-E cell in the bulge thins some candidate");
    ids.push(("mw.bulge.E.thinned".to_owned(), 0, thinned));

    // The index one past the candidate count of the drawn galaxy's outer-bulge layer-A cell.
    let past = CellKey::containing(Layer::A, &diagonal(BULGE_LY)).expect("a cell in the bulge");
    let index = candidate_count(drawn, past);
    assert!(
        index < past.index_capacity(),
        "the cell's index field is full"
    );
    ids.push((
        "drawn.bulge.A.past_the_count".to_owned(),
        1,
        past.candidate_id(index)
            .expect("one past the count has an ID"),
    ));
    ids
}

/// Ten IDs taken through `resolve` as an ID from a save is, pinned (P03.T8.d).
#[test]
fn resolved_ids_are_pinned() {
    let galaxies = galaxies();
    let ids = pinned_ids(&galaxies);
    assert_eq!(ids.len(), 10, "the task asks for ten golden IDs");
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let mut named = 0;
    let mut missing = 0;
    for (label, which, id) in ids {
        let (tag, galaxy) = &galaxies[which];
        // What a save holds is the ID's 64 bits and nothing else, so that is what is decoded here.
        let from_save = SystemId::from_raw(id.raw()).expect("a canonical ID round-trips");
        w.u64_hex(&format!("{label}.raw"), from_save.raw());
        match resolve(galaxy, from_save) {
            Ok(record) => {
                assert_eq!(record.id(), from_save);
                w.line(&format!("{label}.resolve = {tag} system"));
                write_record(&mut w, &label, &record);
                named += 1;
            }
            Err(ResolveSystemError::NoSuchSystem) => {
                w.line(&format!("{label}.resolve = no such system"));
                missing += 1;
            }
            Err(
                e @ (ResolveSystemError::LayerNotGenerated(_)
                | ResolveSystemError::KindNotGenerated),
            ) => {
                panic!("{label}: a grid ID of a stellar layer resolved to {e}")
            }
        }
    }
    assert_eq!(
        (named, missing),
        (8, 2),
        "eight IDs name a system, two do not"
    );
    golden!("placement/ids", w.as_str());
}
