//! The velocity draw and the escape cut (plan 08, P08.T5).

#[expect(
    dead_code,
    reason = "the kinematics tests use only a few shared helpers"
)]
mod common;

use std::collections::BTreeMap;
use std::sync::OnceLock;

use common::sunlike_point;
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::Seed;
use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
use hyperion_sim::galaxy::kinematics::{ESCAPE_CUT_ATTEMPTS, EllipsoidAxes, draw, draw_velocity};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{
    CellKey, NoCache, SystemOrigin, SystemRecord, generate_cell, resolve,
};
use hyperion_sim::galaxy::query::{PAD_SPEED, RangeQuery, epoch_velocity, range_query};
use hyperion_sim::galaxy::{Galaxy, PointLy};
use hyperion_sim::id::Layer;
use hyperion_sim::math;
use hyperion_sim::units::{LightYears, SolarMasses, Years};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;

fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(
            Seed::new(0x0805_0000_0000_0001),
            GalaxyParams::milky_way_like(),
        )
        .expect("the fixture's gas is mostly neutral")
        .with_full_potential()
    })
}

/// The cells the pinned systems are found in, in order: the disc at the Sun's radius, the centre,
/// and the halo above the disc.
const CELLS: [(Layer, [i32; 3]); 8] = [
    (Layer::E, [0, 203, 0]),
    (Layer::E, [1, 203, -1]),
    (Layer::D, [0, 406, 0]),
    (Layer::E, [0, 0, 0]),
    (Layer::E, [-1, -1, 0]),
    (Layer::E, [40, 3, 1]),
    (Layer::E, [0, 150, 90]),
    (Layer::E, [30, -120, -70]),
];

/// Twelve systems across the populations: the first of each component in [`CELLS`]' order, for
/// the young disc, the five sub-discs, the thick disc, the bulge, the bar, the nuclear disc and
/// the halo's first two components.
fn pinned(galaxy: &Galaxy) -> Vec<SystemRecord> {
    let mut first: BTreeMap<usize, SystemRecord> = BTreeMap::new();
    let mut cell = Vec::new();
    for (layer, at) in CELLS {
        cell.clear();
        generate_cell(galaxy, CellKey::new(layer, at).unwrap(), &mut cell);
        for record in &cell {
            let c = record.component().expect("a grid record has a component");
            first.entry(c.index()).or_insert(*record);
        }
    }
    let wanted = 12;
    let records: Vec<_> = first.into_values().take(wanted).collect();
    assert_eq!(records.len(), wanted, "every population found in the cells");
    records
}

/// P08.T5: the golden velocities of twelve pinned IDs across the populations.
#[test]
fn the_pinned_velocities_are_golden() {
    let galaxy = galaxy();
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (k, record) in pinned(galaxy).iter().enumerate() {
        let label = format!("system.{k:02}");
        let d = draw(galaxy, record);
        w.u64_hex(&format!("{label}.id"), record.id().raw());
        w.line(&format!(
            "{label}.population = {}",
            record.population().name()
        ));
        let p = PointLy::from(record.epoch_position());
        w.f64(&format!("{label}.x_ly"), p.x);
        w.f64(&format!("{label}.y_ly"), p.y);
        w.f64(&format!("{label}.z_ly"), p.z);
        let [vx, vy, vz] = d.velocity().metres_per_second();
        w.f64(&format!("{label}.vx_m_s"), vx);
        w.f64(&format!("{label}.vy_m_s"), vy);
        w.f64(&format!("{label}.vz_m_s"), vz);
        w.line(&format!("{label}.attempts = {}", d.attempts()));
    }
    golden!("galaxy_velocity", w.as_str());
}

/// P08.T5: a velocity is a pure function of the galaxy and the record, whatever order the records
/// are drawn in; it is the same through `resolve` as through the cell; and the range query's
/// `epoch_velocity` is it.
#[test]
fn a_velocity_does_not_depend_on_how_its_system_is_reached() {
    let galaxy = galaxy();
    let records = pinned(galaxy);
    assert_order_independent(&records, |r| draw_velocity(galaxy, r));
    for record in &records {
        let resolved = resolve(galaxy, record.id()).expect("a placed system resolves");
        assert_eq!(&resolved, record);
        assert_eq!(
            draw_velocity(galaxy, &resolved),
            draw_velocity(galaxy, record)
        );
        assert_eq!(
            epoch_velocity(galaxy, record),
            draw_velocity(galaxy, record)
        );
    }
    // A galaxy built without its kinematic tables has no velocities.
    let plain = Galaxy::from_params(galaxy.seed(), galaxy.params().clone()).unwrap();
    assert_eq!(
        epoch_velocity(&plain, &records[0]),
        GalacticVelocity::default()
    );
}

/// P08.T5: at the Sun-like point the escape cut redraws under one draw in a hundred.
#[test]
fn the_cut_rarely_redraws_near_the_sun() {
    let galaxy = galaxy();
    let (mut attempts, mut systems) = (0_u32, 0_u32);
    let query = RangeQuery::builder(sunlike_point(galaxy), LightYears::new(50.0))
        .build()
        .expect("a 50 ly query");
    for hit in range_query(galaxy, &mut NoCache::new(), &[], &query).systems() {
        attempts += draw(galaxy, hit.record()).attempts();
        systems += 1;
    }
    let mean = f64::from(attempts) / f64::from(systems);
    eprintln!("{systems} systems near the Sun, {mean:.5} draws each");
    assert!(systems > 500, "{systems}");
    assert!(mean < 1.01, "{mean}");
}

/// A record of component `index` at `p` with the candidate ID `n` of a layer-C cell, for sampling
/// one component's law at one point.
fn synthetic(galaxy: &Galaxy, index: usize, p: [f64; 3], n: u32) -> SystemRecord {
    let key = CellKey::new(Layer::C, [0, 812, 0]).unwrap();
    let component = galaxy.fields().component_id(index).unwrap();
    SystemRecord::from_parts(
        key.candidate_id(n).unwrap(),
        GalacticPosition::from_light_years(p).unwrap(),
        SystemOrigin::Grid(component),
        galaxy.fields().component(component).population(),
        SolarMasses::new(1.0),
        Years::new(1e9),
    )
}

/// P08.T5: for every component, 10⁵ velocities drawn at one point match the ellipsoid's mean and
/// dispersions along its axes, within normal-theory intervals at 4σ, wherever the cut removes
/// under 10⁻³ of the draws.
#[test]
#[ignore = "slow: 10⁵ draws for each of 16 components"]
fn sampled_velocities_match_their_ellipsoids() {
    let galaxy = galaxy();
    let tables = galaxy.kinematics().unwrap();
    let points = [
        [0.0, 26_000.0, 50.0],
        [0.0, 1_500.0, 300.0],
        [3_000.0, 26_000.0, 9_000.0],
    ];
    let draws = 100_000_u32;
    for index in 0..galaxy.fields().components().len() {
        for p in points {
            let point = PointLy::new(p[0], p[1], p[2]);
            let id = galaxy.fields().component_id(index).unwrap();
            if galaxy.fields().component(id).density(&point) <= 0.0 {
                continue;
            }
            let ellipsoid = tables.ellipsoid(id, &point);
            let basis = axes(ellipsoid.axes(), &point);
            let (mut sum, mut sum_sq, mut redrawn) = ([0.0; 3], [0.0; 3], 0_u32);
            for k in 0..draws {
                let d = draw(galaxy, &synthetic(galaxy, index, p, k));
                if d.attempts() > 1 {
                    redrawn += 1;
                }
                let v = d.velocity().metres_per_second().map(|c| c / 1e3);
                for (axis, unit) in basis.iter().enumerate() {
                    let c = v[0] * unit[0] + v[1] * unit[1] + v[2] * unit[2];
                    sum[axis] += c;
                    sum_sq[axis] += c * c;
                }
            }
            if f64::from(redrawn) >= 1e-3 * f64::from(draws) {
                eprintln!(
                    "component {index} at {p:?}: the cut redraws {redrawn} of {draws}, skipped"
                );
                continue;
            }
            let count = f64::from(draws);
            for axis in 0..3 {
                let mean = sum[axis] / count;
                let var = sum_sq[axis] / count - mean * mean;
                let (mu, sigma) = (
                    ellipsoid.mean()[axis].value(),
                    ellipsoid.sigma()[axis].value(),
                );
                let mean_error = sigma / count.sqrt();
                assert!(
                    (mean - mu).abs() <= 4.0 * mean_error + 1e-9,
                    "component {index} at {p:?}, axis {axis}: mean {mean} against {mu}"
                );
                let var_error = sigma * sigma * (2.0 / count).sqrt();
                assert!(
                    (var - sigma * sigma).abs() <= 4.0 * var_error + 1e-9,
                    "component {index} at {p:?}, axis {axis}: σ² {var} against {}",
                    sigma * sigma
                );
            }
        }
    }
}

/// The ellipsoid's unit vectors at `p`, in galactic components.
fn axes(axes: EllipsoidAxes, p: &PointLy) -> [[f64; 3]; 3] {
    let r_cyl = math::hypot(p.x, p.y);
    let (c, s) = (p.x / r_cyl, p.y / r_cyl);
    let (rimward, spinward) = ([c, s, 0.0], [-s, c, 0.0]);
    match axes {
        EllipsoidAxes::Cylindrical => [rimward, spinward, [0.0, 0.0, 1.0]],
        EllipsoidAxes::Spherical => {
            let r = math::hypot(r_cyl, p.z);
            let (st, ct) = (r_cyl / r, p.z / r);
            [[st * c, st * s, ct], [ct * c, ct * s, -st], spinward]
        }
    }
}

/// P08.T5: no velocity reaches its padding speed, over 10⁶ systems of the cells about the centre,
/// the central 100 ly included, where the escape speed passes 1,000 km/s.
#[test]
#[ignore = "slow: draws the velocities of 10⁶ systems near the centre"]
fn no_velocity_reaches_its_padding_speed() {
    let galaxy = galaxy();
    let mut cell = Vec::new();
    let (mut systems, mut fastest, mut capped) = (0_u64, 0.0_f64, 0_u64);
    'walk: for layer in [Layer::E, Layer::D, Layer::C, Layer::B, Layer::A] {
        let size = i32::try_from(layer.cell_size_ly()).unwrap();
        let reach = 256 / size;
        for x in -reach..reach {
            for y in -reach..reach {
                for z in -reach..reach {
                    cell.clear();
                    generate_cell(galaxy, CellKey::new(layer, [x, y, z]).unwrap(), &mut cell);
                    for record in &cell {
                        let d = draw(galaxy, record);
                        let speed = d.velocity().speed().value() / 1e3;
                        assert!(
                            speed < PAD_SPEED.value(),
                            "{speed} km/s for {:?}",
                            record.id()
                        );
                        assert!(speed < d.cut().value());
                        fastest = fastest.max(speed);
                        if d.attempts() == ESCAPE_CUT_ATTEMPTS {
                            capped += 1;
                        }
                        systems += 1;
                        if systems >= 1_000_000 {
                            break 'walk;
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "{systems} systems within 256 ly of the centre: fastest {fastest:.1} km/s, {capped} scaled"
    );
    assert!(systems >= 1_000_000, "{systems}");
}
