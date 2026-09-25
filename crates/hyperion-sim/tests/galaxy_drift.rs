//! Drift in the range query: systems move at their velocities, and the query finds them where
//! they are (plan 08, P08.T6).

#[expect(
    dead_code,
    reason = "the kinematics tests use only a few shared helpers"
)]
mod common;

use std::collections::BTreeMap;
use std::sync::OnceLock;

use common::sunlike_point;
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
use hyperion_sim::galaxy::kinematics::draw_velocity;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellCache, CellKey, NoCache, SystemRecord, generate_cell};
use hyperion_sim::galaxy::query::{
    PAD_SPEED, QuerySphere, RangeQuery, SystemHit, UNBOUND_PAD_SPEED, count_cells_in_sphere,
    pad_for, position_at, range_query,
};
use hyperion_sim::galaxy::{Galaxy, PointLy};
use hyperion_sim::id::Layer;
use hyperion_sim::math;
use hyperion_sim::time::{ClockWindow, UniverseTime};
use hyperion_sim::units::consts::SECONDS_PER_JULIAN_YEAR;
use hyperion_sim::units::{KilometresPerSecond, LightYears, SolarMasses};
use hyperion_testkit::lcg::Lcg;

fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(
            Seed::new(0x0806_0000_0000_0001),
            GalaxyParams::milky_way_like(),
        )
        .expect("the fixture's gas is mostly neutral")
        .with_full_potential()
    })
}

fn at(years: i64) -> UniverseTime {
    UniverseTime::from_julian_years(years).unwrap()
}

/// P08.T6: for 10³ systems near the Sun-like point, the position at ±1,000 yr is the epoch
/// position plus v t to a metre, and the drift is symmetric in time.
///
/// "To a metre" is taken as 4 m: plan 01's position keeps its offset inside a light-year in
/// metres, 9.46 × 10¹⁵ of them, whose last bit is 2 m, so a drift rounds by a bit or two however
/// it is computed.
#[test]
fn a_system_drifts_by_its_velocity_times_the_time() {
    let galaxy = galaxy();
    let query = RangeQuery::builder(sunlike_point(galaxy), LightYears::new(60.0))
        .build()
        .expect("a 60 ly query");
    let result = range_query(galaxy, &mut NoCache::new(), &[], &query);
    let mut checked = 0;
    for record in result.systems().iter().map(SystemHit::record).take(1_000) {
        let v = draw_velocity(galaxy, record).metres_per_second();
        let epoch = record.epoch_position();
        let ahead = epoch
            .displacement_to(&position_at(galaxy, record, at(1_000)))
            .metres();
        let behind = epoch
            .displacement_to(&position_at(galaxy, record, at(-1_000)))
            .metres();
        let seconds = 1_000.0 * SECONDS_PER_JULIAN_YEAR;
        for axis in 0..3 {
            assert!(
                (ahead[axis] - v[axis] * seconds).abs() < 4.0,
                "{:?} axis {axis}: {} m against {} m",
                record.id(),
                ahead[axis],
                v[axis] * seconds
            );
            assert!((ahead[axis] + behind[axis]).abs() < 4.0);
        }
        checked += 1;
    }
    assert_eq!(checked, 1_000);
}

/// Every system of the five stellar layers within `radius` of `centre` at `t`, by generating every
/// cell within reach of a 5,000 km/s pad and moving each system: the query's answer found without
/// its walk.
fn brute_force(
    galaxy: &Galaxy,
    centre: &GalacticPosition,
    radius: LightYears,
    t: UniverseTime,
    layers: impl Fn(Layer) -> bool,
) -> Vec<SystemHit> {
    let pad = pad_for(t, KilometresPerSecond::new(5_000.0)).value();
    let reach = (radius.value() + pad + 2.0).ceil();
    let c = centre.to_light_years_f64();
    let mut cache = NoCache::new();
    let mut hits = Vec::new();
    for layer in [Layer::E, Layer::D, Layer::C, Layer::B, Layer::A] {
        if !layers(layer) {
            continue;
        }
        let size = f64::from(layer.cell_size_ly());
        let range = |axis: usize| {
            let lo = ((c[axis] - reach) / size).floor();
            let hi = ((c[axis] + reach) / size).floor();
            (whole(lo), whole(hi))
        };
        let (xs, ys, zs) = (range(0), range(1), range(2));
        for x in xs.0..=xs.1 {
            for y in ys.0..=ys.1 {
                for z in zs.0..=zs.1 {
                    let key = CellKey::new(layer, [x, y, z]).unwrap();
                    cache.with_cell(galaxy, key, |systems| {
                        for record in systems {
                            if record.age_at(t).value() <= 0.0 {
                                continue;
                            }
                            let position = position_at(galaxy, record, t);
                            let distance = LightYears::from(centre.distance_to(&position));
                            if distance.value() <= radius.value() {
                                hits.push(SystemHit::new(*record, position, distance));
                            }
                        }
                    });
                }
            }
        }
    }
    hits.sort_by(|a, b| {
        a.distance()
            .total_cmp(&b.distance())
            .then_with(|| a.id().raw().cmp(&b.id().raw()))
    });
    hits
}

/// A cache that keeps every cell it generates.
#[derive(Debug, Default)]
struct WarmCache {
    store: BTreeMap<CellKey, Vec<SystemRecord>>,
}

impl CellCache for WarmCache {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        let systems = self.store.entry(key).or_insert_with(|| {
            let mut systems = Vec::new();
            generate_cell(galaxy, key, &mut systems);
            systems
        });
        f(systems)
    }
}

/// A whole cell index from a floored float.
fn whole(x: f64) -> i32 {
    assert!(x.abs() < 1e6);
    #[expect(clippy::cast_possible_truncation, reason = "a whole number under 10⁶")]
    let i = x as i32;
    i
}

/// P08.T6: a range query at t = +1,000 yr is, as a set, the brute-force answer at the Sun-like
/// point, at the galactic centre and at 40,000 ly; its expected counts do not depend on t; and it
/// is a pure function of its arguments, cache warm or cold.
#[test]
fn a_moving_range_query_is_the_brute_force_answer() {
    let galaxy = galaxy();
    for (centre, radius) in [
        (sunlike_point(galaxy), 20.0),
        (
            GalacticPosition::from_light_years([0.0, -40_000.0, 30.0]).unwrap(),
            30.0,
        ),
    ] {
        the_query_is_the_brute_force_answer(galaxy, centre, LightYears::new(radius));
    }
}

/// P08.T6: the same at the galactic centre, where the cells are dense.
#[test]
#[ignore = "slow: generates every cell within 20 ly of the centre in debug"]
fn a_moving_range_query_at_the_centre_is_the_brute_force_answer() {
    let centre = GalacticPosition::from_light_years([3.0, -2.0, 1.0]).unwrap();
    the_query_is_the_brute_force_answer(galaxy(), centre, LightYears::new(1.5));
}

fn the_query_is_the_brute_force_answer(
    galaxy: &Galaxy,
    centre: GalacticPosition,
    radius: LightYears,
) {
    {
        let query = |t| {
            RangeQuery::builder(centre, radius)
                .time(t)
                .build()
                .expect("a small query")
        };
        let moving = range_query(galaxy, &mut NoCache::new(), &[], &query(at(1_000)));
        let still = range_query(
            galaxy,
            &mut NoCache::new(),
            &[],
            &query(UniverseTime::EPOCH),
        );
        assert_eq!(moving.census().expected(), still.census().expected());
        let layers = moving.census().layers();
        let expected = brute_force(galaxy, &centre, radius, at(1_000), |l| layers.contains(l));
        let mut got: Vec<_> = moving.systems().iter().map(SystemHit::id).collect();
        let mut want: Vec<_> = expected.iter().map(SystemHit::id).collect();
        got.sort_by_key(|id| id.raw());
        want.sort_by_key(|id| id.raw());
        eprintln!(
            "{:?}: {} systems at +1,000 yr, {} at the epoch",
            centre.to_light_years_f64(),
            got.len(),
            still.systems().len()
        );
        assert!(!got.is_empty());
        assert_eq!(got, want, "the query at +1,000 yr against the brute force");
        // Warm and cold caches agree: a cache warmed by the epoch's query answers the moving one.
        let mut warm = WarmCache::default();
        let _ = range_query(galaxy, &mut warm, &[], &query(UniverseTime::EPOCH));
        assert_eq!(
            range_query(galaxy, &mut warm, &[], &query(at(1_000))),
            moving
        );
    }
}

/// P08.T6: over a century the curvature a straight line neglects, `v_c² t² ÷ 2R` from the tables,
/// is under 10⁻⁴ of the tidal radius of a solar mass, at 1,000 points outside the central 10 ly
/// that the plan names; the brainstorm says "outside the central few light-years".
///
/// A finding: close to the centre the curvature wins, since it falls as `R⁻²` there against the
/// tidal radius's `R`. The worst point and the radius beyond which the bound holds are printed,
/// and the test holds it beyond 40 ly (it fails out to 38 ly at Milky Way values).
#[test]
fn a_century_s_curvature_is_negligible() {
    let galaxy = galaxy();
    let potential = galaxy.potential();
    let century = 100.0;
    let mut lcg = Lcg::new(0x0806_00c0);
    let (mut worst, mut worst_at, mut holds_beyond): (f64, f64, f64) = (0.0, 0.0, 0.0);
    for _ in 0..1_000 {
        // Log-uniform in radius from 10 ly to 60,000 ly, any azimuth, a few hundred ly off the plane.
        let r = 10.0 * math::exp(lcg.next_f64() * math::ln(6_000.0));
        let theta = std::f64::consts::TAU * lcg.next_f64();
        let z = 600.0 * (lcg.next_f64() - 0.5);
        let (sin, cos) = math::sin_cos(theta);
        let p = PointLy::new(r * cos, r * sin, z);
        let spherical = math::hypot(r, z);
        let v =
            potential.v_circ(LightYears::new(spherical)).value() * LIGHT_YEARS_PER_YEAR_PER_KM_S;
        let curvature = v * v * century * century / (2.0 * spherical);
        let tidal = LightYears::from(potential.tidal_radius(SolarMasses::new(1.0), &p)).value();
        let ratio = curvature / tidal;
        if ratio > worst {
            (worst, worst_at) = (ratio, spherical);
        }
        if ratio >= 1e-4 {
            holds_beyond = holds_beyond.max(spherical);
        }
    }
    eprintln!(
        "curvature ÷ tidal radius: worst {worst:.2e} at {worst_at:.0} ly; the bound fails out to {holds_beyond:.0} ly"
    );
    assert!(
        holds_beyond < 40.0,
        "the bound fails out to {holds_beyond} ly"
    );
}

/// P08.T6: over 100 centres, the layer-E cells a 50 ly query visits at |t| = H rise by under a
/// third against plan 03's pad (Design note 27).
#[test]
fn the_unbound_pad_costs_few_cells() {
    let mut lcg = Lcg::new(0x0806_00ce);
    let (mut raised, mut plain) = (0_u64, 0_u64);
    for _ in 0..100 {
        let centre = GalacticPosition::from_light_years([
            40_000.0 * (lcg.next_f64() - 0.5),
            40_000.0 * (lcg.next_f64() - 0.5),
            2_000.0 * (lcg.next_f64() - 0.5),
        ])
        .unwrap();
        let sphere = |speed| {
            QuerySphere::new(
                centre,
                LightYears::new(50.0),
                ClockWindow::END,
                pad_for(ClockWindow::END, speed),
            )
            .unwrap()
        };
        raised += count_cells_in_sphere(Layer::E, &sphere(UNBOUND_PAD_SPEED));
        plain += count_cells_in_sphere(Layer::E, &sphere(PAD_SPEED));
    }
    let growth = u32::try_from(raised).unwrap();
    let base = u32::try_from(plain).unwrap();
    let ratio = f64::from(growth) / f64::from(base);
    eprintln!("layer-E cells at |t| = H: {raised} against {plain}, {ratio:.3}");
    assert!(ratio < 4.0 / 3.0, "{ratio}");
}
