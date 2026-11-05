//! The golden vectors of one sightline (plan 12, P12.T13; the determinism audit's ask in plan 12's
//! records, "For P12.T13"): its lens events (P12.T4), one segment walk's keys
//! (`cells_along_segment`) and a few bearings (P12.T2), whose angles pass through `math::atan2`.
//!
//! The sightline runs from the Sun-like point to the first system of a layer-C cell 5,000 ly
//! towards the anticentre and 100 ly above the plane, the disc bench's (`lens_walk_disc_5kly`), in
//! the Milky Way fixture with its full potential, whose systems move, so that the walk is the cone
//! of Design note 13 as built. A point lens there magnifies by about 10⁻⁹ in a century, so the
//! query reaches 30,000 Einstein radii to find a few lenses whose fields the golden pins: each
//! lens, its peak and the least impact within the century, its Einstein radius and time, the
//! magnification at the peak, the unclamped closest approach, and its bearing from the observer;
//! and the walk's census. The segment is the sightline's in layer C with a 1 ly tube. The bearings
//! are from the Sun-like point to 100 ly along each named direction and two oblique points, to the
//! centre, and from a point within a light-year of the z axis, whose frame is `galactic_x`.

use hyperion_sim::coords::{GalacticDisplacement, GalacticPosition, UnitVector};
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, NoCache, generate_cell, resolve};
use hyperion_sim::galaxy::query::{cells_along_segment, position_at};
use hyperion_sim::id::Layer;
use hyperion_sim::lensing::{LensQuery, LensSightline, lenses_along};
use hyperion_sim::observe::{Bearing, bearing};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::LightYears;
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The universe of the golden: the Milky Way fixture at this seed.
const SEED: u64 = 0x1213_0000_0000_0003;

/// The observer: the Sun-like point, ly.
const SUN: [f64; 3] = [0.0, 26_000.0, 20.0];

/// The source's layer-C cell: 5,000 ly towards the anticentre, 100 ly above the plane.
const SOURCE_CELL: [i32; 3] = [0, 968, 3];

/// The query's reach, in Einstein radii (module documentation).
const MAX_IMPACT: f64 = 30_000.0;

/// The window's length, Julian years from the epoch.
const CENTURY: i64 = 100;

fn write_bearing(w: &mut GoldenWriter, label: &str, b: &Bearing) {
    w.line(&format!("{label}.frame = {:?}", b.frame()));
    w.f64(&format!("{label}.azimuth_deg"), b.azimuth().value());
    w.f64(&format!("{label}.elevation_deg"), b.elevation().value());
}

/// `from` moved `ly` light-years along `direction`.
fn along(from: &GalacticPosition, direction: [f64; 3], ly: f64) -> GalacticPosition {
    from.translated(GalacticDisplacement::new(
        direction.map(|c| c * ly * METRES_PER_LIGHT_YEAR),
    ))
    .expect("inside the addressable range")
}

/// P12.T13: one sightline's lens events, its segment's keys and a few bearings.
#[test]
fn a_sightline_is_the_golden_vectors() {
    let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
        .with_full_potential();
    let sun = GalacticPosition::from_light_years(SUN).expect("inside the root cube");
    let mut cell = Vec::new();
    generate_cell(
        &galaxy,
        CellKey::new(Layer::C, SOURCE_CELL).expect("a cell of the disc"),
        &mut cell,
    );
    let source = *cell
        .first()
        .expect("a layer-C cell of the disc holds a system");
    let end = UniverseTime::from_julian_years(CENTURY).expect("on the clock");
    let window = TimeWindow::new(UniverseTime::EPOCH, end).expect("a window forward");
    let query = LensQuery::builder(LensSightline::new(sun, source.id()), window)
        .max_impact(MAX_IMPACT)
        .build()
        .expect("a valid query");
    let lenses = lenses_along(&galaxy, &mut NoCache::new(), &[], &query).expect("a grid source");

    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line(&format!("source = {}", source.id()));
    let walked = lenses.walked();
    w.line(&format!(
        "walked {} cells, {} systems, {} spheres, layers {:?}",
        walked.cells(),
        walked.systems_examined(),
        walked.spheres(),
        walked.layers()
    ));
    w.line(&format!("{} lens events", lenses.events().len()));
    for (i, e) in lenses.events().iter().enumerate() {
        w.line(&format!("lens{i} = {}", e.lens()));
        w.line(&format!(
            "lens{i}.peak = {}s {}ns",
            e.peak().seconds(),
            e.peak().subsec_nanos()
        ));
        w.f64(&format!("lens{i}.impact"), e.impact());
        w.f64(
            &format!("lens{i}.einstein_radius_m"),
            e.einstein_radius().value(),
        );
        w.f64(
            &format!("lens{i}.einstein_time_s"),
            e.einstein_time().value(),
        );
        w.f64(
            &format!("lens{i}.peak_magnification"),
            e.peak_magnification(),
        );
        w.f64(&format!("lens{i}.closest_s"), e.closest_seconds());
        w.f64(&format!("lens{i}.closest_impact"), e.closest_impact());
        let lens = resolve(&galaxy, e.lens()).expect("a lens of the grid");
        let towards = bearing(&sun, &position_at(&galaxy, &lens, e.peak())).expect("two points");
        write_bearing(&mut w, &format!("lens{i}.bearing"), &towards);
    }

    let to = position_at(&galaxy, &source, UniverseTime::EPOCH);
    let keys: Vec<CellKey> =
        cells_along_segment(Layer::C, &sun, &to, LightYears::new(1.0)).collect();
    w.line(&format!("segment: {} layer-C keys", keys.len()));
    for key in &keys {
        let [x, y, z] = key.gen_cell().to_array();
        w.line(&format!("key [{x}, {y}, {z}]"));
    }

    let named = sun.directions().expect("off the axis");
    let directions: [(&str, UnitVector); 6] = [
        ("coreward", named.coreward()),
        ("spinward", named.spinward()),
        ("rimward", named.rimward()),
        ("antispinward", named.antispinward()),
        ("north", named.north()),
        ("south", named.south()),
    ];
    for (name, direction) in directions {
        let towards =
            bearing(&sun, &along(&sun, direction.components(), 100.0)).expect("two points");
        write_bearing(&mut w, &format!("bearing.{name}"), &towards);
    }
    let oblique = [
        ("up_spinward", [0.6, -0.48, 0.64]),
        ("down_rimward", [-0.3, 0.9, -0.316_227_766_016_837_94]),
    ];
    for (name, direction) in oblique {
        let towards = bearing(&sun, &along(&sun, direction, 3_000.0)).expect("two points");
        write_bearing(&mut w, &format!("bearing.{name}"), &towards);
    }
    let centre = bearing(&sun, &GalacticPosition::ORIGIN).expect("two points");
    write_bearing(&mut w, "bearing.centre", &centre);
    let by_axis = GalacticPosition::from_light_years([0.4, -0.3, 120.0]).expect("in range");
    let from_axis = bearing(&by_axis, &sun).expect("two points");
    write_bearing(&mut w, "bearing.from_axis", &from_axis);
    golden!("observe/sightline", w.as_str());
}
