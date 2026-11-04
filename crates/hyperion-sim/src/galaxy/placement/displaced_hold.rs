//! The placement tests of P08.T12's hold under every 26 hold (decision-galaxy-25-train §1.5 and
//! §1.6; plan 08, P08.T12.b and the whole task's tests that T12.a–b reach): plan 03's pinned cells
//! of layers A to C unchanged and those of D and E moved (D2's placement half), order independence
//! and `resolve` agreement for displaced candidates, layer E's realised density against its model
//! in six regions, the explosion-site hook asked once per accepted layer-E candidate, and the
//! census C1: candidates per layer, and the largest count change.
//!
//! A record the thinning picks for a displaced class is, until P08.T12.c draws its marks, its
//! class's birth component's record ([`DisplacedClass::birth`]), so these tests read the pick
//! itself ([`pick_of`]) to tell displaced candidates from field ones.
//!
//! [`DisplacedClass::birth`]: crate::galaxy::displaced::DisplacedClass::birth

use std::cell::Cell;

use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

use super::candidate::{Picked, claims_with, evaluate_with_hook, pick_of};
use super::cell::CellBound;
use super::{
    CandidateOutcome, CellKey, ResolveSystemError, STELLAR_LAYERS, SystemRecord, candidate_count,
    evaluate_candidate, generate_cell, resolve,
};
use crate::coords::GalacticPosition;
use crate::galaxy::displaced::PlacementClass;
use crate::galaxy::displaced::hold::{displaced, field_only, field_only_rules};
use crate::galaxy::displaced::marks::kind_index;
use crate::galaxy::imf::{MassBand, MassFunctionKind};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::Layer;
use crate::math;
use crate::rng::Seed;
use crate::stellar::system::{draw_metallicity, primary_model};
use crate::units::Years;

/// The seed of plan 03's pinned drawn galaxy (`tests/placement_golden.rs`).
const DRAWN_SEED: u64 = 0x0308_d000_5eed_0001;

/// A point in the plane at 45° to both axes, `r` light-years from the centre.
fn diagonal(r: f64) -> GalacticPosition {
    let half = 0.5_f64.sqrt();
    GalacticPosition::from_light_years([r * half, r * half, 0.0]).expect("in the cube")
}

/// A point on the young disc's first arm ridge, just outside the bar's end.
fn arm_ridge(galaxy: &Galaxy) -> GalacticPosition {
    let arms = galaxy.fields().arms();
    let r = 1.15 * arms.bar_half_length().value();
    let (sin, cos) = math::sin_cos(arms.ridge_azimuth(r, 0));
    GalacticPosition::from_light_years([r * cos, r * sin, 0.0]).expect("in the cube")
}

/// The Sun-like point.
fn sunlike() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube")
}

/// Plan 03's pinned cells of `galaxy` (`tests/placement_golden.rs`): one of each stellar layer
/// at the Sun-like point, layers A and E in the outer bulge and layer E on an arm ridge.
fn pinned_cells(galaxy: &Galaxy) -> Vec<CellKey> {
    let at =
        |layer: Layer, p: &GalacticPosition| CellKey::containing(layer, p).expect("in the cube");
    let mut cells: Vec<CellKey> = STELLAR_LAYERS
        .iter()
        .map(|spec| at(spec.layer(), &sunlike()))
        .collect();
    cells.push(at(Layer::A, &diagonal(3_200.0)));
    cells.push(at(Layer::E, &diagonal(3_200.0)));
    cells.push(at(Layer::E, &arm_ridge(galaxy)));
    cells
}

/// Whether candidate `index` of `key` is picked for a displaced class.
fn is_displaced(galaxy: &Galaxy, key: CellKey, index: u32) -> bool {
    matches!(pick_of(galaxy, key, index), Some(Picked::Displaced(_)))
}

/// The candidate index of a grid record.
fn index_of(record: &SystemRecord) -> u32 {
    match record.id().kind() {
        crate::id::SystemIdKind::Grid(grid) => grid.index(),
        other => panic!("a grid record has a grid ID, not {other:?}"),
    }
}

/// Plan 03's goldens for layers A to C are unchanged under both arms, and those of D and E move
/// (D2's placement half): in the pinned cells of both pinned galaxies, every A-to-C cell's
/// candidate count and records are the field's bit for bit under every 26 hold, the field's under
/// every other 26 hold ([`field_only_rules`]), every D and E
/// cell's bound moves, and every layer-E cell's candidates or records. (Layer D's Sun-like cell
/// keeps its 16 systems in the fixture: its stay shares lose some 3% to the runaways and
/// walkaways, which moves no candidate of so small a cell.)
#[test]
fn plan_03_s_layers_a_to_c_are_unchanged_and_d_and_e_move() {
    let drawn_seed = Seed::new(DRAWN_SEED);
    let drawn_held = Galaxy::from_params_with_rules(
        drawn_seed,
        GalaxyParams::from_seed(drawn_seed, MassFunctionKind::default()),
        field_only_rules(),
    )
    .expect("the drawn galaxy builds");
    let drawn_displaced = Galaxy::from_params_with_rules(
        drawn_seed,
        GalaxyParams::from_seed(drawn_seed, MassFunctionKind::default()),
        GalaxyRules::every_27_hold(),
    )
    .expect("the drawn galaxy builds its displaced classes");
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for (tag, old, new) in [
        ("mw", field_only(), displaced()),
        ("drawn", &drawn_held, &drawn_displaced),
    ] {
        for key in pinned_cells(old) {
            generate_cell(old, key, &mut a);
            generate_cell(new, key, &mut b);
            let (n_old, n_new) = (candidate_count(old, key), candidate_count(new, key));
            let displaced_picks = b
                .iter()
                .filter(|r| is_displaced(new, key, index_of(r)))
                .count();
            println!(
                "{tag} layer {} {:?}: {n_old} → {n_new} candidates, {} → {} systems, {displaced_picks} \
                 picked for displaced classes",
                key.layer().letter(),
                key.gen_cell().to_array(),
                a.len(),
                b.len()
            );
            match key.layer() {
                Layer::A | Layer::B | Layer::C => {
                    assert_eq!(n_new, n_old, "{tag} {key:?}");
                    assert_eq!(b, a, "{tag} {key:?}");
                }
                Layer::D | Layer::E => {
                    let bound = |g: &Galaxy| CellBound::of(g, key).total();
                    assert!(
                        bound(new).total_cmp(&bound(old)).is_ne(),
                        "{tag} {key:?}'s bound did not move"
                    );
                    if key.layer() == Layer::E {
                        assert!(n_new != n_old || b != a, "{tag} {key:?} did not move");
                    }
                }
                Layer::BrownDwarf | Layer::RoguePlanet => unreachable!("stellar cells only"),
            }
        }
    }
}

/// The whole task's `resolve` agreement for displaced IDs: every record of layers D and E in nine
/// cells (the Sun-like point, the outer bulge and an arm ridge, layer E's 2 × 2 × 2 cells about the
/// Sun-like point among them) resolves to itself under every 26 hold, a displaced class's among
/// them, and a thinned candidate to no system.
#[test]
fn displaced_candidates_resolve_to_their_records() {
    let galaxy = displaced();
    let mut cells = vec![
        CellKey::containing(Layer::D, &sunlike()).expect("in the cube"),
        CellKey::containing(Layer::D, &diagonal(3_200.0)).expect("in the cube"),
        CellKey::containing(Layer::E, &arm_ridge(galaxy)).expect("in the cube"),
    ];
    let sun_e = CellKey::containing(Layer::E, &sunlike()).expect("in the cube");
    let [x, y, z] = sun_e.gen_cell().to_array();
    for (dx, dy, dz) in [
        (0, 0, 0),
        (1, 0, 0),
        (0, 1, 0),
        (1, 1, 0),
        (0, 0, -1),
        (1, 0, -1),
    ] {
        cells.push(CellKey::new(Layer::E, [x + dx, y + dy, z + dz]).expect("in the cube"));
    }
    let (mut systems, mut displaced_records, mut thinned) = (0_u32, 0_u32, 0_u32);
    let mut cell = Vec::new();
    for key in cells {
        generate_cell(galaxy, key, &mut cell);
        for record in &cell {
            assert_eq!(resolve(galaxy, record.id()), Ok(*record));
            systems += 1;
            if is_displaced(galaxy, key, index_of(record)) {
                displaced_records += 1;
            }
        }
        for index in 0..candidate_count(galaxy, key) {
            if pick_of(galaxy, key, index).is_none() {
                let id = key.candidate_id(index).expect("an index under the count");
                assert_eq!(resolve(galaxy, id), Err(ResolveSystemError::NoSuchSystem));
                thinned += 1;
            }
        }
    }
    println!(
        "{systems} systems resolved, {displaced_records} of displaced classes; {thinned} thinned"
    );
    assert!(displaced_records > 0);
}

/// The whole task's order independence: under every 26 hold a candidate's outcome is the same
/// alone, forwards, backwards and permuted, in layers D and E at the Sun-like point and in the
/// outer bulge, displaced candidates among them.
#[test]
fn candidates_are_order_independent_under_every_26_hold() {
    let galaxy = displaced();
    let mut displaced_picks = 0_u32;
    for layer in [Layer::D, Layer::E] {
        for at in [sunlike(), diagonal(3_200.0)] {
            let key = CellKey::containing(layer, &at).expect("in the cube");
            let indices: Vec<u32> = (0..candidate_count(galaxy, key).min(200)).collect();
            assert_order_independent(&indices, |&index| evaluate_candidate(galaxy, key, index));
            displaced_picks += u32::try_from(
                indices
                    .iter()
                    .filter(|&&index| is_displaced(galaxy, key, index))
                    .count(),
            )
            .expect("at most 200");
        }
    }
    assert!(displaced_picks > 0, "the sample holds displaced candidates");
}

/// The expected number of layer-E systems in `cell`, by the midpoint rule in 6³ boxes of the
/// layer's density: field and displaced classes together.
fn expected_in(galaxy: &Galaxy, key: CellKey) -> f64 {
    let (fields, shares) = (galaxy.fields(), galaxy.shares());
    let min = key.origin_ly().map(f64::from);
    let edge = f64::from(key.size_ly());
    let steps = 6_u32;
    let h = edge / f64::from(steps);
    let mut total = 0.0;
    for i in 0..steps {
        for j in 0..steps {
            for k in 0..steps {
                let p = PointLy::new(
                    min[0] + (f64::from(i) + 0.5) * h,
                    min[1] + (f64::from(j) + 0.5) * h,
                    min[2] + (f64::from(k) + 0.5) * h,
                );
                total += fields.layer_density(shares, MassBand::E, &p) * h * h * h;
            }
        }
    }
    total
}

/// The whole task's density test: layer E's records under every 26 hold against the layer's
/// density, field and displaced classes, by Poisson interval at α = 10⁻³, in six regions: the
/// plane at 1, 3 and 6 thin-disc scale lengths, 3,000 ly and 15,000 ly above the plane at the
/// Sun-like radius, and the outer bulge. Each region is a block of cells, wider where the layer is
/// thin.
#[test]
fn layer_e_records_match_its_density_in_six_regions() {
    let galaxy = displaced();
    let r_d = galaxy.params().thin_disc().length().value();
    let regions: [(&str, [f64; 3], [i32; 3]); 6] = [
        ("plane at 1 R_d", [0.0, r_d, 0.0], [4, 4, 2]),
        ("plane at 3 R_d", [0.0, 3.0 * r_d, 0.0], [4, 4, 2]),
        ("plane at 6 R_d", [0.0, 6.0 * r_d, 0.0], [8, 8, 2]),
        ("3,000 ly up", [0.0, 26_000.0, 3_000.0], [8, 8, 2]),
        ("15,000 ly up", [0.0, 26_000.0, 15_000.0], [16, 16, 4]),
        ("outer bulge", [2_262.7, 2_262.7, 0.0], [2, 2, 2]),
    ];
    let mut cell = Vec::new();
    for (name, at, [nx, ny, nz]) in regions {
        let corner = CellKey::containing(
            Layer::E,
            &GalacticPosition::from_light_years(at).expect("in the cube"),
        )
        .expect("in the cube");
        let [x, y, z] = corner.gen_cell().to_array();
        let (mut observed, mut expected, mut moved) = (0_u64, 0.0, 0_u64);
        for i in 0..nx {
            for j in 0..ny {
                for k in 0..nz {
                    let key = CellKey::new(Layer::E, [x + i, y + j, z + k - nz / 2])
                        .expect("in the cube");
                    generate_cell(galaxy, key, &mut cell);
                    observed += u64::try_from(cell.len()).expect("a cell's count");
                    moved += u64::try_from(
                        cell.iter()
                            .filter(|r| is_displaced(galaxy, key, index_of(r)))
                            .count(),
                    )
                    .expect("a cell's count");
                    expected += expected_in(galaxy, key);
                }
            }
        }
        println!("{name}: {observed} layer-E systems ({moved} displaced) against {expected:.1}");
        assert_poisson_count(name, observed, expected, ALPHA);
    }
}

/// P08.T13's inheritance 4: under every 26 hold the restructured candidate evaluation still asks
/// the explosion-site hook once per accepted layer-E candidate, after the marks, and never for a
/// thinned one, in eight layer-E cells at the Sun-like point.
#[test]
fn the_hook_is_asked_once_per_accepted_layer_e_candidate_under_every_26_hold() {
    let galaxy = displaced();
    let asked = Cell::new(0_u32);
    let hook = |g: &Galaxy, record: &SystemRecord, _: Option<Years>| {
        claims_with(
            g,
            record,
            |_, _| {
                asked.set(asked.get() + 1);
                None
            },
            |_, _, _| false,
        )
    };
    let sun = CellKey::containing(Layer::E, &sunlike()).expect("in the cube");
    let [x, y, z] = sun.gen_cell().to_array();
    let (mut accepted, mut displaced_records) = (0_u32, 0_u32);
    for dx in 0..8 {
        let key = CellKey::new(Layer::E, [x + dx, y, z]).expect("in the cube");
        let bound = CellBound::of(galaxy, key);
        for index in 0..candidate_count(galaxy, key) {
            let before = asked.get();
            let outcome = evaluate_with_hook(galaxy, key, index, &bound, hook);
            let asks = asked.get() - before;
            match outcome {
                CandidateOutcome::Accepted(_) => {
                    assert_eq!(asks, 1, "{key:?} candidate {index}");
                    accepted += 1;
                    if is_displaced(galaxy, key, index) {
                        displaced_records += 1;
                    }
                }
                CandidateOutcome::Thinned
                | CandidateOutcome::ClaimedByCatalogue
                | CandidateOutcome::InLossCone => assert_eq!(asks, 0, "{key:?} candidate {index}"),
            }
        }
    }
    println!("{accepted} accepted, {displaced_records} of displaced classes, each asked once");
    assert!(displaced_records > 0);
}

/// C1, the census at the hold's landing: over every stellar layer's cell at six places (the
/// Sun-like point, the outer bulge, an arm ridge, 3,000 ly above the Sun-like point, the halo
/// 15,000 ly up and 400 ly from the centre), candidates and systems with the field alone, under
/// every other 26 hold, against under every 26 hold, the movers by count, the largest count
/// change, and the systems of displaced
/// classes. Layers A to C never move; every moved record is finite.
#[test]
fn the_census_moves_only_layers_d_and_e() {
    let (old, new) = (field_only(), displaced());
    let places = [
        ("sun", sunlike()),
        ("bulge", diagonal(3_200.0)),
        ("arm", arm_ridge(old)),
        (
            "3,000 ly up",
            GalacticPosition::from_light_years([0.0, 26_000.0, 3_000.0]).expect("in the cube"),
        ),
        (
            "halo",
            GalacticPosition::from_light_years([0.0, 20_000.0, 15_000.0]).expect("in the cube"),
        ),
        (
            "centre",
            GalacticPosition::from_light_years([400.0, 300.0, 0.0]).expect("in the cube"),
        ),
    ];
    let (mut a, mut b) = (Vec::new(), Vec::new());
    let mut largest: (i64, &str, Layer) = (0, "", Layer::A);
    let mut movers = 0_u32;
    for spec in STELLAR_LAYERS {
        let layer = spec.layer();
        let (mut candidates, mut systems, mut moved_systems) = ([0_u64; 2], [0_u64; 2], 0_u64);
        for (name, at) in &places {
            let key = CellKey::containing(layer, at).expect("in the cube");
            let counts = [candidate_count(old, key), candidate_count(new, key)];
            generate_cell(old, key, &mut a);
            generate_cell(new, key, &mut b);
            candidates[0] += u64::from(counts[0]);
            candidates[1] += u64::from(counts[1]);
            systems[0] += u64::try_from(a.len()).expect("a count");
            systems[1] += u64::try_from(b.len()).expect("a count");
            let change = i64::from(counts[1]) - i64::from(counts[0]);
            if change.abs() > largest.0.abs() {
                largest = (change, name, layer);
            }
            if counts[0] != counts[1] || a != b {
                movers += 1;
            }
            for record in &b {
                assert!(record.primary_initial_mass().value().is_finite());
                assert!(record.age_at_epoch().value().is_finite());
                assert!(
                    record
                        .epoch_position()
                        .offset_metres()
                        .iter()
                        .all(|m| m.is_finite())
                );
                if is_displaced(new, key, index_of(record)) {
                    moved_systems += 1;
                }
            }
            if matches!(layer, Layer::A | Layer::B | Layer::C) {
                assert_eq!(
                    (counts[1], &b),
                    (counts[0], &a),
                    "layer {layer:?} at {name}"
                );
            }
        }
        println!(
            "layer {}: candidates {} → {}, systems {} → {}, {moved_systems} of displaced classes",
            layer.letter(),
            candidates[0],
            candidates[1],
            systems[0],
            systems[1]
        );
    }
    println!(
        "{movers} of {} cells moved; the largest count change {} in layer {} at {}",
        STELLAR_LAYERS.len() * places.len(),
        largest.0,
        largest.2.letter(),
        largest.1
    );
    assert!(movers > 0);
}

/// A placement class as a number, for a digest: 0 alive, `1 + s` retained in speed bin `s`, and
/// `16 + 4 × class + kind` displaced.
fn placement_code(class: PlacementClass) -> f64 {
    match class {
        PlacementClass::Alive => 0.0,
        PlacementClass::Retained { speed } => {
            f64::from(u8::try_from(speed.index()).expect("under eight")) + 1.0
        }
        PlacementClass::Displaced { class, kind } => {
            let class = u16::try_from(class.index()).expect("under 128");
            let kind = u8::try_from(kind_index(kind)).expect("under four");
            16.0 + 4.0 * f64::from(class) + f64::from(kind)
        }
    }
}

/// The digest (`f64_digest`) of the new arm's placement in plan 03's pinned cells of the fixture,
/// with the census's counts and the face-on map at the Sun-like point: each cell's bound and its
/// classes' rests, its candidate count, and each record's position, mass, age, component, whether
/// it was picked for a class, and its conditional marks (P08.T12.c: placement class, mark
/// attempt, metallicity age and metallicity), with the kick loop's kept attempt, natal kick and
/// remnant mass for the first 40 constrained records of each cell. Blessed natively on x86-64; the wasip1 suite compares it
/// (D5). A 26-arm figure on plan 15's provisional form table, re-pinned by name when the
/// production table lands; re-pinned on the rebase onto P09.T2.c's field share and P09.T35's
/// carve-out, which every 26 hold composes with the classes; re-pinned at generator versions 25
/// and 26, and at 27 (it was `0x9f41_f812_1008_a969`), whose galaxy the fixture is, every 27
/// hold's, and every 26 hold's with its retargets until the bump; and again with P15.T10.b's refit
/// of the binary rows on 27's engine, whose carved shares the field's thinning reads (it was
/// `0x8bb9_7c3f_e9db_adfc` before it).
const DISPLACED_CELLS_DIGEST: u64 = 0x10e9_90be_dd90_9b4b;

/// The new arm's cells, census and map, pinned on every target (named `pinned` for the
/// WebAssembly suite's filter).
#[test]
fn the_new_arm_s_cells_are_pinned() {
    let galaxy = displaced();
    let mut values = Vec::new();
    let mut cell = Vec::new();
    for key in pinned_cells(galaxy) {
        let bound = CellBound::of(galaxy, key);
        values.push(bound.total());
        let classes = galaxy
            .fields()
            .displaced()
            .map_or(0, |d| d.order(key.band()).len());
        values.extend((0..classes).map(|i| bound.rest(i)));
        values.push(f64::from(candidate_count(galaxy, key)));
        generate_cell(galaxy, key, &mut cell);
        let mut constrained = 0;
        for record in &cell {
            values.extend(record.epoch_position().cell().to_array().map(f64::from));
            values.extend(record.epoch_position().offset_metres());
            values.push(record.primary_initial_mass().value());
            values.push(record.age_at_epoch().value());
            let component = record.component().expect("a grid record");
            values.push(f64::from(
                u8::try_from(component.index()).expect("under 24"),
            ));
            values.push(if is_displaced(galaxy, key, index_of(record)) {
                1.0
            } else {
                0.0
            });
            values.push(placement_code(record.placement_class()));
            values.push(f64::from(record.mark_attempt()));
            values.push(record.metallicity_age().map_or(-1.0, Years::value));
            values.push(draw_metallicity(galaxy, record).fe_h().value());
            // The kick loop's kept attempt, natal kick and remnant mass, for the first constrained
            // records of each cell (a track each).
            if constrained < 40 && record.kick_range(galaxy).is_some() {
                constrained += 1;
                let (model, attempts) =
                    primary_model(galaxy, record, draw_metallicity(galaxy, record));
                values.push(f64::from(attempts));
                if let Some(kick) = model.natal_kick() {
                    values.push(kick.speed().value());
                    values.extend(kick.direction().components());
                }
                values.extend(model.remnant().map(|r| r.mass().value()));
            }
        }
    }
    let counts = crate::galaxy::query::expected_counts(
        galaxy,
        &sunlike(),
        crate::units::LightYears::new(50.0),
        crate::galaxy::query::SubstellarRequest::None,
    );
    values.extend(counts.to_array());
    for selection in [
        crate::galaxy::map::MapSelection::AllSystems,
        crate::galaxy::map::MapSelection::LayerERemnants,
    ] {
        values.push(crate::galaxy::map::column_density_face_on(
            galaxy.fields(),
            0.0,
            26_000.0,
            selection,
        ));
    }
    let digest = hyperion_testkit::golden::f64_digest(&values);
    println!("digest {digest:#018x} over {} values", values.len());
    assert_eq!(digest, DISPLACED_CELLS_DIGEST, "re-pin by name");
}
