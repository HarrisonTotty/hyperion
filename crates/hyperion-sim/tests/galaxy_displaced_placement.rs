//! Layers D and E as placement and the census read them, where plan 08's displaced classes join
//! them (plan 08, P08.T12).
//!
//! P08.T12 replaces layers D and E: their bounds, counts, candidates and marks all change, while
//! layers A to C do not move. It lands behind a hold (`DISPLACED_VERSION`, decision-galaxy-25-train
//! §1), so below its version every galaxy keeps the field's layers bit for bit. This file pins what
//! the hold will move, at the version in force, before the hold lands (the ruling's D1): one cell
//! of each layer at the Sun-like point, its bound, its density at the centre, its candidate count
//! and a digest of every record; the expected counts of both layers in a 50 ly sphere there; the
//! two layers' rows of the share matrix; and the face-on column density at the point. The hold's
//! own commit leaves the file unchanged, which is the proof that the held arm is the old code where
//! plan 03's goldens do not reach (the bound, the census and the map), and the bump re-blesses it.

#[expect(
    dead_code,
    reason = "the pre-pin uses the Sun-like point of tests/common alone"
)]
mod common;

use common::sunlike_point;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::map::{MapSelection, column_density_face_on};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, candidate_count, generate_cell};
use hyperion_sim::galaxy::query::{SubstellarRequest, expected_counts};
use hyperion_sim::id::Layer;
use hyperion_sim::units::LightYears;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The seed of the pinned fixture galaxy: plan 03's (`tests/placement_golden.rs`).
const FIXTURE_SEED: u64 = 0x0308_d000_0000_0000;

/// The FNV-1a offset basis and prime, 64-bit (Fowler, Noll and Vo), as plan 03's golden digests
/// take them.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Folds one record into an FNV-1a digest, every field bit for bit: the ID, the epoch position's
/// light-year and offsets, the primary's initial mass, the age and the component.
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

/// D1: layers D and E at the Sun-like point, as the version in force places and counts them.
#[test]
fn layers_d_and_e_are_pre_pinned_at_the_version_in_force() {
    let galaxy = Galaxy::from_params(Seed::new(FIXTURE_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let (fields, shares) = (galaxy.fields(), galaxy.shares());
    let sun = sunlike_point(&galaxy);
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line(&format!("# galaxy mw, {}", galaxy.seed()));
    let mut cell = Vec::new();
    for layer in [Layer::D, Layer::E] {
        let label = format!("sun.{}", layer.letter());
        let band = MassBand::from(layer);
        let key = CellKey::containing(layer, &sun).expect("a stellar layer at a point in the cube");
        let cell_box = key.cell_box();
        let [x, y, z] = key.gen_cell().to_array();
        w.line(&format!(
            "# {label}: layer {} cell ({x}, {y}, {z}), {} ly",
            layer.letter(),
            key.size_ly()
        ));
        w.f64(
            &format!("{label}.bound_per_ly3"),
            fields.layer_bound(shares, band, &cell_box),
        );
        w.f64(
            &format!("{label}.density_at_centre_per_ly3"),
            fields.layer_density(shares, band, &cell_box.centre()),
        );
        w.line(&format!(
            "{label}.candidates = {}",
            candidate_count(&galaxy, key)
        ));
        generate_cell(&galaxy, key, &mut cell);
        let mut hash = FNV_OFFSET;
        for record in &cell {
            digest_record(&mut hash, record);
        }
        w.line(&format!("{label}.systems = {}", cell.len()));
        w.u64_hex(&format!("{label}.digest"), hash);
        for (column, &share) in shares.row(band).iter().enumerate() {
            w.f64(&format!("{label}.share[{column}]"), share);
        }
    }
    let counts = expected_counts(
        &galaxy,
        &sun,
        LightYears::new(50.0),
        SubstellarRequest::None,
    );
    for layer in [Layer::D, Layer::E] {
        w.f64(
            &format!("sphere_50ly.{}", layer.letter()),
            counts.get(layer),
        );
    }
    let [x, y, _] = sun.to_light_years_f64();
    w.f64(
        "face_on.all_systems_per_ly2",
        column_density_face_on(fields, x, y, MapSelection::AllSystems),
    );
    golden!("placement/displaced_prepin", w.as_str());
}
