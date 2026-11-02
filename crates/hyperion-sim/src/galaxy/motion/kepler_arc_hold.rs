//! The tests of P09.T28.d.c's hold, [`KEPLER_ARC_VERSION`] (decision-galaxy-25-train §1.3's
//! pattern): its tripwire; and, under every 27 hold against the same rules with the series alone
//! (`kepler_arc` held), the arc from the epoch moving the Kepler-regime records on exact orbits and
//! no others, each within the walk's pad and its Kepler orbit's shell (D2); the grid's wide orbits
//! against a direct integration; and the same seed twice and in any order (D4). The centre's source
//! against a scan and the census C1 run under every 27 hold beside every 25 hold in
//! `kepler_secular_hold.rs`.
//!
//! The bit-identity of the version in force is held by the goldens, as for P09.T28.d.b's hold: below
//! 27 no galaxy builds an exact orbit with the arc, which the tripwire checks. The bump to 27
//! deletes the tripwire and the held arm, and these become tests of the version's own galaxy.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use hyperion_testkit::golden::f64_digest;
use hyperion_testkit::order::assert_order_independent;

use super::kepler_regime_hold::{apart, records_within, years};
use super::spherical::tests::{distance, integrate, profile_pull, turned};
use super::spherical::{KEPLER_ARC_VERSION, KeplerArcRules};
use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::query::{position_at, walk_pad};
use crate::galaxy::rules::GalaxyRules;
use crate::id::{Layer, SystemId};
use crate::rng::Seed;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

/// The Milky Way fixture's seed, P09.T28.b.a's pre-pin's and P09.T28.d.b's hold's.
const MILKY_WAY_SEED: u64 = 0x0928_b0a0_0000_0001;

/// The times the tests move records to, Julian years from the epoch: ±100 and ±1,000, which is
/// ±H.
const TIMES: [i64; 4] = [-1_000, -100, 100, 1_000];

/// The relative margin of the shell test, P09.T29's (`SHELL_MARGIN`).
const SHELL: f64 = 1e-6;

/// Every 27 hold but this one: the exact orbit by the series alone.
fn series_rules() -> GalaxyRules {
    GalaxyRules {
        kepler_arc: KeplerArcRules::Series,
        ..GalaxyRules::every_27_hold()
    }
}

/// The fixture with its kinematic tables, so that its grid systems move, built under every 27
/// hold, the holds read when a galaxy is built among them: the arc from the epoch.
fn arc() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(MILKY_WAY_SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_27_hold(),
        )
        .expect("the Milky Way fixture builds under every 27 hold")
        .with_full_potential()
    })
}

/// [`arc`]'s galaxy under [`series_rules`].
fn series() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| arc().clone().with_rules(series_rules()))
}

/// Whether `trajectory` follows an exact orbit and takes its states near the epoch by the arc.
fn on_the_arc(trajectory: &Trajectory) -> bool {
    match &trajectory.law {
        Law::Spherical(_, exact) => exact.takes_the_arc(),
        Law::Straight | Law::Still | Law::Kepler(_) | Law::NoSuchSystem => false,
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

/// P09.T28.d.c is held until the 26 → 27 bump: the rules in force are version 26's and before,
/// under which an exact orbit takes its state from the series at every time, and no galaxy's
/// trajectory takes the arc, however its records move.
#[test]
fn the_kepler_arc_is_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < KEPLER_ARC_VERSION,
        "the 26 → 27 bump puts P09.T28.d.c on the record: a non-circular exact orbit's state \
         within half a radial period of the epoch by the arc from the epoch \
         (`SphericalOrbit::by_arc`, through `Trajectory::new`). Delete `KEPLER_ARC_VERSION`, \
         `KeplerArcRules` with its `Series` arm, `GalaxyRules::kepler_arc`, the `match` on it and \
         this test, in the bump's own checkpoint commit"
    );
    let galaxy = Galaxy::from_params(Seed::new(MILKY_WAY_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture builds");
    assert_eq!(galaxy.rules().kepler_arc, KeplerArcRules::Series);
    assert_eq!(
        GalaxyRules::of_version(GENERATOR_VERSION).kepler_arc,
        KeplerArcRules::Series
    );
    assert_eq!(
        GalaxyRules::every_26_hold_with_its_retargets().kepler_arc,
        KeplerArcRules::Series
    );
    assert_eq!(
        GalaxyRules::every_27_hold().kepler_arc,
        KeplerArcRules::Local
    );
    // Under the held arm, an exact orbit at 26 takes the series alone.
    let start = member(PointLy::new(0.3, 0.1, 0.0), [-2e5, 6e5, 1e5]);
    for rules in [
        GalaxyRules::of_version(GENERATOR_VERSION),
        GalaxyRules::every_26_hold_with_its_retargets(),
    ] {
        let held = Galaxy::from_params_with_rules(
            Seed::new(MILKY_WAY_SEED),
            GalaxyParams::milky_way_like(),
            rules,
        )
        .expect("the Milky Way fixture builds");
        let trajectory = Trajectory::new(&held, start);
        assert!(trajectory.exact_orbit().is_some());
        assert!(!on_the_arc(&trajectory));
        assert_eq!(
            trajectory,
            Trajectory::in_potential(held.centre_motion(), held.centre_potential(), start)
        );
    }
}

/// A centre member at `p` ly moving at `velocity` m/s, whose velocity is its own.
fn member(p: PointLy, velocity: [f64; 3]) -> EpochState {
    use crate::galaxy::Population;
    use crate::id::{CentreMemberId, MemberSlot};
    use crate::units::Years;
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

/// The digest (`f64_digest`) of the arc's positions in the D2 test: each record's cell and offsets
/// that the arc moves off the series, at −1,000, −100, +100 and +1,000 yr, in walk order. Blessed
/// natively on x86-64; the wasip1 suite compares it.
const ARC_POSITIONS_DIGEST: u64 = 0x0921_95a6_2b91_b2fa;

/// D2 under every 27 hold: the arc from the epoch moves the fixture's Kepler-regime grid records
/// on exact orbits off the series' positions, bit for bit their trajectories' states, and leaves
/// every record outside the regime, every one on a Kepler orbit or a near-circular exact one, and
/// every record at the epoch where the series puts it; no record is made or lost, since the loss
/// cone's verdict reads the Kepler orbit; and every record on an exact orbit lies within the walk's
/// pad and its Kepler orbit's shell. Layer B within 1.5 influence radii, at ±100 and ±1,000 yr, as
/// P09.T28.d.b's D2. Named `pinned` for the WebAssembly suite's filter: the counts and a digest of
/// the positions are pinned, as blessed natively.
#[test]
fn the_arc_moves_records_on_exact_orbits_and_no_others_pinned() {
    let (series, arc) = (series(), arc());
    let edge = series.centre_motion().influence_radius().value();
    let records = records_within(series, Layer::B, 1.5 * edge);
    assert_eq!(records, records_within(arc, Layer::B, 1.5 * edge));
    let mut laws: BTreeMap<&str, u32> = BTreeMap::new();
    let (mut moved, mut farthest, mut positions) = (0_u32, 0.0_f64, Vec::new());
    for record in &records {
        let epoch = *record.epoch_position();
        assert_eq!(position_at(arc, record, UniverseTime::EPOCH), epoch);
        let start = EpochState::of_grid_record(arc, record);
        let trajectory = Trajectory::new(arc, start);
        let law = match &trajectory.law {
            Law::Spherical(..) if on_the_arc(&trajectory) => "arc",
            Law::Spherical(..) => "series",
            Law::Straight => "straight",
            Law::Still => "still",
            Law::Kepler(_) => "kepler",
            Law::NoSuchSystem => unreachable!("the cells hold no carved-out record"),
        };
        *laws.entry(law).or_default() += 1;
        for y in TIMES {
            let t = years(y);
            let (a, b) = (position_at(series, record, t), position_at(arc, record, t));
            if law == "arc" {
                let StateAt::Exists(state) = trajectory.state_at(t) else {
                    unreachable!("an exact orbit exists at every time")
                };
                assert_eq!(&b, state.position(), "{:?} at {y} yr", record.id());
                assert!(b.offset_metres().iter().all(|m| m.is_finite()));
                if a != b {
                    moved += 1;
                    farthest = farthest.max(apart(&a, &b));
                }
                positions.extend(b.cell().to_array().map(f64::from));
                positions.extend(b.offset_metres());
            } else {
                assert_eq!(b, a, "{:?} off the arc moved", record.id());
            }
            if trajectory.exact_orbit().is_some() {
                assert!(
                    within_bounds(arc, &trajectory, &epoch, &b, t),
                    "{:?} at {y} yr: beyond its pad or shell",
                    record.id()
                );
            }
        }
    }
    println!(
        "{} records within 1.5 influence radii, by law: {laws:?}; {moved} record-times moved off \
         the series, the farthest by {farthest:.4e} ly",
        records.len()
    );
    assert_eq!(
        (laws.get("arc").copied(), moved),
        (Some(3_281), 13_124),
        "pin the counts as measured"
    );
    assert_eq!(
        f64_digest(&positions),
        ARC_POSITIONS_DIGEST,
        "{farthest} ly"
    );
}

/// The grid's wide orbits under every 27 hold (P09.T28.d.c's test): every Kepler-regime record of
/// the fixture's layer B within 1.5 influence radii on an exact orbit whose Δ exceeds 2, its
/// apocentre at least e⁴ times its pericentre, at ±100 and ±1,000 yr against a direct
/// integration in the profile's pull (RK4 at 2 × 10⁻³ local dynamical times, converged against
/// 10⁻³ to a hundredth of the bound), lies within the plan's bound, a tenth of its tidal radius
/// at pericentre for a solar mass, by the arc from the epoch; where the series leaves some 500 of
/// them over it (research-leans-9's 252 of 6,562 at ±1,000 yr).
#[test]
fn the_arc_meets_the_plan_s_bound_on_the_grid_s_wide_orbits() {
    let (series, arc) = (series(), arc());
    let motion = arc.centre_motion();
    let pull = profile_pull(motion.profile());
    let edge = motion.influence_radius().value();
    let records = records_within(arc, Layer::B, 1.5 * edge);
    let (mut state_times, mut series_over, mut worst, mut worst_series) = (0_u32, 0_u32, 0.0, 0.0);
    let mut worst_id = None;
    for record in &records {
        let start = EpochState::of_grid_record(arc, record);
        let trajectory = Trajectory::new(arc, start);
        let (Law::Spherical(orbit, exact), true) = (&trajectory.law, on_the_arc(&trajectory))
        else {
            continue;
        };
        if exact.log_range() <= 2.0 {
            continue;
        }
        let bound = 0.1
            * motion
                .tidal_radius_at_pericentre(orbit, SolarMasses::new(1.0))
                .value();
        for y in TIMES {
            let t = years(y);
            let span = f64::from(i32::try_from(y).expect("a small span")) * SECONDS_PER_JULIAN_YEAR;
            let (truth, _) = integrate(&pull, orbit.position, orbit.velocity, span, 2e-3);
            let (finer, swept) = integrate(&pull, orbit.position, orbit.velocity, span, 1e-3);
            assert!(
                distance(finer, truth) < 1e-2 * bound,
                "{:?} at {y} yr: the reference converges to {} of the bound",
                record.id(),
                distance(finer, truth) / bound
            );
            let reference = turned(
                finer,
                (orbit.position, orbit.velocity),
                swept * exact.relativistic_advance_rad() / exact.apsidal_angle_rad(),
            );
            let error = |galaxy: &Galaxy| {
                let StateAt::Exists(state) = position_velocity_at(galaxy, &start, t) else {
                    unreachable!("an exact orbit exists at every time")
                };
                let got = GalacticPosition::ORIGIN
                    .displacement_to(state.position())
                    .metres();
                distance(got, reference) / bound
            };
            let (by_arc, by_series) = (error(arc), error(series));
            state_times += 1;
            series_over += u32::from(by_series > 1.0);
            worst_series = f64::max(worst_series, by_series);
            if by_arc > worst {
                worst = by_arc;
                worst_id = Some((record.id(), y));
            }
            assert!(
                by_arc < 1.0,
                "{:?} at {y} yr (Δ {:.3}): {by_arc} tenths, the series {by_series}",
                record.id(),
                exact.log_range()
            );
        }
    }
    println!(
        "{state_times} state-times of wide orbits: the arc's worst {worst:.3e} tenths of the tidal \
         radius ({worst_id:?}); the series {series_over} over the bound, the worst {worst_series:.3e}"
    );
    assert!(state_times > 5_000, "{state_times}");
}

/// D4 under every 27 hold: states on the arc are the same in any order, on one galaxy, warm, and
/// on a fresh copy each time, where the first answer builds the centre's motion and potential,
/// and the same on a galaxy built again.
#[test]
fn the_arc_gives_the_same_seed_the_same_answers_twice_and_in_any_order() {
    let again = Galaxy::from_params_with_rules(
        Seed::new(MILKY_WAY_SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_27_hold(),
    )
    .expect("the Milky Way fixture builds under every 27 hold")
    .with_full_potential();
    let cold = again.clone();
    assert_eq!(&again, arc());
    let edge = again.centre_motion().influence_radius().value();
    let on_arcs: Vec<EpochState> = records_within(arc(), Layer::B, 1.5 * edge)
        .iter()
        .map(|r| EpochState::of_grid_record(arc(), r))
        .filter(|start| on_the_arc(&Trajectory::new(arc(), *start)))
        .collect();
    assert!(on_arcs.len() >= 3, "{} on the arc", on_arcs.len());
    let (first, middle, last) = (
        on_arcs[0],
        on_arcs[on_arcs.len() / 2],
        on_arcs[on_arcs.len() - 1],
    );
    let asks = [
        (first, years(-1_000)),
        (middle, years(100)),
        (last, years(1_000)),
        (first, years(1)),
        (
            member(PointLy::new(0.2, -0.1, 0.05), [3e5, 5e5, -1e5]),
            years(-100),
        ),
        (
            member(PointLy::new(0.1, 0.0, 0.0), [2e5, 9e5, 3e5]),
            years(1_000),
        ),
    ];
    let answer = |galaxy: &Galaxy, (start, t): &(EpochState, UniverseTime)| {
        position_velocity_at(galaxy, start, *t)
    };
    assert_order_independent(&asks, |ask| answer(&again, ask));
    assert_order_independent(&asks, |ask| answer(&cold.clone(), ask));
    let warm: Vec<StateAt> = asks.iter().map(|ask| answer(&again, ask)).collect();
    let fresh: Vec<StateAt> = asks.iter().map(|ask| answer(&cold.clone(), ask)).collect();
    assert_eq!(warm, fresh);
    let elsewhere: Vec<StateAt> = asks.iter().map(|ask| answer(arc(), ask)).collect();
    assert_eq!(warm, elsewhere);
}
