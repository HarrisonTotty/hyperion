//! The tests of the exact orbit in the centre's potential, from generator version 25 (plan 09,
//! P09.T28.d.b; decision-galaxy-25-train §1.5 and §1.6), against the two-body law: the exact orbit
//! moving the Kepler-regime records bound in the centre's potential and no others, each within the
//! walk's pad and its Kepler orbit's shell (D2); the regime's edge; the same seed twice and in any
//! order, the centre's potential built first or last (D4); the centre's source against a scan of
//! its members on their exact orbits; and the census C1.
//!
//! The version's positions near the centre are pinned by the goldens, P09.T28.b.a's pre-pin among
//! them, and by D2's digest. Where a test needs the two-body law a record would follow without the
//! exact orbit, it computes it directly ([`Trajectory::of`], the Kepler orbit about the mass inside
//! the epoch radius, which the exact orbit's law keeps for an orbit unbound in the potential).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden::f64_digest;
use hyperion_testkit::order::assert_order_independent;

use super::kepler_regime_hold::{
    apart, census_galaxies, census_galaxies_under, census_radius, ly, records_within, years,
};
use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Population;
use crate::galaxy::features::centre::members::CentrePlacement;
use crate::galaxy::features::centre::{CentreMemberSource, KeepCentreOrbits};
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::{CellKey, Existence, generate_cell};
use crate::galaxy::query::{LayerSet, QuerySphere, SystemHit, SystemSource, position_at, walk_pad};
use crate::galaxy::rules::GalaxyRules;
use crate::id::{CentreMemberId, Layer, MemberSlot, SystemId};
use crate::rng::Seed;
use crate::units::Years;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

/// The Milky Way fixture's seed, P09.T28.b.a's pre-pin's and P09.T28.b.b's tests'.
const MILKY_WAY_SEED: u64 = 0x0928_b0a0_0000_0001;

/// The times the tests move records to, Julian years from the epoch: ±100 and ±1,000, which is
/// ±H.
const TIMES: [i64; 4] = [-1_000, -100, 100, 1_000];

/// The Milky Way fixture with its kinematic tables, so that its grid systems move.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(MILKY_WAY_SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
            .with_full_potential()
    })
}

/// The law a trajectory takes, by name, for the tallies.
fn law_of(trajectory: &Trajectory) -> &'static str {
    match trajectory.law {
        Law::Straight => "straight",
        Law::Still => "still",
        Law::Kepler(_) => "kepler",
        Law::Spherical(..) => "spherical",
        Law::NoSuchSystem => "no such system",
    }
}

/// Whether `at`, `t` from the epoch, lies within the walk's pad from `epoch` (for a sphere of
/// 0.01 ly about it, as P09.T28.b.b's census asks) and within the Kepler orbit's shell, if the
/// trajectory has one, to [`SHELL`] relative.
fn within_bounds(
    galaxy: &Galaxy,
    trajectory: &Trajectory,
    epoch: &GalacticPosition,
    at: &GalacticPosition,
    t: UniverseTime,
) -> bool {
    let layer = trajectory.start().record().layer();
    let room = 0.01 + walk_pad(galaxy, layer, at, LightYears::new(0.01), t).value();
    let padded = apart(epoch, at) <= room;
    let shelled = trajectory.orbit().is_none_or(|orbit| {
        let r = apart(&GalacticPosition::ORIGIN, at);
        let most = orbit.apocentre().map_or(f64::INFINITY, LightYears::value);
        r >= orbit.pericentre().value() * (1.0 - SHELL) && r <= most * (1.0 + SHELL)
    });
    padded && shelled
}

/// The relative margin of the shell test, P09.T29's (`SHELL_MARGIN`).
const SHELL: f64 = 1e-6;

/// A centre member at `p` ly moving at `velocity` m/s, whose velocity is its own.
fn member(p: PointLy, velocity: [f64; 3]) -> EpochState {
    let slot = MemberSlot::InCell {
        band: Layer::A,
        level: 4,
        cell: [0, 0, 0],
        index: 0,
    };
    let record = SystemRecord::from_parts(
        SystemId::from(CentreMemberId::new(slot).expect("a canonical slot")),
        GalacticPosition::from_light_years([p.x, p.y, p.z]).expect("near the centre"),
        SystemOrigin::CentreMember { attempt: 0 },
        Population::NuclearDisc,
        SolarMasses::new(1.0),
        Years::new(1e9),
    );
    EpochState::new(record, GalacticVelocity::new(velocity)).expect("a finite state")
}

/// The digest (`f64_digest`) of the exact orbit's positions in the D2 test: each record's cell and
/// offsets that the exact orbit moves, at −1,000, −100, +100 and +1,000 yr, in walk order. Blessed
/// natively on x86-64; the wasip1 suite compares it. Re-pinned by name, with its counts, in
/// `MGE_BOXY`'s potential (P15.T3.c; it was `0x5a3e_f8cd_0176_dd6d`, of 3,281 records and 13,124
/// record-times), and at generator version 26 (it was `0xea8a_e8e2_d8ea_ee1e`, of 3,279 and
/// 13,116).
const EXACT_POSITIONS_DIGEST: u64 = 0x6511_054f_8c58_03dd;

/// D2: the exact orbit moves the fixture's Kepler-regime grid records bound in the centre's
/// potential off their two-body orbits ([`position_at_two_body`]), bit for bit their trajectories'
/// states, and leaves every record outside the regime, every one unbound in Ψ (on its Kepler
/// orbit) and every record at the epoch where the two-body law puts it; no placed record is carved
/// out, since the loss cone's verdict reads the Kepler orbit; and every moved record lies within
/// the walk's pad and its Kepler orbit's shell. Layer B within 1.5 influence radii, at ±100 and
/// ±1,000 yr, as P09.T28.b.b's D2. Named `pinned` for the WebAssembly suite's filter (D5): the
/// counts and a digest of the positions are pinned, as blessed natively.
#[test]
fn the_exact_orbit_moves_bound_kepler_regime_records_and_no_others_pinned() {
    let galaxy = galaxy();
    let edge = galaxy.centre_motion().influence_radius().value();
    let records = records_within(galaxy, Layer::B, 1.5 * edge);
    let mut laws: BTreeMap<&str, u32> = BTreeMap::new();
    let (mut moved, mut farthest, mut positions) = (0_u32, 0.0_f64, Vec::new());
    for record in &records {
        let epoch = *record.epoch_position();
        assert_eq!(position_at(galaxy, record, UniverseTime::EPOCH), epoch);
        let start = EpochState::of_grid_record(galaxy, record);
        let trajectory = Trajectory::new(galaxy, start);
        *laws.entry(law_of(&trajectory)).or_default() += 1;
        for y in TIMES {
            let t = years(y);
            let (a, b) = (
                position_at_two_body(galaxy, &start, t),
                position_at(galaxy, record, t),
            );
            match &trajectory.law {
                Law::Spherical(..) => {
                    let StateAt::Exists(state) = trajectory.state_at(t) else {
                        unreachable!("an exact orbit exists at every time")
                    };
                    assert_eq!(&b, state.position(), "{:?} at {y} yr", record.id());
                    assert!(b.offset_metres().iter().all(|m| m.is_finite()));
                    assert!(
                        within_bounds(galaxy, &trajectory, &epoch, &b, t),
                        "{:?} at {y} yr: beyond its pad or shell",
                        record.id()
                    );
                    if a != b {
                        moved += 1;
                        farthest = farthest.max(apart(&a, &b));
                    }
                    positions.extend(b.cell().to_array().map(f64::from));
                    positions.extend(b.offset_metres());
                }
                Law::Straight | Law::Still | Law::Kepler(_) => {
                    assert_eq!(b, a, "{:?} off the exact law moved", record.id());
                }
                Law::NoSuchSystem => unreachable!("the cells hold no carved-out record"),
            }
        }
    }
    println!(
        "{} records within 1.5 influence radii, by law: {laws:?}; {moved} record-times moved off \
         their two-body orbits, the farthest by {farthest:.4e} ly",
        records.len()
    );
    assert_eq!(
        (laws.get("spherical").copied(), moved),
        (Some(3_296), 13_184),
        "pin the counts as measured"
    );
    assert_eq!(
        f64_digest(&positions),
        EXACT_POSITIONS_DIGEST,
        "{farthest} ly"
    );
}

/// A grid record's position under the two-body law, the exact orbit's oracle: [`Trajectory::of`]'s
/// state, and for a record the loss cone carves out its straight line, as
/// [`position_at`](crate::galaxy::query::position_at) gives one made by hand.
fn position_at_two_body(galaxy: &Galaxy, start: &EpochState, t: UniverseTime) -> GalacticPosition {
    match Trajectory::of(galaxy.centre_motion(), *start).state_at(t) {
        StateAt::Exists(state) => *state.position(),
        StateAt::NoSuchSystem => drifted(start.record(), start.velocity(), t),
    }
}

/// P09.T28.b's continuity at the regime's edge under the exact orbit: a member just inside the
/// influence radius on its exact orbit and one just outside on a straight line part a thousand
/// years on by the pull over the span, `½ G M(<r) t² ÷ r²`, as the two-body law does there
/// (`the_two_regimes_meet_at_the_influence_radius`), the mass now Ψ's, with its halo.
#[test]
fn the_exact_orbit_meets_the_straight_line_at_the_influence_radius() {
    let galaxy = Galaxy::from_params(Seed::new(MILKY_WAY_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture builds");
    let motion = galaxy.centre_motion();
    let edge = motion.influence_radius().value();
    let along = [0.6, 0.64, 0.48];
    let at = |r: f64| PointLy::new(along[0] * r, along[1] * r, along[2] * r);
    let span = 1_000.0 * SECONDS_PER_JULIAN_YEAR;
    let mut worst = 0.0_f64;
    for (n, velocity) in [
        [0.0, 0.0, 20.0],
        [0.0, 150.0, 0.0],
        [-200.0, 30.0, 40.0],
        [300.0, -250.0, 200.0],
    ]
    .into_iter()
    .enumerate()
    {
        let velocity = velocity.map(|c| c * 1e3);
        let (inner, outer) = (
            member(at(edge - 0.1), velocity),
            member(at(edge + 0.1), velocity),
        );
        let law = Trajectory::new(&galaxy, inner);
        assert_eq!(law_of(&law), "spherical");
        assert_eq!(law_of(&Trajectory::new(&galaxy, outer)), "straight");
        for y in [-1_000, 1_000] {
            let t = years(y);
            let moved = |start: &EpochState| match position_velocity_at(&galaxy, start, t) {
                StateAt::Exists(state) => start
                    .record()
                    .epoch_position()
                    .displacement_to(state.position())
                    .metres(),
                StateAt::NoSuchSystem => panic!("far from the loss cone"),
            };
            let (a, b) = (moved(&inner), moved(&outer));
            let gap = length([0, 1, 2].map(|i| a[i] - b[i])) / METRES_PER_LIGHT_YEAR;
            worst = worst.max(gap);
            assert!(gap < 0.014, "{gap} ly apart at {velocity:?} m/s");
            if n == 0 {
                let r = (edge - 0.1) * METRES_PER_LIGHT_YEAR;
                let mu = GravitationalParameter::from_solar_masses(
                    motion.profile().total_enclosed_mass(edge - 0.1),
                );
                let closed = 0.5 * mu.value() * span * span / (r * r) / METRES_PER_LIGHT_YEAR;
                assert!(
                    (gap / closed - 1.0).abs() < 0.02,
                    "{gap} ly against {closed}"
                );
            }
        }
    }
    println!("at the edge the exact orbit and the straight line part by at most {worst} ly");
}

/// D4 under the version's rules: a galaxy built under them is the same galaxy, with the same
/// positions to the bit, as the version's fixture given them after (`Galaxy::with_rules`), so that
/// `with_full_potential` carries its rules; its centre's potential is the one
/// [`CentrePotential::new`] builds; and positions, states and pads are the same in any order, on
/// one galaxy, warm, and on a fresh copy each time, where the first answer builds the centre's
/// motion and potential.
#[test]
fn the_exact_orbit_gives_the_same_seed_the_same_answers_twice_and_in_any_order() {
    let seed = Seed::new(MILKY_WAY_SEED);
    let again = Galaxy::from_params_with_rules(
        seed,
        GalaxyParams::milky_way_like(),
        GalaxyRules::of_version(GENERATOR_VERSION),
    )
    .expect("the Milky Way fixture builds")
    .with_full_potential();
    // Before anything reads its memos.
    let cold = again.clone();
    let given = galaxy()
        .clone()
        .with_rules(GalaxyRules::of_version(GENERATOR_VERSION));
    assert_eq!(again, given);
    let near = CellKey::containing(Layer::B, &ly(0.5, 0.5, 0.5)).expect("in the cube");
    let mut cell = Vec::new();
    generate_cell(&again, near, &mut cell);
    cell.retain(|r| regime_of(&again, r) == Regime::Kepler);
    let bound: Vec<SystemRecord> = cell
        .iter()
        .copied()
        .filter(|r| {
            law_of(&Trajectory::new(
                &again,
                EpochState::of_grid_record(&again, r),
            )) == "spherical"
        })
        .collect();
    assert!(
        bound.len() >= 3,
        "{} of {} bound in Ψ",
        bound.len(),
        cell.len()
    );
    let (first, middle, last) = (bound[0], bound[bound.len() / 2], bound[bound.len() - 1]);
    let asks = [
        Ask::At(first, years(-1_000)),
        Ask::At(middle, years(100)),
        Ask::At(last, years(1_000)),
        Ask::At(first, years(1)),
        Ask::Member(
            member(PointLy::new(0.2, -0.1, 0.05), [3e5, 5e5, -1e5]),
            years(-100),
        ),
        Ask::Member(
            member(PointLy::new(2.0, 1.0, -0.5), [-1e5, 1.2e5, 4e4]),
            years(1_000),
        ),
        Ask::Pad(Layer::A, ly(1.0, 0.0, 0.0), years(-1_000)),
    ];
    assert_order_independent(&asks, |ask| ask.answer(&again));
    assert_order_independent(&asks, |ask| ask.answer(&cold.clone()));
    // Warm, the memo built by the first answer, against cold, built by each.
    let warm: Vec<Answer> = asks.iter().map(|ask| ask.answer(&again)).collect();
    let fresh: Vec<Answer> = asks.iter().map(|ask| ask.answer(&cold.clone())).collect();
    assert_eq!(warm, fresh);
    let direct = CentrePotential::new(again.centre_motion());
    let (memo, built) = (again.centre_potential().values(), direct.values());
    assert_eq!(memo.len(), built.len());
    for (a, b) in memo.iter().zip(&built) {
        assert_same_bits(*a, *b);
    }
    for record in [first, middle, last] {
        for y in TIMES {
            assert_eq!(
                position_at(&again, &record, years(y)),
                position_at(&given, &record, years(y))
            );
        }
    }
}

/// The heap the exact orbits own: a trajectory's exact orbit, boxed; a cell's orbits, every
/// member's with its trajectory, shell and light; and the galaxy's potential, once read.
#[test]
fn the_exact_orbits_heap_is_counted() {
    let galaxy = Galaxy::from_params(Seed::new(MILKY_WAY_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture builds");
    // Before anything reads the centre's memos.
    let before = galaxy.heap_bytes();
    let exact = Trajectory::new(
        &galaxy,
        member(PointLy::new(0.2, 0.1, 0.0), [-1e5, 6e5, 0.0]),
    );
    assert!(exact.exact_orbit().is_some());
    assert_eq!(exact.heap_bytes(), size_of::<SphericalOrbit>());
    let (motion, potential) = (
        galaxy.centre_motion().heap_bytes(),
        galaxy.centre_potential().heap_bytes(),
    );
    assert!(potential > 10_000, "{potential} bytes");
    // The potential is shared (P09.T28.d.c), so its own value is on the heap too.
    assert_eq!(
        galaxy.heap_bytes(),
        before + motion + size_of::<CentrePotential>() + potential
    );
    let kept = KeepCentreOrbits::new(&galaxy);
    let source = CentreMemberSource::new(&galaxy, &kept).expect("the fixture's centre builds");
    let placement = source.placement();
    let cell = placement
        .grid()
        .cell(5, [16, 16, 24])
        .expect("a level-5 cell");
    let orbits = crate::galaxy::features::centre::CentreOrbitCache::orbits(
        &kept,
        &galaxy,
        placement,
        MassBand::A,
        cell,
    );
    let exact_orbits = orbits
        .trajectories()
        .iter()
        .filter(|t| t.exact_orbit().is_some())
        .count();
    assert!(exact_orbits > 0);
    assert_eq!(
        orbits.heap_bytes(),
        orbits.trajectories().len()
            * (size_of::<Trajectory>()
                + size_of::<[f64; 2]>()
                + size_of::<crate::galaxy::features::centre::CentreMemberLight>())
            + exact_orbits * size_of::<SphericalOrbit>()
    );
}

/// A question D4 asks a galaxy, in any order.
#[derive(Debug)]
enum Ask {
    At(SystemRecord, UniverseTime),
    Member(EpochState, UniverseTime),
    Pad(Layer, GalacticPosition, UniverseTime),
}

/// What a galaxy answers an [`Ask`].
#[derive(Debug, PartialEq)]
enum Answer {
    At(GalacticPosition),
    Member(StateAt),
    Pad(LightYears),
}

impl Ask {
    fn answer(&self, galaxy: &Galaxy) -> Answer {
        match self {
            Self::At(record, t) => Answer::At(position_at(galaxy, record, *t)),
            Self::Member(start, t) => Answer::Member(position_velocity_at(galaxy, start, *t)),
            Self::Pad(layer, centre, t) => {
                Answer::Pad(walk_pad(galaxy, *layer, centre, LightYears::new(0.5), *t))
            }
        }
    }
}

/// Every centre member with an epoch position within `radius` ly of the black hole, and the black
/// hole: a brute force, independent of the source's choice of cells.
fn members_within(galaxy: &Galaxy, centre: &CentrePlacement<'_>, radius: f64) -> Vec<EpochState> {
    let mut all = vec![EpochState::of_centre_member(&centre.black_hole())];
    for cell in centre
        .grid()
        .cells_touching(&PointLy::default(), LightYears::new(radius))
    {
        for band in MassBand::ALL {
            all.extend(
                centre
                    .members_in_cell(galaxy, band, cell)
                    .iter()
                    .filter(|m| {
                        apart(&GalacticPosition::ORIGIN, m.record().epoch_position()) <= radius
                    })
                    .map(EpochState::of_centre_member),
            );
        }
    }
    all
}

/// Nearest first, then by ID: the range query's order.
fn sort(hits: &mut [SystemHit]) {
    hits.sort_by(|a, b| {
        a.distance()
            .total_cmp(&b.distance())
            .then_with(|| a.id().raw().cmp(&b.id().raw()))
    });
}

/// The centre's source re-checked under the exact orbits (P09.T28.d.b's "What follows"): the
/// source chooses its cells by P09.T29's pads and skips members by their Kepler orbits' shells,
/// and its hits equal a brute-force scan of every member within 0.2 ly on its exact orbit, for
/// spheres within 0.1 ly of the black hole a decade either way, as P09.T29's fast test does under
/// the two-body law: under the version's rules and under every 27 hold, the exact orbit taking its
/// state near the epoch by the arc from the epoch (P09.T28.d.c) under both.
#[test]
fn the_centre_s_source_finds_what_a_scan_finds_on_the_exact_orbits() {
    for rules in [
        GalaxyRules::of_version(GENERATOR_VERSION),
        GalaxyRules::every_27_hold(),
    ] {
        the_centre_s_source_finds_what_a_scan_finds_under(rules);
    }
}

/// [`the_centre_s_source_finds_what_a_scan_finds_on_the_exact_orbits`] in a galaxy built under
/// `rules`.
fn the_centre_s_source_finds_what_a_scan_finds_under(rules: GalaxyRules) {
    let galaxy = Galaxy::from_params_with_rules(
        Seed::new(0x0929_0000_0000_0001),
        GalaxyParams::milky_way_like(),
        rules,
    )
    .expect("the Milky Way fixture builds");
    let kept = KeepCentreOrbits::new(&galaxy);
    let source = CentreMemberSource::new(&galaxy, &kept).expect("the fixture's centre builds");
    let members = members_within(&galaxy, source.placement(), 0.2);
    let exact = members
        .iter()
        .filter(|m| law_of(&Trajectory::new(&galaxy, **m)) == "spherical")
        .count();
    println!(
        "{} members within 0.2 ly, {exact} on exact orbits",
        members.len()
    );
    assert!(exact * 2 > members.len(), "{exact} of {}", members.len());
    let layers = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .fold(LayerSet::EMPTY, LayerSet::with);
    let mut moved_in = 0_usize;
    for y in [-10, 10] {
        let t = years(y);
        for (centre, radius) in [
            (ly(0.0, 0.0, 0.0), 0.02),
            (ly(0.05, -0.03, 0.02), 0.03),
            (ly(-0.02, 0.01, 0.06), 0.04),
        ] {
            let sphere = QuerySphere::new(centre, LightYears::new(radius), t, LightYears::ZERO)
                .expect("a positive radius");
            let mut found = Vec::new();
            source.systems_in_sphere(&galaxy, &sphere, layers, &mut found);
            sort(&mut found);
            let mut want: Vec<SystemHit> = members
                .iter()
                .filter(|m| matches!(m.record().existence_at(t), Existence::Exists))
                .filter_map(|m| match position_velocity_at(&galaxy, m, t) {
                    StateAt::Exists(state) => {
                        let d = LightYears::from(centre.distance_to(state.position()));
                        (d.value() <= radius)
                            .then(|| SystemHit::new(*m.record(), *state.position(), d))
                    }
                    StateAt::NoSuchSystem => None,
                })
                .collect();
            sort(&mut want);
            assert_eq!(found, want, "{radius} ly about {centre:?} at {y} yr");
            moved_in += found
                .iter()
                .filter(|h| apart(&centre, h.record().epoch_position()) > radius)
                .count();
        }
    }
    println!("{moved_in} hits moved in from outside their sphere");
    assert!(
        moved_in > 0,
        "the pads chose no cell beyond the spheres that mattered"
    );
}

/// The census C1 of decision-galaxy-25-train §1.6, the exact orbit against the two-body law
/// ([`position_at_two_body`]): over P09.T28.b.a's pre-pin (three galaxies, every grid record within
/// 1.5 influence radii, the substellar layers within 4 ly), at ±100 and ±1,000 yr (±H), the records
/// by law, the movers by count, the largest move, every mover finite, no record outside the exact
/// law moved, every record within the walk's pad and its Kepler orbit's shell, and no placed record
/// carved out (the loss cone's verdict reads the Kepler orbit, so the carve-outs are
/// P09.T28.b.b's). Under the version's rules and under every 27 hold, in the three galaxies built
/// under those rules, the exact orbit taking its state near the epoch by the arc from the epoch
/// (P09.T28.d.c) under both.
#[test]
#[ignore = "slow: generates the grid's central cells of every layer for three galaxies twice, \
            some minutes"]
fn the_exact_orbit_s_census_near_the_black_hole() {
    for (label, galaxy) in census_galaxies() {
        let edge = galaxy.centre_motion().influence_radius().value();
        println!("{label}: edge {edge:.3} ly");
        for layer in Layer::ALL {
            census_of_the_layer(&galaxy, layer);
        }
    }
    for (label, galaxy) in census_galaxies_under(GalaxyRules::every_27_hold()) {
        let edge = galaxy.centre_motion().influence_radius().value();
        println!("{label}, every 27 hold: edge {edge:.3} ly");
        for layer in Layer::ALL {
            census_of_the_layer(&galaxy, layer);
        }
    }
}

/// The census C1 of one layer's records within [`census_radius`], on their exact orbits against
/// the two-body law.
fn census_of_the_layer(galaxy: &Galaxy, layer: Layer) {
    let radius = census_radius(galaxy, layer);
    let records = records_within(galaxy, layer, radius);
    let mut laws: BTreeMap<&str, u64> = BTreeMap::new();
    let mut movers = [0_u64; 4];
    let mut largest = [0.0_f64; 4];
    for record in &records {
        let start = EpochState::of_grid_record(galaxy, record);
        let trajectory = Trajectory::new(galaxy, start);
        *laws.entry(law_of(&trajectory)).or_default() += 1;
        assert_ne!(
            law_of(&trajectory),
            "no such system",
            "{:?} in the loss cone was placed",
            record.id()
        );
        let epoch = *record.epoch_position();
        for (k, y) in TIMES.into_iter().enumerate() {
            let t = years(y);
            let (a, b) = (
                position_at_two_body(galaxy, &start, t),
                position_at(galaxy, record, t),
            );
            assert!(b.offset_metres().iter().all(|m| m.is_finite()));
            if a != b {
                assert_eq!(
                    law_of(&trajectory),
                    "spherical",
                    "{:?} moved off the exact law",
                    record.id()
                );
                movers[k] += 1;
                largest[k] = largest[k].max(apart(&a, &b));
            }
            if regime_of(galaxy, record) == Regime::Kepler {
                assert!(
                    within_bounds(galaxy, &trajectory, &epoch, &b, t),
                    "{:?} at {y} yr: beyond its pad or shell",
                    record.id()
                );
            }
        }
    }
    let per_time: Vec<String> = TIMES
        .iter()
        .zip(movers.iter().zip(&largest))
        .map(|(y, (m, l))| format!("{y:+} yr {m} moved, at most {l:.4e} ly"))
        .collect();
    println!(
        "  {layer:?} within {radius:.2} ly: {} records, by law {laws:?}; {}",
        records.len(),
        per_time.join("; ")
    );
}
