//! The motion about the central black hole (plan 09, P09.T28): the Kepler propagator's goldens
//! (P09.T28.a), and the regime, the state at a time and the loss cone's verdict (P09.T28.b.a).
//!
//! The propagator is, with the Eddington inversion, the piece of the centre most exposed to a
//! platform difference (plan 09, "Verification"), so its states are pinned as bits and checked on
//! every CI target, x86-64, `AArch64` and wasm32. Nothing generated reads it until P09.T28.b.b
//! wires it into the drift hook: these goldens pin the propagator alone and today's positions near
//! the centre, and move no system.

use std::sync::OnceLock;

use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
use hyperion_sim::galaxy::consts::G;
use hyperion_sim::galaxy::features::centre::CentreModel;
use hyperion_sim::galaxy::features::centre::marks::{kepler_pericentre, loss_cone_radius};
use hyperion_sim::galaxy::features::centre::members::resolve_centre_member;
use hyperion_sim::galaxy::motion::{
    CentreMotion, EpochState, KeplerOrbit, LossConeVerdict, Regime, StateAt, loss_cone_verdict,
    position_velocity_at, regime_of,
};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemOrigin, SystemRecord, generate_cell};
use hyperion_sim::galaxy::query::{QuerySphere, cells_in_sphere, position_at};
use hyperion_sim::galaxy::{Galaxy, PointLy, Population};
use hyperion_sim::id::{CentreMemberId, Layer, MemberSlot, SystemId};
use hyperion_sim::time::{ClockWindow, UniverseTime};
use hyperion_sim::units::consts::{METRES_PER_AU, METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR};
use hyperion_sim::units::{GravitationalParameter, LightYears, Seconds, SolarMasses, Years};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, f64_digest};
use hyperion_testkit::order::assert_order_independent;

/// One fixture state: position relative to the black hole (ly), velocity (km/s), the mass orbited
/// (M☉), and what it is.
struct Case {
    position: [f64; 3],
    velocity_km_s: [f64; 3],
    mass: f64,
    what: &'static str,
}

/// Bound orbits from circular to e ≈ 0.9999 at 0.001–10 ly, S2 near apocentre, nearly and exactly
/// (to rounding) parabolic states, hyperbolic flybys, a retrograde orbit and one tilted out of
/// every plane.
const CASES: [Case; 12] = [
    Case {
        position: [1.0, 0.0, 0.0],
        velocity_km_s: [0.0, 245.51, 0.0],
        mass: 4.297e6,
        what: "near-circular at 1 ly",
    },
    Case {
        position: [0.030_8, 0.001, -0.000_5],
        velocity_km_s: [-20.0, 470.0, 35.0],
        mass: 4.297e6,
        what: "S2-like near apocentre",
    },
    Case {
        position: [0.001, 0.0, 0.0],
        velocity_km_s: [0.0, 30.0, 5.0],
        mass: 4.297e6,
        what: "e near 1 at 0.001 ly",
    },
    Case {
        position: [3.0, -2.0, 1.5],
        velocity_km_s: [40.0, 60.0, -25.0],
        mass: 4.8e6,
        what: "eccentric at 4 ly with enclosed stars",
    },
    Case {
        position: [9.5, 1.0, 0.2],
        velocity_km_s: [-30.0, 70.0, 10.0],
        mass: 8.6e6,
        what: "near the influence radius",
    },
    Case {
        position: [0.1, 0.0, 0.0],
        velocity_km_s: [0.0, -600.0, 0.0],
        mass: 4.297e6,
        what: "retrograde at 0.1 ly",
    },
    Case {
        position: [0.2, 0.3, -0.1],
        velocity_km_s: [-150.0, 250.0, 330.0],
        mass: 4.297e6,
        what: "tilted, bound",
    },
    Case {
        position: [0.5, 0.0, 0.0],
        velocity_km_s: [0.0, 0.0, 491.0],
        mass: 4.297e6,
        what: "nearly parabolic, bound",
    },
    Case {
        position: [0.5, 0.0, 0.0],
        velocity_km_s: [0.0, 0.0, 491.05],
        mass: 4.297e6,
        what: "nearly parabolic, unbound",
    },
    Case {
        position: [2.0, 1.0, 0.0],
        velocity_km_s: [-900.0, -300.0, 100.0],
        mass: 4.297e6,
        what: "hyperbolic, inbound",
    },
    Case {
        position: [0.05, 0.02, 0.01],
        velocity_km_s: [2_500.0, 1_000.0, -800.0],
        mass: 4.297e6,
        what: "hypervelocity, outbound",
    },
    Case {
        position: [-0.4, 0.7, 0.9],
        velocity_km_s: [60.0, -45.0, 20.0],
        mass: 5.2e6,
        what: "slow, deep plunge",
    },
];

/// Times from the epoch, Julian years: across the clock window, both signs.
const TIMES_YEARS: [f64; 6] = [-1_000.0, -16.0, -0.25, 0.25, 16.0, 1_000.0];

#[test]
fn kepler_propagator_states_are_pinned() {
    let mut writer = GoldenWriter::new();
    writer.header(GENERATOR_VERSION.get());
    writer.line("# KeplerOrbit::propagate about the central black hole (plan 09, P09.T28.a).");
    writer.line("# Per case and time: position (ly) and velocity (m/s) on the galactic axes.");
    for (n, case) in CASES.iter().enumerate() {
        let [x, y, z] = case.position;
        let orbit = KeplerOrbit::from_state(
            PointLy::new(x, y, z),
            GalacticVelocity::new(case.velocity_km_s.map(|v| v * 1e3)),
            GravitationalParameter::from_solar_masses(SolarMasses::new(case.mass)),
        )
        .expect("fixture states are valid");
        writer.line(&format!("# case[{n:02}]: {}", case.what));
        writer.f64(&format!("case[{n:02}].eccentricity"), orbit.eccentricity());
        writer.f64(
            &format!("case[{n:02}].pericentre_ly"),
            orbit.pericentre().value(),
        );
        for (k, years) in TIMES_YEARS.into_iter().enumerate() {
            let state = orbit.propagate(Seconds::new(years * SECONDS_PER_JULIAN_YEAR));
            let position = state.position();
            let velocity = state.velocity().metres_per_second();
            let label = format!("case[{n:02}].t[{k}]");
            for (axis, value) in ["x", "y", "z"]
                .into_iter()
                .zip([position.x, position.y, position.z])
            {
                writer.f64(&format!("{label}.{axis}"), value);
            }
            for (axis, value) in ["vx", "vy", "vz"].into_iter().zip(velocity) {
                writer.f64(&format!("{label}.{axis}"), value);
            }
        }
    }
    golden!("galaxy/motion/kepler", writer.as_str());
}

/// The Milky Way fixture without its kinematic tables: the regime and the orbits need only the
/// centre, and its grid systems have no velocities.
fn milky_way() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(
            Seed::new(0x0928_b0a0_0000_0001),
            GalaxyParams::milky_way_like(),
        )
        .expect("the Milky Way fixture's gas is mostly neutral")
    })
}

/// A centre member's record at `ly` from the black hole: a source whose velocity is its own.
fn member_at(ly: [f64; 3]) -> SystemRecord {
    let slot = MemberSlot::InCell {
        band: Layer::A,
        level: 4,
        cell: [0, 0, 0],
        index: 0,
    };
    SystemRecord::from_parts(
        SystemId::from(CentreMemberId::new(slot).expect("a canonical slot")),
        GalacticPosition::from_light_years(ly).expect("near the centre"),
        SystemOrigin::CentreMember { attempt: 0 },
        Population::NuclearDisc,
        SolarMasses::new(1.0),
        Years::new(1e9),
    )
}

/// A grid record at `ly` from the black hole, of the galaxy's first component.
fn grid_at(galaxy: &Galaxy, ly: [f64; 3]) -> SystemRecord {
    let key = CellKey::new(Layer::C, [0, 0, 0]).expect("the central cell");
    SystemRecord::from_parts(
        key.candidate_id(1).expect("candidate 1 exists"),
        GalacticPosition::from_light_years(ly).expect("near the centre"),
        SystemOrigin::Grid(
            galaxy
                .fields()
                .component_id(0)
                .expect("one component at least"),
        ),
        Population::NuclearDisc,
        SolarMasses::new(1.0),
        Years::new(1e9),
    )
}

/// An epoch state the tests build, finite and slower than light.
fn epoch_state(record: SystemRecord, velocity: GalacticVelocity) -> EpochState {
    EpochState::new(record, velocity).expect("a finite velocity below light's")
}

fn km_s(v: [f64; 3]) -> GalacticVelocity {
    GalacticVelocity::new(v.map(|c| c * 1e3))
}

fn years(y: i64) -> UniverseTime {
    UniverseTime::from_julian_years(y).expect("inside the clock window")
}

/// The state at `t`, which must exist.
fn state(
    galaxy: &Galaxy,
    start: &EpochState,
    t: UniverseTime,
) -> (GalacticPosition, GalacticVelocity) {
    match position_velocity_at(galaxy, start, t) {
        StateAt::Exists(state) => (*state.position(), state.velocity()),
        StateAt::NoSuchSystem => panic!("{start:?} was carved out"),
    }
}

/// The distance between two positions, ly.
fn apart(a: &GalacticPosition, b: &GalacticPosition) -> f64 {
    a.distance_to(b).value() / METRES_PER_LIGHT_YEAR
}

/// The displacement from the epoch to `t`, metres.
fn moved(galaxy: &Galaxy, start: &EpochState, t: UniverseTime) -> [f64; 3] {
    let (at, _) = state(galaxy, start, t);
    start
        .record()
        .epoch_position()
        .displacement_to(&at)
        .metres()
}

fn assert_same_state(
    a: (GalacticPosition, GalacticVelocity),
    b: (GalacticPosition, GalacticVelocity),
) {
    assert_eq!(a.0.cell(), b.0.cell());
    for (x, y) in a.0.offset_metres().into_iter().zip(b.0.offset_metres()) {
        assert_same_bits(x, y);
    }
    for (x, y) in
        a.1.metres_per_second()
            .into_iter()
            .zip(b.1.metres_per_second())
    {
        assert_same_bits(x, y);
    }
}

/// Design note 12 at the edge of the regime: a system just inside the influence radius follows
/// its orbit and one just outside a straight line, and a thousand years on the two laws part by
/// the pull over the span, `½ μ t² ÷ r²` to leading order, `G M_bh t² ÷ r²` at the edge, where the
/// mass inside weighs the black hole. The plan's figure, under 0.01 ly at 9.9 and 10.1 ly, is for
/// the brainstorm's "about 10 ly"; the fixture's edge is 7.19 ly, where the closed form is
/// 0.0130 ly (0.0132 ly 0.1 ly inside it) and the fastest states part by 0.0136 ly, so that is
/// pinned as it stands, and the two laws are held to the plan's 0.01 ly at its 10 ly (P09.T28.b.a
/// as built).
#[test]
fn the_two_regimes_meet_at_the_influence_radius() {
    let galaxy = milky_way();
    let motion = galaxy.centre_motion();
    let edge = motion.influence_radius().value();
    assert!((7.0..7.4).contains(&edge), "{edge} ly");
    let along = [0.6, 0.64, 0.48];
    let at = |r: f64| along.map(|c| c * r);
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
        let inner = epoch_state(member_at(at(edge - 0.1)), km_s(velocity));
        let outer = epoch_state(member_at(at(edge + 0.1)), km_s(velocity));
        assert_eq!(regime_of(galaxy, inner.record()), Regime::Kepler);
        assert_eq!(regime_of(galaxy, outer.record()), Regime::Straight);
        for t in [ClockWindow::START, ClockWindow::END] {
            let (a, b) = (moved(galaxy, &inner, t), moved(galaxy, &outer, t));
            let gap = (0..3)
                .map(|i| (a[i] - b[i]) * (a[i] - b[i]))
                .sum::<f64>()
                .sqrt()
                / METRES_PER_LIGHT_YEAR;
            worst = worst.max(gap);
            assert!(gap < 0.014, "{gap} ly apart at {velocity:?} km/s");
            if n == 0 {
                // Slow enough that the pull barely changes over the span.
                let r = (edge - 0.1) * METRES_PER_LIGHT_YEAR;
                let mu = GravitationalParameter::from_solar_masses(
                    motion.enclosed_mass(LightYears::new(edge - 0.1)),
                );
                let closed = 0.5 * mu.value() * span * span / (r * r) / METRES_PER_LIGHT_YEAR;
                assert!(
                    (gap / closed - 1.0).abs() < 0.02,
                    "{gap} ly against {closed}"
                );
                assert!(
                    (closed - 0.0132).abs() < 2e-4,
                    "{closed} ly 0.1 ly inside the edge"
                );
            }
        }
    }
    println!("at the edge the two laws part by at most {worst} ly over 1,000 yr");
    println!(
        "M(< 10 ly) = {:e} M☉ against the black hole's {:e}",
        motion.stars_within(LightYears::new(10.0)).value(),
        motion.profile().black_hole().value()
    );
    // The plan's figure where the brainstorm puts the edge, by the two laws directly.
    for r in [9.9, 10.1] {
        let position = PointLy::new(r * 0.6, r * 0.64, r * 0.48);
        let mu =
            GravitationalParameter::from_solar_masses(motion.enclosed_mass(LightYears::new(r)));
        for velocity in [
            [0.0, 0.0, 20.0],
            [0.0, 150.0, 0.0],
            [-200.0, 30.0, 40.0],
            [300.0, -250.0, 200.0],
        ] {
            let v = km_s(velocity);
            let orbit = KeplerOrbit::from_state(position, v, mu).expect("a valid state");
            let later = orbit.propagate(Seconds::new(span)).position();
            let line = [position.x, position.y, position.z]
                .into_iter()
                .zip(v.metres_per_second())
                .map(|(p, c)| p + c * span / METRES_PER_LIGHT_YEAR);
            let gap = [later.x, later.y, later.z]
                .into_iter()
                .zip(line)
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f64>()
                .sqrt();
            assert!(gap < 0.01, "{gap} ly apart at {r} ly and {velocity:?} km/s");
        }
    }
}

/// At the epoch either law returns the epoch state to the bit: for centre members and grid
/// systems, inside the regime and out, the black hole itself, and a grid system with no velocity.
#[test]
fn the_epoch_state_comes_back_to_the_bit() {
    let galaxy = milky_way();
    let black_hole = resolve_centre_member(galaxy, CentreMemberId::CENTRAL_BLACK_HOLE)
        .expect("the black hole resolves");
    let starts = [
        epoch_state(
            member_at([0.03, 0.001, -0.000_5]),
            km_s([-20.0, 470.0, 35.0]),
        ),
        epoch_state(member_at([3.0, -2.0, 1.5]), km_s([40.0, 60.0, -25.0])),
        epoch_state(member_at([30.0, 2.0, 1.5]), km_s([40.0, 60.0, -25.0])),
        epoch_state(grid_at(galaxy, [1.0, 0.5, 0.2]), km_s([-90.0, 140.0, 10.0])),
        epoch_state(
            grid_at(galaxy, [12.0, 0.5, 0.2]),
            km_s([-90.0, 140.0, 10.0]),
        ),
        epoch_state(
            grid_at(galaxy, [1.0, 0.5, 0.2]),
            GalacticVelocity::default(),
        ),
        EpochState::of_centre_member(&black_hole),
    ];
    for start in &starts {
        let epoch = (*start.record().epoch_position(), start.velocity());
        assert_same_state(state(galaxy, start, UniverseTime::EPOCH), epoch);
    }
    assert_eq!(regime_of(galaxy, black_hole.record()), Regime::Kepler);
    // The black hole has no orbit and stays where it is.
    let (at, _) = state(
        galaxy,
        &EpochState::of_centre_member(&black_hole),
        ClockWindow::END,
    );
    assert_eq!(at, GalacticPosition::ORIGIN);
}

/// An S2-like member follows P09.T28.a's propagator about the black hole and the stars inside its
/// epoch radius, as the centre's model weighs them: a 16-year period, back where it started after
/// each, and the same state as the propagator gives at every time across the clock window.
#[test]
fn an_s2_like_member_follows_its_kepler_orbit() {
    let galaxy = milky_way();
    let model = CentreModel::new(galaxy).expect("the fixture's centre builds");
    // S2's elements (GRAVITY Collaboration 2020, A&A 636, L5, table E.1), started at apocentre.
    let (a, e) = (0.0163, 0.8846);
    let apocentre = a * (1.0 + e);
    let record = member_at([apocentre, 0.0, 0.0]);
    // The black hole and every star inside the epoch radius, from the centre's model, at the
    // radius the record holds.
    let at = PointLy::from(record.epoch_position());
    let r0 = (at.x * at.x + at.y * at.y + at.z * at.z).sqrt();
    let profile = model.profile();
    let inside = profile.stellar_mass_within(r0)
        + profile
            .galaxy()
            .expect("the model's profile is built in the galaxy")
            .extended_mass(LightYears::new(r0));
    let mu = GravitationalParameter::from_solar_masses(profile.black_hole() + inside);
    let speed = (mu.value() * (1.0 - e) / (apocentre * METRES_PER_LIGHT_YEAR)).sqrt();
    let start = epoch_state(record, GalacticVelocity::new([0.0, speed, 0.0]));
    assert_eq!(regime_of(galaxy, start.record()), Regime::Kepler);
    let orbit = KeplerOrbit::from_state(
        PointLy::from(start.record().epoch_position()),
        start.velocity(),
        mu,
    )
    .expect("a valid state");
    let period = orbit.period().expect("bound").value() / SECONDS_PER_JULIAN_YEAR;
    assert!((period - 16.0).abs() < 0.1, "{period} yr");
    for t in [-1_000, -333, -16, -1, 1, 8, 16, 333, 1_000] {
        let (at, velocity) = state(galaxy, &start, years(t));
        let expected = orbit.propagate(Seconds::new(years(t).since_epoch().as_seconds_f64()));
        let p = expected.position();
        let off = apart(
            &at,
            &GalacticPosition::from_light_years([p.x, p.y, p.z]).expect("in range"),
        );
        assert!(off < 1e-12 * apocentre + 1e-15, "{off} ly off after {t} yr");
        for (x, y) in velocity
            .metres_per_second()
            .into_iter()
            .zip(expected.velocity().metres_per_second())
        {
            assert_same_bits(x, y);
        }
    }
    // The same state twice gives the same bits.
    assert_eq!(
        position_velocity_at(galaxy, &start, years(333)),
        position_velocity_at(galaxy, &start, years(333))
    );
    // Whole periods bring it back.
    let back = apart(
        start.record().epoch_position(),
        &state(galaxy, &start, years(16)).0,
    );
    assert!(
        back < 0.01 * apocentre,
        "{back} ly from its start after 16 yr"
    );
}

/// A galaxy built without its kinematic tables gives its grid systems no velocity, so a grid
/// system in the regime has no orbit and keeps its epoch position at every time, and is not
/// carved out (P09.T28.a as built: `Radial`).
#[test]
fn a_galaxy_without_kinematic_tables_keeps_its_grid_systems_still() {
    let galaxy = milky_way();
    assert!(galaxy.kinematics().is_none());
    let edge = galaxy.centre_motion().influence_radius().value();
    let record = grid_at(galaxy, [0.3 * edge, -0.2 * edge, 0.1 * edge]);
    let start = EpochState::of_grid_record(galaxy, &record);
    assert_eq!(start.velocity(), GalacticVelocity::default());
    assert_eq!(regime_of(galaxy, &record), Regime::Kepler);
    assert_eq!(loss_cone_verdict(galaxy, &start), LossConeVerdict::Clear);
    for t in [
        ClockWindow::START,
        years(-100),
        years(100),
        ClockWindow::END,
    ] {
        let epoch = (*record.epoch_position(), GalacticVelocity::default());
        assert_same_state(state(galaxy, &start, t), epoch);
    }
}

/// The loss cone's verdict on a pinned plunging orbit: a grid system at 1 ly falling in at
/// 200 km/s with 1.38 km/s across passes about 1 au from the black hole, inside its 2 au loss
/// cone, so there is no such system at any time; the same state outside the regime follows its
/// straight line, and a centre member in it keeps the verdict its placement gave.
#[test]
fn the_loss_cone_carves_out_a_plunging_grid_orbit() {
    let galaxy = milky_way();
    let black_hole = galaxy.params().black_hole().mass();
    let plunging = km_s([-200.0, 1.38, 0.0]);
    let grazing = km_s([-200.0, 3.0, 0.0]);
    let au_per_ly = METRES_PER_AU / METRES_PER_LIGHT_YEAR;
    // The members' test: the Kepler pericentre about the black hole alone, ly and km/s.
    let pericentre_au = |v: GalacticVelocity, at: [f64; 3]| {
        let mu = G * black_hole.value();
        kepler_pericentre(at, v.metres_per_second().map(|c| c / 1e3), mu) / au_per_ly
    };
    let at = [1.0, 0.0, 0.0];
    let q = pericentre_au(plunging, at);
    let cone = loss_cone_radius(black_hole).value() / au_per_ly;
    assert!(
        (0.8..1.2).contains(&q) && q < cone,
        "pericentre {q} au, cone {cone} au"
    );
    assert!(pericentre_au(grazing, at) > cone);

    let carved = epoch_state(grid_at(galaxy, at), plunging);
    assert_eq!(regime_of(galaxy, carved.record()), Regime::Kepler);
    assert_eq!(loss_cone_verdict(galaxy, &carved), LossConeVerdict::Inside);
    for t in [
        ClockWindow::START,
        UniverseTime::EPOCH,
        years(100),
        ClockWindow::END,
    ] {
        assert_eq!(
            position_velocity_at(galaxy, &carved, t),
            StateAt::NoSuchSystem
        );
    }
    let clear = epoch_state(grid_at(galaxy, at), grazing);
    assert_eq!(loss_cone_verdict(galaxy, &clear), LossConeVerdict::Clear);
    let member = epoch_state(member_at(at), plunging);
    assert_eq!(loss_cone_verdict(galaxy, &member), LossConeVerdict::Clear);
    let far = galaxy.centre_motion().influence_radius().value() * 1.5;
    let outside = epoch_state(grid_at(galaxy, [far, 0.0, 0.0]), plunging);
    assert_eq!(loss_cone_verdict(galaxy, &outside), LossConeVerdict::Clear);
    assert!(matches!(
        position_velocity_at(galaxy, &outside, ClockWindow::END),
        StateAt::Exists(_)
    ));
}

/// The centre's motion holds the centre model's profile and loss cone, bit for bit, and weighs
/// every star inside against the black hole: the cluster's and the rest of the galaxy's. At Milky
/// Way values the cluster is nearly all of them and the edge moves by 0.2% from the model's
/// cluster-only radius; in a galaxy whose black hole outweighs its whole cluster, as a fifth of
/// drawn galaxies' does, the model's radius is the end of its bracket, 10⁴ ly, and the regime's
/// edge is the rest of the galaxy's (main's ruling of 2026-10-27 on P09.T28.b.a). The galaxy
/// builds it when first read, the same bits as a direct build, with its equality and `Debug` text
/// unchanged by the read; the same seed builds it twice alike.
#[test]
fn the_centre_motion_weighs_every_star_inside() {
    let galaxy = milky_way();
    let motion = galaxy.centre_motion();
    let model = CentreModel::new(galaxy).expect("the fixture's centre builds");
    assert_eq!(motion.profile(), model.profile());
    assert_eq!(motion.loss_cone(), model.loss_cone());
    let edge = motion.influence_radius();
    let cluster_only = model.influence_radius().value();
    assert!(
        (edge.value() / cluster_only - 1.0).abs() < 0.005,
        "{} ly against the cluster's {cluster_only} ly",
        edge.value()
    );
    // The definition: the stars inside the edge weigh the black hole.
    let weighed = motion.stars_within(edge).value() / model.profile().black_hole().value();
    assert!((weighed - 1.0).abs() < 1e-9, "{weighed}");
    for r in [1e-4, 0.0163, 0.3, 3.0, 7.2, 9.9, 10.1, 128.0] {
        let r = LightYears::new(r);
        let rest = model
            .profile()
            .galaxy()
            .expect("in the galaxy")
            .extended_mass(r);
        assert!(
            motion.stars_within(r).value()
                >= model.profile().stellar_mass_within(r.value()).value()
        );
        assert_same_bits(
            motion.enclosed_mass(r).value(),
            (model.profile().black_hole()
                + (model.profile().stellar_mass_within(r.value()) + rest))
                .value(),
        );
    }
    // Built when first read, and the same bits as a direct build; a galaxy not yet read equals one
    // that has been, and so does its `Debug` text.
    let direct = CentreMotion::new(galaxy.params(), galaxy.mass_model()).expect("valid masses");
    assert_eq!(motion, &direct);
    let again = Galaxy::from_params(
        Seed::new(0x0928_b0a0_0000_0001),
        GalaxyParams::milky_way_like(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral");
    assert_eq!(&again, galaxy);
    let unread = format!("{again:?}");
    assert_eq!(again.centre_motion(), motion);
    assert_eq!(format!("{again:?}"), unread);
    assert_eq!(format!("{galaxy:?}"), unread);

    // Seed 1's black hole, 4.2 × 10⁷ M☉, outweighs its 8.3 × 10⁶ M☉ cluster.
    let heavy = Galaxy::new(Seed::new(1));
    let model = CentreModel::new(&heavy).expect("a drawn centre builds");
    let bh = model.profile().black_hole().value();
    assert!(model.profile().mass().value() < bh);
    // The cluster's own radius finds no root and stops at its bracket's end (a finding, recorded).
    assert!(model.influence_radius().value() > 9_999.0);
    let edge = heavy.centre_motion().influence_radius();
    assert!((50.0..120.0).contains(&edge.value()), "{} ly", edge.value());
    let weighed = heavy.centre_motion().stars_within(edge).value() / bh;
    assert!((weighed - 1.0).abs() < 1e-9, "{weighed}");
}

/// Reading the centre's motion adds its tables to the galaxy's heap, some 20 KiB, and nothing
/// before it is read.
#[test]
fn the_centre_motion_s_tables_count_once_read() {
    let galaxy = Galaxy::from_params(
        Seed::new(0x0928_b0a0_0000_0003),
        GalaxyParams::milky_way_like(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral");
    let before = galaxy.heap_bytes();
    let _ = galaxy.centre_motion();
    let added = galaxy.heap_bytes() - before;
    assert!((10 << 10..64 << 10).contains(&added), "{added} bytes");
}

/// The motion's three readers give the same answers in any order, and on a galaxy whose centre
/// motion was never read as on one whose was: the lazy value is the same whoever reads it first
/// (the sim-determinism skill's lazy exception).
#[test]
fn the_motion_is_order_independent_pinned() {
    let unread = Galaxy::from_params(
        Seed::new(0x0928_b0a0_0000_0004),
        GalaxyParams::milky_way_like(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral");
    let black_hole = resolve_centre_member(&unread, CentreMemberId::CENTRAL_BLACK_HOLE)
        .expect("the black hole resolves");
    let edge = 7.19;
    let starts = [
        epoch_state(
            member_at([0.03, 0.001, -0.000_5]),
            km_s([-20.0, 470.0, 35.0]),
        ),
        epoch_state(member_at([0.0, 1.2 * edge, 0.5]), km_s([40.0, 60.0, -25.0])),
        epoch_state(
            grid_at(&unread, [1.0, 0.5, 0.2]),
            km_s([-90.0, 140.0, 10.0]),
        ),
        epoch_state(
            grid_at(&unread, [1.2 * edge, 0.5, 0.2]),
            km_s([-90.0, 140.0, 10.0]),
        ),
        epoch_state(grid_at(&unread, [1.0, 0.0, 0.0]), km_s([-200.0, 1.38, 0.0])),
        epoch_state(
            grid_at(&unread, [1.0, 0.5, 0.2]),
            GalacticVelocity::default(),
        ),
        EpochState::of_centre_member(&black_hole),
    ];
    let read = |galaxy: &Galaxy, start: &EpochState| {
        (
            regime_of(galaxy, start.record()),
            loss_cone_verdict(galaxy, start),
            position_velocity_at(galaxy, start, ClockWindow::START),
            position_velocity_at(galaxy, start, ClockWindow::END),
        )
    };
    // Each evaluation on a fresh clone of the unread galaxy reads its motion cold.
    assert_order_independent(&starts, |start| read(&unread.clone(), start));
    let warm = unread.clone();
    let _ = warm.centre_motion();
    for start in &starts {
        assert_eq!(
            read(&warm, start),
            read(&unread.clone(), start),
            "{start:?}"
        );
    }
    assert_eq!(read(&warm, &starts[4]).1, LossConeVerdict::Inside);
}

/// The fixture's centre motion, pinned on every target: its influence radius, the mass inside
/// eight radii and one orbit's state a thousand years on (the lazy exception's pin, run in the
/// fast suites, wasm32-wasip1's included).
#[test]
fn centre_motion_figures_are_pinned() {
    let galaxy = milky_way();
    let motion = galaxy.centre_motion();
    let mut writer = GoldenWriter::new();
    writer.header(GENERATOR_VERSION.get());
    writer.line("# The Milky Way fixture's centre motion (plan 09, P09.T28.b.a).");
    writer.f64("influence_radius_ly", motion.influence_radius().value());
    for (k, r) in [1e-4, 0.0163, 0.3, 3.0, 7.2, 9.9, 10.1, 128.0]
        .into_iter()
        .enumerate()
    {
        let r = LightYears::new(r);
        writer.f64(&format!("r[{k}].ly"), r.value());
        writer.f64(
            &format!("r[{k}].stars_within_msun"),
            motion.stars_within(r).value(),
        );
        writer.f64(
            &format!("r[{k}].enclosed_mass_msun"),
            motion.enclosed_mass(r).value(),
        );
    }
    let start = epoch_state(member_at([0.2, 0.3, -0.1]), km_s([-150.0, 250.0, 330.0]));
    for (k, span) in [-1_000, 1_000].into_iter().enumerate() {
        let StateAt::Exists(state) = position_velocity_at(galaxy, &start, years(span)) else {
            panic!("the pinned orbit misses the loss cone");
        };
        let [x, y, z] = state.position().cell().to_array();
        writer.line(&format!("orbit.t[{k}].cell = [{x}, {y}, {z}]"));
        for (axis, value) in ["x", "y", "z"]
            .into_iter()
            .zip(state.position().offset_metres())
        {
            writer.f64(&format!("orbit.t[{k}].offset_{axis}"), value);
        }
        for (axis, value) in ["vx", "vy", "vz"]
            .into_iter()
            .zip(state.velocity().metres_per_second())
        {
            writer.f64(&format!("orbit.t[{k}].{axis}"), value);
        }
    }
    golden!("galaxy/motion/centre_motion", writer.as_str());
}

/// A layer's name in the golden file.
fn layer_name(layer: Layer) -> &'static str {
    match layer {
        Layer::A => "a",
        Layer::B => "b",
        Layer::C => "c",
        Layer::D => "d",
        Layer::E => "e",
        Layer::BrownDwarf => "brown_dwarf",
        Layer::RoguePlanet => "rogue_planet",
    }
}

/// The galaxies of the pre-pin: the Milky Way fixture and two drawn seeds, each with an edge of a
/// few to ten light-years (a drawn galaxy's reaches 177 ly, whose grid there holds some 10⁸
/// records), with their kinematic tables, so that their grid systems move.
fn pre_pin_galaxies() -> [(&'static str, Galaxy); 3] {
    let fixture = Galaxy::from_params(
        Seed::new(0x0928_b0a0_0000_0001),
        GalaxyParams::milky_way_like(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral");
    [
        ("milky_way_like", fixture),
        (
            "seed 0x5eed0000c0ffee00",
            Galaxy::new(Seed::new(0x5eed_0000_c0ff_ee00)),
        ),
        ("seed 0x2", Galaxy::new(Seed::new(2))),
    ]
    .map(|(label, galaxy)| (label, galaxy.with_full_potential()))
}

/// The radius of the pre-pin's sphere for `layer`: 1.5 influence radii for the stellar layers;
/// for the brown dwarfs and the rogue planets at most 4 ly, the reach of the rogue planets' eight
/// cells about the centre, since within 1.5 influence radii a drawn galaxy holds some 10⁵ brown
/// dwarfs and 10⁷ rogue planets (P09.T28.b.a as built).
fn pre_pin_radius(galaxy: &Galaxy, layer: Layer) -> f64 {
    let edge = 1.5 * galaxy.centre_motion().influence_radius().value();
    match layer {
        Layer::A | Layer::B | Layer::C | Layer::D | Layer::E => edge,
        Layer::BrownDwarf | Layer::RoguePlanet => edge.min(4.0),
    }
}

/// How many records nearest the black hole each layer lists in full.
const LISTED: usize = 4;

/// The pre-pin of decision-galaxy-25-train §1.5 (D1) for P09.T28.b.b's hold: for three galaxies,
/// every grid record within 1.5 influence radii of the black hole (the substellar layers within
/// [`pre_pin_radius`]), its regime, and today's [`position_at`] at ±100 yr and ±1,000 yr, which is
/// ±H. Each layer is pinned by its count, its regimes and its positions as digests over the
/// records in walk order, and its records nearest the black hole are listed in full. It is an
/// extension at the version in force; P09.T28.b.b's hold must leave it unchanged.
///
/// It also checks that [`position_velocity_at`] gives each straight-regime record today's
/// [`position_at`] to the bit, and reports what the Kepler arm would move, for P09.T28.b.b's
/// census.
#[test]
#[ignore = "slow: generates the grid's central cells of every layer for three galaxies, some \
            minutes; P09.T28.b.b runs it by name at its hold"]
fn regime_records_near_the_centre_are_pinned() {
    let mut writer = GoldenWriter::new();
    writer.header(GENERATOR_VERSION.get());
    writer
        .line("# P09.T28.b.a's pre-pin (decision-galaxy-25-train §1.5, D1): grid records near the");
    writer.line(
        "# black hole, their regime and today's position_at at -1000, -100, +100 and +1000 yr.",
    );
    writer.line("# Stellar layers within 1.5 influence radii, substellar within that or 4 ly.");
    writer.line("# Digests in walk order: ids (high, low 32 bits), regimes (1 Kepler), positions");
    writer.line(
        "# (cell x, y, z, offsets x, y, z in metres); then the records nearest the black hole.",
    );
    for (k, (label, galaxy)) in pre_pin_galaxies().iter().enumerate() {
        let edge = galaxy.centre_motion().influence_radius().value();
        writer.line(&format!("# galaxy[{k}]: {label}"));
        writer.f64(&format!("galaxy[{k}].influence_radius_ly"), edge);
        let mut tally = Tally::default();
        for layer in Layer::ALL {
            let label = format!("galaxy[{k}].{}", layer_name(layer));
            pin_layer(&mut writer, &mut tally, galaxy, &label, layer);
        }
        println!(
            "{label}: edge {edge:.3} ly, {} Kepler-regime records; the Kepler arm would carve out \
             {} and move one by at most {:.4} ly",
            tally.kepler, tally.carved, tally.largest
        );
    }
    golden!("galaxy/motion/regime", writer.as_str());
}

/// The times of the pre-pin, Julian years from the epoch: ±100 and ±1,000, which is ±H.
const PRE_PIN_TIMES: [i64; 4] = [-1_000, -100, 100, 1_000];

/// What the Kepler arm would do to a galaxy's pre-pinned records, for P09.T28.b.b's census.
#[derive(Default)]
struct Tally {
    kepler: u64,
    carved: u64,
    largest: f64,
}

/// Every grid record of `layer` within [`pre_pin_radius`] of the black hole, in walk order.
fn records_near(galaxy: &Galaxy, layer: Layer) -> (f64, Vec<SystemRecord>) {
    let radius = pre_pin_radius(galaxy, layer);
    let sphere = QuerySphere::new(
        GalacticPosition::ORIGIN,
        LightYears::new(radius),
        UniverseTime::EPOCH,
        LightYears::ZERO,
    )
    .expect("a sphere at the centre");
    let (mut cell, mut near) = (Vec::new(), Vec::new());
    for key in cells_in_sphere(layer, &sphere) {
        generate_cell(galaxy, key, &mut cell);
        near.extend(
            cell.iter()
                .copied()
                .filter(|r| apart(&GalacticPosition::ORIGIN, r.epoch_position()) <= radius),
        );
    }
    (radius, near)
}

/// One layer's lines of the pre-pin: its count, its regimes and its positions as digests, then
/// its records nearest the black hole in full; and the Kepler arm's moves, into `tally`.
fn pin_layer(
    writer: &mut GoldenWriter,
    tally: &mut Tally,
    galaxy: &Galaxy,
    label: &str,
    layer: Layer,
) {
    let (radius, near) = records_near(galaxy, layer);
    let ids: Vec<f64> = near
        .iter()
        .flat_map(|r| {
            let raw = r.id().raw();
            [
                f64::from(u32::try_from(raw >> 32).expect("high half")),
                f64::from(u32::try_from(raw & 0xffff_ffff).expect("low half")),
            ]
        })
        .collect();
    let regimes: Vec<Regime> = near.iter().map(|r| regime_of(galaxy, r)).collect();
    let kepler = regimes.iter().filter(|&&r| r == Regime::Kepler).count();
    writer.line(&format!(
        "# {label}: {} records, {kepler} in the regime",
        near.len()
    ));
    writer.f64(&format!("{label}.radius_ly"), radius);
    writer.u64_hex(
        &format!("{label}.records"),
        u64::try_from(near.len()).expect("fits"),
    );
    writer.u64_hex(
        &format!("{label}.kepler"),
        u64::try_from(kepler).expect("fits"),
    );
    writer.u64_hex(&format!("{label}.ids"), f64_digest(&ids));
    let flags: Vec<f64> = regimes
        .iter()
        .map(|&r| if r == Regime::Kepler { 1.0 } else { 0.0 })
        .collect();
    writer.u64_hex(&format!("{label}.regimes"), f64_digest(&flags));
    let mut values: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::with_capacity(6 * near.len()));
    for (record, &regime) in near.iter().zip(&regimes) {
        tally_record(tally, galaxy, record, regime, &mut values);
    }
    for (slot, t) in values.iter().zip(PRE_PIN_TIMES) {
        writer.u64_hex(&format!("{label}.t[{t:+}].positions"), f64_digest(slot));
    }
    tally.kepler += u64::try_from(kepler).expect("fits");
    pin_nearest(writer, galaxy, label, &near);
}

/// One record's positions at the pre-pin's times, into `values`; and, for P09.T28.b.b's census,
/// the Kepler arm's verdict and move, into `tally`, with the straight arm held to today's
/// [`position_at`] to the bit.
fn tally_record(
    tally: &mut Tally,
    galaxy: &Galaxy,
    record: &SystemRecord,
    regime: Regime,
    values: &mut [Vec<f64>; 4],
) {
    let start = EpochState::of_grid_record(galaxy, record);
    let verdict = loss_cone_verdict(galaxy, &start);
    if verdict == LossConeVerdict::Inside {
        tally.carved += 1;
    }
    for (slot, t) in values.iter_mut().zip(PRE_PIN_TIMES) {
        let at = position_at(galaxy, record, years(t));
        slot.extend(at.cell().to_array().map(f64::from));
        slot.extend(at.offset_metres());
        match (regime, position_velocity_at(galaxy, &start, years(t))) {
            (Regime::Straight, StateAt::Exists(state)) => assert_same_state(
                (*state.position(), state.velocity()),
                (at, start.velocity()),
            ),
            (Regime::Kepler, StateAt::Exists(state)) => {
                tally.largest = tally.largest.max(apart(state.position(), &at));
            }
            (Regime::Kepler, StateAt::NoSuchSystem) => {
                assert_eq!(verdict, LossConeVerdict::Inside);
            }
            (Regime::Straight, StateAt::NoSuchSystem) => {
                panic!("{record:?} outside the regime was carved out");
            }
        }
    }
}

/// The [`LISTED`] records of `near` nearest the black hole, in full.
fn pin_nearest(writer: &mut GoldenWriter, galaxy: &Galaxy, label: &str, near: &[SystemRecord]) {
    let mut nearest: Vec<&SystemRecord> = near.iter().collect();
    nearest.sort_by(|a, b| {
        apart(&GalacticPosition::ORIGIN, a.epoch_position())
            .total_cmp(&apart(&GalacticPosition::ORIGIN, b.epoch_position()))
            .then(a.id().raw().cmp(&b.id().raw()))
    });
    for (n, record) in nearest.into_iter().take(LISTED).enumerate() {
        let at = format!("{label}.nearest[{n}]");
        writer.u64_hex(&format!("{at}.id"), record.id().raw());
        writer.line(&format!(
            "{at}.regime = {}",
            regime_of(galaxy, record).name()
        ));
        writer.f64(
            &format!("{at}.epoch_distance_ly"),
            apart(&GalacticPosition::ORIGIN, record.epoch_position()),
        );
        for t in PRE_PIN_TIMES {
            let p = position_at(galaxy, record, years(t));
            let [x, y, z] = p.cell().to_array();
            writer.line(&format!("{at}.t[{t:+}].cell = [{x}, {y}, {z}]"));
            for (axis, value) in ["x", "y", "z"].into_iter().zip(p.offset_metres()) {
                writer.f64(&format!("{at}.t[{t:+}].offset_{axis}"), value);
            }
        }
    }
}
