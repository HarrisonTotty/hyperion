//! The golden vectors of retarded observation (plan 12, P12.T13): what two observers see of six
//! pinned systems, read at the retarded time (P12.T1–T3).
//!
//! The galaxy is the Milky Way fixture with its full potential, as the server builds it, so that
//! each system moves on its own line. Each system is the one a rule picks from the records about
//! a place at the epoch, so that a version that redraws the grid re-blesses the file rather than
//! losing a pin:
//!
//! - `neighbour`: the nearest layer-A system to the Sun-like point;
//! - `massive`: the layer-E system with the heaviest primary about it;
//! - `member`: the member nearest the centre of the first feature with members that plan 09's
//!   catalogue finds within 3,000 ly of the Sun, moving at its own velocity in its feature's
//!   interior;
//! - `nuclear_disc`: the nearest nuclear-disc system of layer A to a point 300 ly from the centre
//!   in the plane, whose stated curvature error is the brainstorm's largest (Design note 3);
//! - `halo`: the nearest halo system of layer E to a point 18,000 ly above the plane;
//! - `far_disc`: the nearest layer-A system to a point on the far side of the disc, 61,000 ly from
//!   the Sun.
//!
//! The observers are `sun`, at the Sun-like point at the epoch, and `far`, 88,000 ly from it on the
//! far side of the disc, 700 years later. For each system and observer the golden holds the
//! system's present on its own line and its distance, the emitted time and the light's age, the
//! apparent position and the velocity then, the existence and the brief then, the stated curvature
//! error, the gap between the extrapolated present and the system's present (Design note 1: under
//! a metre), and the bearing of the apparent position, whose angles pass through `math::atan2`.

use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, NoFeatureCache};
use hyperion_sim::galaxy::features::members::{
    FeatureInterior, FeatureMemberSource, KeepInteriors,
};
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use hyperion_sim::galaxy::query::{SystemHit, SystemSource};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::Layer;
use hyperion_sim::observe::{
    Drift, ObservedSystem, Observer, Trajectory, bearing, extrapolate_to_present, observe_hit_in,
    stars_of,
};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::LightYears;
use hyperion_sim::version::GENERATOR_VERSION;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The universe of the golden: the Milky Way fixture at this seed.
const SEED: u64 = 0x1213_0000_0000_0001;

/// The Sun-like point, ly.
const SUN: [f64; 3] = [0.0, 26_000.0, 20.0];

/// The far observer's place, ly: on the far side of the disc, 6,000 ly above the plane.
const FAR: [f64; 3] = [-48_000.0, -40_000.0, 6_000.0];

/// The far observer's time, Julian years after the epoch.
const FAR_YEARS: i64 = 700;

/// How a pinned system is picked from the records about its place.
#[derive(Debug, Clone, Copy)]
enum Pick {
    /// The nearest.
    Nearest,
    /// The one with the heaviest primary, the nearer of two as heavy.
    Heaviest,
    /// The nearest of the population.
    Population(Population),
}

/// A grid rule: the layer whose cells are read, the place they are about, ly, and the pick.
type GridRule = (Layer, [f64; 3], Pick);

fn at(ly: [f64; 3]) -> GalacticPosition {
    GalacticPosition::from_light_years(ly).expect("inside the root cube")
}

/// The grid record `pick` picks from `layer`'s cell holding `place` and its 26 neighbours, of
/// those the feature members do not suppress at the epoch: cells rather than a range query, whose
/// cold walk would cost seconds a search.
fn grid_pick(
    galaxy: &Galaxy,
    source: &FeatureMemberSource<'_>,
    layer: Layer,
    place: &GalacticPosition,
    pick: Pick,
) -> Option<SystemRecord> {
    let [cx, cy, cz] = CellKey::containing(layer, place)
        .expect("a place in the root cube")
        .gen_cell()
        .to_array();
    let mut records: Vec<SystemRecord> = Vec::new();
    let mut cell = Vec::new();
    for x in cx - 1..=cx + 1 {
        for y in cy - 1..=cy + 1 {
            for z in cz - 1..=cz + 1 {
                let Ok(key) = CellKey::new(layer, [x, y, z]) else {
                    continue;
                };
                generate_cell(galaxy, key, &mut cell);
                records.extend(
                    cell.iter()
                        .filter(|r| !source.suppresses(galaxy, r, UniverseTime::EPOCH)),
                );
            }
        }
    }
    let distance = |r: &SystemRecord| r.epoch_position().distance_to(place).value();
    let admitted = records.into_iter().filter(|r| match pick {
        Pick::Nearest | Pick::Heaviest => true,
        Pick::Population(population) => r.population() == population,
    });
    match pick {
        Pick::Heaviest => admitted.min_by(|a, b| {
            let mass = |r: &SystemRecord| r.primary_initial_mass().value();
            mass(b)
                .total_cmp(&mass(a))
                .then(distance(a).total_cmp(&distance(b)))
        }),
        Pick::Nearest | Pick::Population(_) => {
            admitted.min_by(|a, b| distance(a).total_cmp(&distance(b)))
        }
    }
}

/// The member nearest its feature's centre, the first of two as near: read from the interior's
/// cells, not through the member source, whose lookup reads the lists of every feature for
/// thousands of light-years about (P09.T37), some 8 s.
fn member_pick(galaxy: &Galaxy, interior: &FeatureInterior) -> Option<SystemRecord> {
    let mut members = Vec::new();
    let mut nearest: Option<(f64, SystemRecord)> = None;
    for cell in interior.grid().owned_cells() {
        for band in MassBand::ALL {
            interior.members_in_cell(galaxy, band, cell, &mut members);
            for (member, at) in &members {
                let r2 = at.x * at.x + at.y * at.y + at.z * at.z;
                if nearest.is_none_or(|(best, _)| r2 < best) {
                    nearest = Some((r2, *member.record()));
                }
            }
        }
    }
    nearest.map(|(_, record)| record)
}

fn write_position(w: &mut GoldenWriter, label: &str, position: &GalacticPosition) {
    let [x, y, z] = position.cell().to_array();
    w.line(&format!("{label}_cell = [{x}, {y}, {z}]"));
    for (axis, value) in ["x", "y", "z"].into_iter().zip(position.offset_metres()) {
        w.f64(&format!("{label}_offset_{axis}_m"), value);
    }
}

/// Writes what `observer` sees of a system on `line`, and returns the gap between its
/// extrapolated present and its present on the line, m.
fn write_seen(
    w: &mut GoldenWriter,
    line: &Drift,
    seen: &ObservedSystem,
    observer: &Observer,
) -> f64 {
    let hit = seen.hit();
    write_position(w, "present", hit.position());
    w.f64("distance_now_ly", hit.distance().value());
    let r = seen.retardation();
    w.line(&format!(
        "emitted = {}s {}ns",
        r.emitted().seconds(),
        r.emitted().subsec_nanos()
    ));
    w.line(&format!(
        "light_age = {}s {}ns",
        r.light_age().seconds(),
        r.light_age().subsec_nanos()
    ));
    w.f64("retardation_distance_now_ly", r.distance_now().value());
    write_position(w, "apparent", r.apparent_position());
    for (axis, value) in ["x", "y", "z"]
        .into_iter()
        .zip(r.velocity_then().metres_per_second())
    {
        w.f64(&format!("velocity_then_{axis}_m_s"), value);
    }
    w.line(&format!("motion = {:?}", r.motion()));
    w.line(&format!("existence_then = {:?}", seen.existence_then()));
    match seen.brief_then() {
        Some(brief) => {
            w.line(&format!(
                "brief_then = {:?} {} of {} stars",
                brief.kind(),
                brief.class(),
                brief.star_count()
            ));
            w.f64(
                "brief_then_effective_temperature_k",
                brief.effective_temperature().value(),
            );
            if let Some(log) = brief.log_luminosity() {
                w.f64("brief_then_log_luminosity", log.value());
            }
        }
        None => w.line("brief_then = none"),
    }
    w.f64("curvature_error_ly", seen.error().length().value());
    w.f64("curvature_error_arcsec", seen.error().angle().value());
    let extrapolated = extrapolate_to_present(r, observer.time());
    let gap = extrapolated
        .distance_to(&line.position_at(observer.time()))
        .value();
    w.f64("extrapolated_gap_m", gap);
    let towards = bearing(observer.position(), r.apparent_position()).expect("two points");
    w.line(&format!("bearing_frame = {:?}", towards.frame()));
    w.f64("bearing_azimuth_deg", towards.azimuth().value());
    w.f64("bearing_elevation_deg", towards.elevation().value());
    gap
}

/// P12.T13: the retarded observations of six pinned systems from two observers.
#[test]
fn retarded_observations_are_the_golden_vectors() {
    let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
        .with_full_potential();
    let sun = at(SUN);
    let interior = FeatureCatalogue::near(&galaxy, &sun, LightYears::new(3_000.0), &NoFeatureCache)
        .find_map(|f| FeatureInterior::of(&galaxy, &f))
        .expect("a feature with members lies within 3,000 ly of the Sun");
    let interiors = KeepInteriors::new(&galaxy);
    let source = FeatureMemberSource::new(&NoFeatureCache, &interiors);
    // Each system's rule: a grid layer's cells about a place, or the feature's members.
    let systems: [(&str, Option<GridRule>); 6] = [
        ("neighbour", Some((Layer::A, SUN, Pick::Nearest))),
        ("massive", Some((Layer::E, SUN, Pick::Heaviest))),
        ("member", None),
        (
            "nuclear_disc",
            Some((
                Layer::A,
                [0.0, 300.0, 0.0],
                Pick::Population(Population::NuclearDisc),
            )),
        ),
        (
            "halo",
            Some((
                Layer::E,
                [12_000.0, 8_000.0, 18_000.0],
                Pick::Population(Population::Halo),
            )),
        ),
        (
            "far_disc",
            Some((Layer::A, [-20_000.0, -30_000.0, 30.0], Pick::Nearest)),
        ),
    ];
    let far_time = UniverseTime::from_julian_years(FAR_YEARS).expect("in the clock window");
    let observers = [
        (
            "sun",
            Observer::new(sun, UniverseTime::EPOCH).expect("inside"),
        ),
        ("far", Observer::new(at(FAR), far_time).expect("inside")),
    ];
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let mut widest_gap: f64 = 0.0;
    for (name, rule) in systems {
        let record = match rule {
            Some((layer, place, pick)) => grid_pick(&galaxy, &source, layer, &at(place), pick),
            None => member_pick(&galaxy, &interior),
        }
        .unwrap_or_else(|| panic!("{name}: no system by its rule"));
        let rule = rule.map_or_else(
            || "the feature's nearest member".to_owned(),
            |(layer, place, pick)| format!("{pick:?} of layer {layer:?} about {place:?} ly"),
        );
        w.line(&format!("{name} = {} ({rule})", record.id()));
        let stars = stars_of(&galaxy, &interiors, &record).expect("a system's stars");
        let line = Drift::of_record_in(&galaxy, &interiors, &record).expect("a system's own line");
        for (who, observer) in &observers {
            // The system as a search at the observer's time finds it, on its own line: the
            // observation reads its present from the line, not from the hit.
            let present = line.position_at(observer.time());
            let distance = LightYears::from(observer.position().distance_to(&present));
            let hit = SystemHit::new(record, present, distance);
            let seen = observe_hit_in(&galaxy, &interiors, &hit, &stars, observer)
                .expect("no pinned system is a centre member or a catalogue system");
            w.line(&format!("{name} from {who}"));
            widest_gap = widest_gap.max(write_seen(&mut w, &line, &seen, observer));
        }
    }
    // An extrapolated jump lands on the star (Design note 1).
    assert!(
        widest_gap < 1.0,
        "an extrapolated present {widest_gap} m off"
    );
    golden!("observe/retarded", w.as_str());
}
