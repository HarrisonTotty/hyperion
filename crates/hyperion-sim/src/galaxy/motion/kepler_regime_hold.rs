//! The tests of P09.T28.b.b's hold, [`KEPLER_REGIME_VERSION`] (decision-galaxy-25-train §1.5 and
//! §1.6): its tripwire; and, under every 25 hold, the Kepler regime moving the grid's records
//! inside the influence radius and no others (D2), the loss cone's carve-out, the same seed twice
//! (D4), a range query near the black hole against a scan under either arm, and the census C1.
//!
//! The bit-identity of the version in force is held by the goldens, P09.T28.b.a's pre-pin
//! (`regime_records_near_the_centre_are_pinned`, D1) among them, and by each test here comparing
//! the held arm with the straight lines. The bump to 25 deletes the tripwire and the held arm, and
//! these become tests of the version's own galaxy.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden::f64_digest;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use crate::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell, ClassId, system_of};
use crate::galaxy::imf::MassFunctionKind;
use crate::galaxy::placement::{
    CandidateOutcome, CellCache, CellKey, NoCache, ResolveSystemError, evaluate_candidate,
    generate_cell, resolve,
};
use crate::galaxy::query::{
    LayerCounts, LayerSet, QuerySphere, RangeQuery, SubstellarRequest, SystemHit, cells_in_sphere,
    count_cells_in_sphere, decide_census, expected_counts, kepler_pad_for, pad_for, pad_speed,
    position_at, range_query, sphere_for_layer, walk_pad,
};
use crate::galaxy::rules::GalaxyRules;
use crate::id::{Layer, SystemId, SystemIdKind};
use crate::rng::Seed;
use crate::time::{ClockWindow, Span};
use crate::units::MetresPerSecond;

/// The seed of P09.T28.b.a's pre-pin's Milky Way fixture.
const MILKY_WAY_SEED: u64 = 0x0928_b0a0_0000_0001;

/// The times the tests move records to, Julian years from the epoch: ±100 and ±1,000, which is
/// ±H.
const TIMES: [i64; 4] = [-1_000, -100, 100, 1_000];

/// The pre-pin's Milky Way fixture with its kinematic tables, so that its grid systems move, under
/// the generator version's rules: the held arm.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(MILKY_WAY_SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
            .with_full_potential()
    })
}

/// [`held`]'s galaxy under every 25 hold: the Kepler arm.
fn kepler() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| held().clone().with_rules(GalaxyRules::every_25_hold()))
}

pub(super) fn years(y: i64) -> UniverseTime {
    UniverseTime::from_julian_years(y).expect("in the clock window")
}

pub(super) fn ly(x: f64, y: f64, z: f64) -> GalacticPosition {
    GalacticPosition::from_light_years([x, y, z]).expect("in range")
}

/// The distance between two positions, ly.
pub(super) fn apart(a: &GalacticPosition, b: &GalacticPosition) -> f64 {
    a.distance_to(b).value() / METRES_PER_LIGHT_YEAR
}

/// Every grid record of `layer` whose epoch position lies within `radius` ly of the black hole,
/// in walk order, as `galaxy` places them.
pub(super) fn records_within(galaxy: &Galaxy, layer: Layer, radius: f64) -> Vec<SystemRecord> {
    records_within_from(&mut NoCache::new(), galaxy, layer, radius)
}

/// [`records_within`] through `cells`.
fn records_within_from(
    cells: &mut impl CellCache,
    galaxy: &Galaxy,
    layer: Layer,
    radius: f64,
) -> Vec<SystemRecord> {
    let sphere = QuerySphere::new(
        GalacticPosition::ORIGIN,
        LightYears::new(radius),
        UniverseTime::EPOCH,
        LightYears::ZERO,
    )
    .expect("a sphere at the centre");
    let mut near = Vec::new();
    for key in cells_in_sphere(layer, &sphere) {
        cells.with_cell(galaxy, key, |cell| {
            near.extend(
                cell.iter()
                    .copied()
                    .filter(|r| apart(&GalacticPosition::ORIGIN, r.epoch_position()) <= radius),
            );
        });
    }
    near
}

/// The record's candidate index in its cell.
fn index_of(record: &SystemRecord) -> u32 {
    let SystemIdKind::Grid(grid) = record.id().kind() else {
        panic!("{record:?} is a grid record")
    };
    grid.index()
}

/// A cell cache that keeps every cell it is asked for, so that a query and a scan generate each
/// once.
#[derive(Default)]
struct KeepCells(BTreeMap<CellKey, Vec<SystemRecord>>);

impl CellCache for KeepCells {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        f(self.0.entry(key).or_insert_with(|| {
            let mut systems = Vec::new();
            generate_cell(galaxy, key, &mut systems);
            systems
        }))
    }
}

/// P09.T28.b.b is held until the 24 → 25 bump: the rules in force are version 24's and before,
/// under which every grid system drifts in a straight line, the loss cone carves out no grid
/// system and the walks pad for the straight lines alone.
#[test]
fn the_kepler_regime_is_held_only_until_the_25_bump() {
    assert!(
        GENERATOR_VERSION < KEPLER_REGIME_VERSION,
        "the 24 → 25 bump puts P09.T28.b.b on the record (decision-galaxy-25-train §1 and §2): \
         Kepler-regime grid systems on their orbits (`query::position_at`), the loss cone's \
         carve-out (`CandidateOutcome::InLossCone`) and the walks' Kepler pad (`query::walk_pad`). \
         Delete `KEPLER_REGIME_VERSION`, `KeplerRegimeRules` with its `Straight` arm, \
         `GalaxyRules::kepler_regime`, the three `match`es on it and this test, in the bump's own \
         checkpoint commit, blessed at 24 (§1.6, C2)"
    );
    let galaxy = Galaxy::from_params(Seed::new(MILKY_WAY_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture builds");
    assert_eq!(galaxy.rules().kepler_regime, KeplerRegimeRules::Straight);
    assert_eq!(
        GalaxyRules::of_version(GENERATOR_VERSION).kepler_regime,
        KeplerRegimeRules::Straight
    );
    assert_eq!(
        GalaxyRules::every_25_hold().kepler_regime,
        KeplerRegimeRules::Kepler
    );
}

/// The digest (`f64_digest`) of the Kepler arm's positions in the D2 test: each Kepler-regime
/// record's cell and offsets at −1,000, −100, +100 and +1,000 yr, in walk order. Blessed natively
/// on x86-64; the wasip1 suite compares it. Re-pinned by name when P09.T28.d.b's exact orbit
/// joined every 25 hold (it was `0xdc2a_4657_d88c_161e` on the two-body law).
const MOVED_POSITIONS_DIGEST: u64 = 0xce83_b24f_e65c_7554;

/// D2 under every 25 hold: the Kepler arm moves the fixture's grid records inside the influence
/// radius onto their orbits, bit for bit [`position_velocity_at`]'s, and leaves every record
/// outside it, and every record at the epoch (the plan's "positions at t = 0 unchanged"), where
/// the held arm puts it, which is the straight line; its cells are the held arm's less the records
/// whose orbits pass inside the loss cone, each [`CandidateOutcome::InLossCone`] and "no such
/// system"; and the walks pad by the straight lines under the held arm and for the orbits under the
/// new one. Layer B within 1.5 influence radii, whose eight central cells are the stellar layers'
/// cheapest to place, at ±100 and ±1,000 yr. Named `pinned` for the WebAssembly suite's filter
/// (D5): the arm propagates on the record's path, so the counts and a digest of the positions it
/// gives are pinned, as blessed natively. Under every 25 hold the orbits are P09.T28.d.b's exact
/// ones, which re-pinned the digest by name (decision-galaxy-25-train §1.3).
#[test]
fn the_kepler_arm_moves_records_inside_the_radius_and_no_others_pinned() {
    let (held, kepler) = (held(), kepler());
    let edge = held.centre_motion().influence_radius().value();
    let old = records_within(held, Layer::B, 1.5 * edge);
    let new = records_within(kepler, Layer::B, 1.5 * edge);
    let (mut inside, mut outside, mut moved, mut farthest) = (0_u32, 0_u32, 0_u32, 0.0_f64);
    let mut positions = Vec::new();
    let mut kept = new.iter().peekable();
    for record in &old {
        let epoch = *record.epoch_position();
        assert_eq!(position_at(held, record, UniverseTime::EPOCH), epoch);
        assert_eq!(position_at(kepler, record, UniverseTime::EPOCH), epoch);
        let start = EpochState::of_grid_record(held, record);
        let carved = loss_cone_verdict(held, &start) == LossConeVerdict::Inside;
        assert_eq!(carved_by_the_loss_cone(kepler, record), carved);
        if kept.peek() == Some(&record) {
            kept.next();
            assert!(!carved, "{:?} in the loss cone was placed", record.id());
        } else {
            assert!(
                carved,
                "{:?} was dropped outside the loss cone",
                record.id()
            );
            let key = CellKey::of(record.id()).expect("a grid ID");
            assert_eq!(
                evaluate_candidate(kepler, key, index_of(record)),
                CandidateOutcome::InLossCone
            );
            assert_eq!(resolve(held, record.id()), Ok(*record));
            assert_eq!(
                resolve(kepler, record.id()),
                Err(ResolveSystemError::NoSuchSystem)
            );
            continue;
        }
        let regime = regime_of(held, record);
        match regime {
            Regime::Straight => outside += 1,
            Regime::Kepler => inside += 1,
        }
        for y in TIMES {
            let t = years(y);
            let straight = drifted(record, start.velocity(), t);
            let (a, b) = (position_at(held, record, t), position_at(kepler, record, t));
            assert_eq!(
                a,
                straight,
                "{:?}: the held arm is the straight line",
                record.id()
            );
            match (regime, position_velocity_at(kepler, &start, t)) {
                (Regime::Straight, _) => {
                    assert_eq!(b, a, "{:?} outside the regime moved", record.id());
                }
                (Regime::Kepler, StateAt::Exists(state)) => {
                    assert_eq!(&b, state.position(), "{:?} at {y} yr", record.id());
                    assert!(b.offset_metres().iter().all(|m| m.is_finite()));
                    if b != a {
                        moved += 1;
                        farthest = farthest.max(apart(&a, &b));
                    }
                    positions.extend(b.cell().to_array().map(f64::from));
                    positions.extend(b.offset_metres());
                }
                (Regime::Kepler, StateAt::NoSuchSystem) => {
                    unreachable!("a record in the loss cone is carved out above")
                }
            }
        }
    }
    assert_eq!(
        kept.next(),
        None,
        "the new arm placed a record the held arm does not"
    );
    println!(
        "{inside} records inside the radius, {outside} outside, {} carved out; {moved} \
         record-times moved off their straight lines, the farthest by {farthest:.4} ly",
        old.len() - new.len()
    );
    assert_eq!((inside, outside, moved), (3_286, 9_385, 13_144));
    assert_eq!(
        old.len(),
        new.len(),
        "the fixture's layer B has no carve-out"
    );
    assert_eq!(
        f64_digest(&positions),
        MOVED_POSITIONS_DIGEST,
        "{farthest} ly"
    );
    the_walks_pad_by_the_arm(held, kepler);
}

/// The walks' pad: the straight lines' under the held arm, to the bit, and the Kepler pad under
/// the new one, near the black hole and far from it.
fn the_walks_pad_by_the_arm(held: &Galaxy, kepler: &Galaxy) {
    for (centre, radius) in [(ly(0.5, 0.3, 0.2), 1.0), (ly(5_000.0, 0.0, 0.0), 2.0)] {
        for layer in Layer::ALL {
            for y in TIMES {
                let (r, t) = (LightYears::new(radius), years(y));
                let straight = pad_for(t, pad_speed(layer)).value();
                let curved = kepler_pad_for(held, layer, &centre, r, t).value();
                assert_same_bits(walk_pad(held, layer, &centre, r, t).value(), straight);
                assert_same_bits(walk_pad(kepler, layer, &centre, r, t).value(), curved);
            }
        }
    }
}

/// A catalogue system moves by its class under either arm (P09.T33.a over this hold): plan 03's
/// `position_at` gives a core collapse its class's line broken at its explosion, bit for bit the
/// same under both rules, and so does `kepler_regime_position_at` asked about it directly, rather
/// than an orbit or a drift at its epoch velocity, which is its remnant's once it has exploded. Its
/// class's cell and the system its ID names are the same under both rules, since the class reads
/// none. Its fastest speed (`DeathMarks`'s `v_fast`) is under its layer's pad speed, so either
/// arm's walk pad would hold it, though no walk reaches a catalogue system yet: the class's lookups
/// pad by its own reach, and P09.T37's source will. The class cell at the black hole, where the
/// nuclear disc's young deaths lie, at the epoch, ±100 and ±1,000 yr and either side of any
/// explosion inside the clock window; and, where the line and that drift part, a millennium on the
/// far side of each explosion from the epoch. Its counts are pinned as measured.
#[test]
fn a_catalogue_system_moves_by_its_class_under_either_arm() {
    let (held, kepler) = (held(), kepler());
    let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [0, 0, 0]).expect("the centre's cell");
    let entries =
        CatalogueClassCell::generate(held, &CoreCollapseProcess::new(held), key).into_entries();
    assert!(!entries.is_empty(), "the nuclear disc has recent deaths");
    assert_eq!(
        CatalogueClassCell::generate(kepler, &CoreCollapseProcess::new(kepler), key).into_entries(),
        entries,
        "the class's cell reads no rule"
    );
    let mut inside = 0_u32;
    for entry in &entries {
        let record = entry.value().record();
        let system = system_of(held, entry.id()).expect("an entry names its system");
        assert_eq!(system_of(kepler, entry.id()), Ok(system));
        if regime_of(held, record) == Regime::Kepler {
            inside += 1;
        }
        let layer = record.layer();
        assert_eq!(layer, Layer::E, "a core collapse's primary is of band E");
        let fastest = entry.value().marks().fastest_member_speed().value();
        assert!(
            fastest < MetresPerSecond::from(pad_speed(layer)).value(),
            "{:?} moves at {fastest} m/s",
            entry.id()
        );
        let death = entry.value().site().death();
        let one_second = Span::from_seconds(1);
        let times = TIMES
            .map(years)
            .into_iter()
            .chain([UniverseTime::EPOCH])
            .chain(
                [Some(death), death.checked_sub(one_second)]
                    .into_iter()
                    .flatten(),
            )
            .filter(|&t| ClockWindow::contains(t));
        for t in times {
            let class = system.position_at(t);
            assert_eq!(
                position_at(held, record, t),
                class,
                "{:?} at {t}",
                entry.id()
            );
            assert_eq!(
                position_at(kepler, record, t),
                class,
                "{:?} at {t}",
                entry.id()
            );
            assert_eq!(
                kepler_regime_position_at(kepler, record, t),
                class,
                "{:?} at {t}",
                entry.id()
            );
            for galaxy in [held, kepler] {
                let pad = walk_pad(galaxy, layer, &class, LightYears::ZERO, t).value();
                let moved = apart(record.epoch_position(), &class);
                assert!(moved <= pad, "{:?} moved {moved} ly by {t}", entry.id());
            }
        }
        // On the leg the epoch is not on, a drift at the epoch velocity misses the line by the
        // kick's travel, so this is where the arm's answer is told from it.
        let millennium = Span::from_julian_years(1_000).expect("a millennium");
        // From its explosion on, its line's velocity is the remnant's.
        let far_leg = if death <= UniverseTime::EPOCH {
            death.checked_sub(millennium)
        } else {
            death.checked_add(millennium)
        }
        .expect("inside the clock's range");
        let class = system.position_at(far_leg);
        assert_eq!(
            kepler_regime_position_at(kepler, record, far_leg),
            class,
            "{:?} at {far_leg}",
            entry.id()
        );
        assert_ne!(
            drifted(record, epoch_velocity(kepler, record), far_leg),
            class,
            "{:?}: its kick moves it",
            entry.id()
        );
    }
    println!(
        "{} core collapses in the class cell at the black hole, {inside} inside the influence radius",
        entries.len()
    );
    // None lies inside the fixture's 7.19 ly edge, so the arms are told apart by the origin alone.
    assert_eq!((entries.len(), inside), (20, 0));
}

/// The seed whose central grid holds the one record of the census C1 that the loss cone carves
/// out: a rogue planet 2.54 ly from the black hole on a plunging orbit.
const PLUNGING_SEED: u64 = 0x5eed_0000_c0ff_ee00;

/// That rogue planet's ID.
const PLUNGING_ROGUE_PLANET: u64 = 0xcfff_e000_3fff_0b41;

/// D4 under every 25 hold, and the loss cone's carve-out on a record that has one: a galaxy built
/// under the rules is the same galaxy, with the same cells and the same positions to the bit, as
/// one built under the version's and given the rules after (`Galaxy::with_rules`, which the other
/// tests use), and the census C1's carved-out rogue planet is placed and resolved under the held
/// arm, and under the new one is [`CandidateOutcome::InLossCone`], "no such system" and missing
/// from its cell, which loses nothing else; and the new arm's cells, resolutions, positions and pads
/// are the same in any order, with the centre's motion built first or last.
#[test]
fn the_kepler_arm_gives_the_same_seed_the_same_answers_twice_and_in_any_order() {
    let seed = Seed::new(PLUNGING_SEED);
    let params = || GalaxyParams::from_seed(seed, MassFunctionKind::default());
    let held = Galaxy::from_params(seed, params())
        .expect("a drawn galaxy keeps half its gas neutral")
        .with_full_potential();
    let again = Galaxy::from_params_with_rules(seed, params(), GalaxyRules::every_25_hold())
        .expect("a drawn galaxy keeps half its gas neutral")
        .with_full_potential();
    // Before anything reads its memos.
    let cold = again.clone();
    assert_eq!(again.rules(), GalaxyRules::every_25_hold());
    let edge = held.centre_motion().influence_radius();
    let kepler = held.clone().with_rules(GalaxyRules::every_25_hold());
    assert_eq!(again, kepler);
    assert_ne!(held, kepler);

    let id = SystemId::from_raw(PLUNGING_ROGUE_PLANET).expect("a well-formed ID");
    let key = CellKey::of(id).expect("a grid ID");
    let plunging = resolve(&held, id).expect("the held arm places it");
    let near = CellKey::containing(Layer::B, &ly(0.5, 0.5, 0.5)).expect("in the cube");
    let mut cell = Vec::new();
    generate_cell(&held, near, &mut cell);
    cell.retain(|r| regime_of(&held, r) == Regime::Kepler);
    let (first, last) = (cell[0], cell[cell.len() - 1]);
    let asks = [
        Ask::Cell(key),
        Ask::Cell(near),
        Ask::Resolve(id),
        Ask::Resolve(first.id()),
        Ask::At(plunging, years(100)),
        Ask::At(first, years(-100)),
        Ask::At(last, years(1_000)),
        Ask::Pad(Layer::A, ly(1.0, 0.0, 0.0), years(-1_000)),
    ];
    // The carve-out builds the centre's motion from inside cell generation, so whichever of these
    // runs first builds the memo: once on one galaxy, warm after its first answer, and once on a
    // fresh copy each time, cold (the sim-determinism skill's order independence).
    assert_order_independent(&asks, |ask| ask.answer(&again));
    assert_order_independent(&asks, |ask| ask.answer(&cold.clone()));
    // `with_rules` kept the memo `held` had read, which is the one a fresh build makes.
    assert_same_bits(
        again.centre_motion().influence_radius().value(),
        kepler.centre_motion().influence_radius().value(),
    );
    assert_same_bits(
        kepler.centre_motion().influence_radius().value(),
        edge.value(),
    );
    assert_eq!(plunging.layer(), Layer::RoguePlanet);
    assert_eq!(regime_of(&held, &plunging), Regime::Kepler);
    let start = EpochState::of_grid_record(&held, &plunging);
    assert_eq!(loss_cone_verdict(&held, &start), LossConeVerdict::Inside);
    for galaxy in [&kepler, &again] {
        assert_eq!(
            evaluate_candidate(galaxy, key, index_of(&plunging)),
            CandidateOutcome::InLossCone
        );
        assert_eq!(resolve(galaxy, id), Err(ResolveSystemError::NoSuchSystem));
        // Made by hand, it is given its straight line, the held arm's answer.
        let t = years(100);
        assert_eq!(
            position_at(galaxy, &plunging, t),
            position_at(&held, &plunging, t)
        );
    }

    let (mut old, mut new, mut twice) = (Vec::new(), Vec::new(), Vec::new());
    generate_cell(&held, key, &mut old);
    generate_cell(&kepler, key, &mut new);
    generate_cell(&again, key, &mut twice);
    assert_eq!(new, twice);
    let kept: Vec<SystemRecord> = old.iter().copied().filter(|r| r.id() != id).collect();
    assert_eq!(old.len(), kept.len() + 1);
    assert_eq!(new, kept);
    for record in &new {
        for y in TIMES {
            assert_eq!(
                position_at(&again, record, years(y)),
                position_at(&kepler, record, years(y)),
                "{:?} at {y} yr",
                record.id()
            );
        }
    }
}

/// A question D4 asks a galaxy, in any order.
#[derive(Debug)]
enum Ask {
    Cell(CellKey),
    Resolve(SystemId),
    At(SystemRecord, UniverseTime),
    Pad(Layer, GalacticPosition, UniverseTime),
}

/// What a galaxy answers an [`Ask`].
#[derive(Debug, PartialEq)]
enum Answer {
    Cell(Vec<SystemRecord>),
    Resolved(Result<SystemRecord, ResolveSystemError>),
    At(GalacticPosition),
    Pad(LightYears),
}

impl Ask {
    fn answer(&self, galaxy: &Galaxy) -> Answer {
        match self {
            Self::Cell(key) => {
                let mut cell = Vec::new();
                generate_cell(galaxy, *key, &mut cell);
                Answer::Cell(cell)
            }
            Self::Resolve(id) => Answer::Resolved(resolve(galaxy, *id)),
            Self::At(record, t) => Answer::At(position_at(galaxy, record, *t)),
            Self::Pad(layer, centre, t) => {
                Answer::Pad(walk_pad(galaxy, *layer, centre, LightYears::new(0.5), *t))
            }
        }
    }
}

/// The census C1's galaxies: P09.T28.b.a's pre-pin's three, with their kinematic tables.
pub(super) fn census_galaxies() -> [(&'static str, Galaxy); 3] {
    let drawn = |seed: u64| {
        Galaxy::from_params(
            Seed::new(seed),
            GalaxyParams::from_seed(Seed::new(seed), MassFunctionKind::default()),
        )
        .expect("a drawn galaxy keeps half its gas neutral")
    };
    [
        (
            "milky_way_like",
            Galaxy::from_params(Seed::new(MILKY_WAY_SEED), GalaxyParams::milky_way_like())
                .expect("the Milky Way fixture builds"),
        ),
        ("seed 0x5eed0000c0ffee00", drawn(PLUNGING_SEED)),
        ("seed 0x2", drawn(2)),
    ]
    .map(|(label, galaxy)| (label, galaxy.with_full_potential()))
}

/// The pre-pin's radius for `layer`: 1.5 influence radii, and for the substellar layers at most
/// 4 ly (P09.T28.b.a as built).
pub(super) fn census_radius(galaxy: &Galaxy, layer: Layer) -> f64 {
    let edge = 1.5 * galaxy.centre_motion().influence_radius().value();
    match layer {
        Layer::A | Layer::B | Layer::C | Layer::D | Layer::E => edge,
        Layer::BrownDwarf | Layer::RoguePlanet => edge.min(4.0),
    }
}

/// What the Kepler arm does to one layer's records at one time.
#[derive(Default)]
struct Moves {
    movers: u64,
    largest: f64,
    /// Kepler-regime records farther from their epoch positions than the straight lines' pad,
    /// which only the Kepler pad holds.
    beyond_straight: u64,
}

/// The census C1 of decision-galaxy-25-train §1.6, under every 25 hold against the held arm: over
/// P09.T28.b.a's pre-pin (three galaxies, every grid record within 1.5 influence radii, the
/// substellar layers within 4 ly), at ±100 and ±1,000 yr (±H), the movers by count, the largest
/// move, every mover finite, no record outside the regime moved, every record held by the walk's
/// pad from where it ends (with those beyond the straight lines' pad counted), and the carve-outs
/// by ID, each "no such system" under the new arm and the loss cone's verdict; with the
/// Kepler-regime records on unbound orbits (P09.T29's deferred finding, for the grid's records),
/// and the range query's census near the black hole under each arm, which the walk's pad moves
/// only through the cell budget.
#[test]
#[ignore = "slow: generates the grid's central cells of every layer for three galaxies under \
            both arms, some minutes"]
fn the_kepler_arm_s_census_near_the_black_hole() {
    for (label, held) in census_galaxies() {
        let kepler = held.clone().with_rules(GalaxyRules::every_25_hold());
        let edge = held.centre_motion().influence_radius().value();
        println!("{label}: edge {edge:.3} ly");
        for layer in Layer::ALL {
            census_of_the_layer(&held, &kepler, layer);
        }
        census_of_the_pad(&held, &kepler);
    }
}

/// The census C1 of one layer's records within [`census_radius`], `held` against `kepler`.
fn census_of_the_layer(held: &Galaxy, kepler: &Galaxy, layer: Layer) {
    let radius = census_radius(held, layer);
    let (old, new) = (
        records_within(held, layer, radius),
        records_within(kepler, layer, radius),
    );
    let mut moves: [Moves; 4] = Default::default();
    let (mut in_regime, mut unbound, mut carved) = (0_u64, 0_u64, Vec::new());
    let mut kept = new.iter().peekable();
    for record in &old {
        let start = EpochState::of_grid_record(held, record);
        let regime = regime_of(held, record);
        if regime == Regime::Kepler {
            in_regime += 1;
            if held
                .centre_motion()
                .kepler_orbit(&start)
                .is_some_and(|orbit| orbit.eccentricity() >= 1.0)
            {
                unbound += 1;
            }
        }
        let inside = loss_cone_verdict(held, &start) == LossConeVerdict::Inside;
        let epoch = *record.epoch_position();
        assert_eq!(position_at(kepler, record, UniverseTime::EPOCH), epoch);
        if kept.peek() == Some(&record) {
            kept.next();
            assert!(!inside, "{:?} in the loss cone was placed", record.id());
        } else {
            assert!(
                inside,
                "{:?} was dropped outside the loss cone",
                record.id()
            );
            let key = CellKey::of(record.id()).expect("a grid ID");
            assert_eq!(
                evaluate_candidate(kepler, key, index_of(record)),
                CandidateOutcome::InLossCone
            );
            assert_eq!(
                resolve(kepler, record.id()),
                Err(ResolveSystemError::NoSuchSystem)
            );
            assert_eq!(resolve(held, record.id()), Ok(*record));
            carved.push(*record);
            continue;
        }
        for (slot, y) in moves.iter_mut().zip(TIMES) {
            let t = years(y);
            let (a, b) = (position_at(held, record, t), position_at(kepler, record, t));
            assert!(b.offset_metres().iter().all(|m| m.is_finite()));
            if a != b {
                assert_eq!(regime, Regime::Kepler, "{:?} moved outside", record.id());
                slot.movers += 1;
                slot.largest = slot.largest.max(apart(&a, &b));
            }
            // The walk's pad under the new arm holds the record from where it ends, for a sphere
            // of 0.01 ly about it there.
            let from_epoch = apart(&epoch, &b);
            let room = 0.01 + walk_pad(kepler, layer, &b, LightYears::new(0.01), t).value();
            assert!(
                from_epoch <= room,
                "{:?} at {y} yr: {from_epoch} ly from its epoch position, beyond {room}",
                record.id()
            );
            if from_epoch > 0.01 + pad_for(t, pad_speed(layer)).value() {
                slot.beyond_straight += 1;
            }
        }
    }
    assert_eq!(
        kept.next(),
        None,
        "{layer:?}: the new arm placed a new record"
    );
    let per_time: Vec<String> = TIMES
        .iter()
        .zip(&moves)
        .map(|(y, m)| {
            format!(
                "{y:+} yr {} moved, at most {:.4} ly, {} beyond the straight pad",
                m.movers, m.largest, m.beyond_straight
            )
        })
        .collect();
    println!(
        "  {layer:?} within {radius:.2} ly: {} records, {in_regime} in the regime ({unbound} \
         unbound about the mass inside), {} carved out; {}",
        old.len(),
        carved.len(),
        per_time.join("; ")
    );
    for record in &carved {
        let p = PointLy::from(record.epoch_position());
        println!(
            "    carved out: {:#018x}, {:.4} ly out, {:?}",
            record.id().raw(),
            distance_ly(&p),
            record.layer()
        );
    }
}

/// The range query's census near the black hole under each arm: for spheres at and near it, at
/// ±100 and ±1,000 yr, the layers it admits and the cells its walk would visit, for stellar
/// queries and for queries with the substellar layers.
fn census_of_the_pad(held: &Galaxy, kepler: &Galaxy) {
    let spheres = [
        (ly(0.0, 0.0, 0.0), 0.5),
        (ly(0.0, 0.0, 0.0), 2.0),
        (ly(0.0, 0.0, 0.0), 10.0),
        (ly(0.0, 0.0, 0.0), 50.0),
        (ly(3.0, 1.0, 0.0), 1.0),
        (ly(0.0, 12.0, 2.0), 3.0),
    ];
    let (mut queries, mut moved) = (0_u32, 0_u32);
    for (centre, radius) in spheres {
        for substellar in [
            SubstellarRequest::None,
            SubstellarRequest::BrownDwarfsAndRoguePlanets,
        ] {
            for y in TIMES {
                let query = RangeQuery::builder(centre, LightYears::new(radius))
                    .time(years(y))
                    .substellar(substellar)
                    .mass_floor(substellar.floor())
                    .build()
                    .expect("a valid query");
                let grid = expected_counts(held, query.centre(), query.radius(), substellar);
                let census = |galaxy: &Galaxy| {
                    let mut cells = 0_u64;
                    let census = decide_census(&query, &grid, &LayerCounts::ZERO, |layer| {
                        let n =
                            count_cells_in_sphere(layer, &sphere_for_layer(galaxy, &query, layer));
                        cells += n;
                        n
                    });
                    (census, cells)
                };
                let ((a, cells_a), (b, cells_b)) = (census(held), census(kepler));
                queries += 1;
                if a != b {
                    moved += 1;
                    println!(
                        "    census moved: {radius} ly at {centre:?}, {y:+} yr, {substellar:?}: \
                         {:?} ({:?}) against {:?} ({:?})",
                        a.complete_down_to(),
                        a.stopped_by(),
                        b.complete_down_to(),
                        b.stopped_by()
                    );
                }
                if cells_a != cells_b {
                    println!(
                        "    {radius} ly at {:.1} ly, {y:+} yr, {substellar:?}: cells counted \
                         {cells_a} held, {cells_b} Kepler (down to {:?})",
                        apart(&GalacticPosition::ORIGIN, &centre),
                        b.complete_down_to()
                    );
                }
            }
        }
    }
    println!("  the range query's census moved for {moved} of {queries} queries");
}

/// A range query near the black hole against a scan, under each arm: for spheres near and at the
/// black hole at ±100 and ±1,000 yr, the grid's hits are every record of the layers the census
/// admits that [`position_at`] puts inside the sphere at the time. The scan takes every record
/// that starts in the regime, wherever it is, and the straight-regime records within the radius
/// and the layer's straight pad, which their speeds, below the layer's pad speed, cannot leave
/// (plan 03's premise, asserted by `epoch_velocity` in debug builds). Under the held arm the
/// straight pad chooses the cells, and under the new one the Kepler pad does.
#[test]
#[ignore = "slow: generates layer E's eight central cells, some 1.7 million records, under both \
            arms, some minutes"]
fn a_range_query_near_the_black_hole_finds_what_a_scan_finds_under_either_arm() {
    let spheres = [
        (ly(0.5, 0.3, 0.2), 0.5),
        (ly(2.0, -1.5, 0.5), 1.0),
        (ly(-4.0, 1.0, 3.0), 2.0),
        (ly(0.0, 0.0, 0.0), 1.5),
    ];
    for galaxy in [held(), kepler()] {
        let edge = galaxy.centre_motion().influence_radius().value();
        let mut cells = KeepCells::default();
        let mut in_regime: BTreeMap<Layer, Vec<SystemRecord>> = BTreeMap::new();
        let (mut hits, mut moved_in) = (0_usize, 0_usize);
        for (centre, radius) in spheres {
            for y in TIMES {
                let t = years(y);
                let query = RangeQuery::builder(centre, LightYears::new(radius))
                    .time(t)
                    .build()
                    .expect("a valid query");
                let result = range_query(galaxy, &mut cells, &[], &query);
                let layers: LayerSet = result.census().layers();
                let mut want: Vec<SystemHit> = Vec::new();
                for layer in layers.iter() {
                    let regime = in_regime.entry(layer).or_insert_with(|| {
                        let mut near = records_within_from(&mut cells, galaxy, layer, edge);
                        near.retain(|r| regime_of(galaxy, r) == Regime::Kepler);
                        near
                    });
                    let reach = radius + pad_for(t, pad_speed(layer)).value() + 1e-6;
                    let around =
                        QuerySphere::new(centre, LightYears::new(reach), t, LightYears::ZERO)
                            .expect("a positive radius");
                    let mut straight = Vec::new();
                    for key in cells_in_sphere(layer, &around) {
                        cells.with_cell(galaxy, key, |cell| {
                            straight.extend(cell.iter().copied().filter(|r| {
                                regime_of(galaxy, r) == Regime::Straight
                                    && apart(&centre, r.epoch_position()) <= reach
                            }));
                        });
                    }
                    for record in regime.iter().chain(&straight) {
                        let at = position_at(galaxy, record, t);
                        let distance = apart(&centre, &at);
                        if distance <= radius {
                            want.push(SystemHit::new(*record, at, LightYears::new(distance)));
                            if apart(&centre, record.epoch_position())
                                > radius + pad_for(t, pad_speed(layer)).value()
                            {
                                moved_in += 1;
                            }
                        }
                    }
                }
                let mut got: Vec<SystemHit> = result.systems().to_vec();
                got.sort_by_key(SystemHit::id);
                want.sort_by_key(SystemHit::id);
                let ids = |hits: &[SystemHit]| hits.iter().map(SystemHit::id).collect::<Vec<_>>();
                assert_eq!(
                    ids(&got),
                    ids(&want),
                    "{:?}: {radius} ly at {centre:?}, {y:+} yr",
                    galaxy.rules()
                );
                for (g, w) in got.iter().zip(&want) {
                    assert_eq!(g.position(), w.position());
                }
                hits += got.len();
            }
        }
        println!(
            "{:?}: {hits} hits over {} queries, {moved_in} of them from beyond the straight pad",
            galaxy.rules(),
            spheres.len() * TIMES.len()
        );
        if galaxy.rules().kepler_regime == KeplerRegimeRules::Straight {
            assert_eq!(moved_in, 0, "the straight lines stay within their pad");
        }
    }
}
