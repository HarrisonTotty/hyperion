//! Time in the range query: the pad that covers motion, where a system is at `t`, and whether it is
//! there at all (plan 03, P03.T9.e).
//!
//! A cell is chosen by the epoch positions of the systems it holds, because that is what a cell
//! holds; a system asked about at another time has moved. The query therefore walks a sphere padded
//! by the farthest anything can travel between the epoch and `t` ([`pad_for`]), and then tests each
//! system it finds at `t`, against the unpadded radius ([`hit_at`]). The pad is tiny — 0.33 ly a
//! century, 2% of a 50 ly sphere — and costs only a few more cells.
//!
//! Systems move in straight lines at their velocities, which plan 08 draws on the reserved
//! `system.velocity` tag ([`kinematics::draw_velocity`](crate::galaxy::kinematics::draw_velocity)):
//! [`position_at`] is the epoch position plus the drift since the epoch. A galaxy built without its
//! kinematic tables (without [`Galaxy::with_full_potential`]) has no velocities, and its systems
//! keep their epoch positions at every `t`, as every system did before plan 08. No position at the
//! epoch moves either way.

use super::result::SystemHit;
use super::walk::QuerySphere;
use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::kinematics::draw_velocity;
use crate::galaxy::placement::{Existence, SystemRecord};
use crate::id::Layer;
use crate::time::UniverseTime;
use crate::units::consts::SPEED_OF_LIGHT;
use crate::units::{KilometresPerSecond, LightYears, MetresPerSecond, Seconds};

/// The speed a sphere is padded by, 1,000 km/s: above any escape speed where the grid places a
/// system (brainstorm, "The range query").
///
/// At Milky Way values the escape speed near the Sun is 574 km/s, against 500–580 km/s observed
/// (brainstorm, "Fields"), and it stays under this everywhere the five layers reach, so nothing
/// bound can cross a padded sphere's edge inwards within the clock window. Two things are outside
/// that: the central parsec, where the escape speed reaches 1,100 km/s at 0.1 ly (brainstorm, "Dense
/// features"), which plan 09's source scans by its own rule and not by this walk; and plan 08's
/// unbound class, which needs more and raises [`pad_speed`] for layer E above this.
pub const PAD_SPEED: KilometresPerSecond = KilometresPerSecond::new(1_000.0);

/// The speed layer E's spheres are padded by, 3,000 km/s: above anything plan 08 places there
/// (plan 08, Design note 27).
///
/// The brainstorm says only that "the unbound class needs more" than [`PAD_SPEED`]. The fastest
/// object plan 08 places is a remnant at the kick law's upper clamp, about 2,200 km/s, launched
/// along a rotation of up to about 300 km/s, and the reserved hypervelocity survivors move at up to
/// 2,500 km/s; 3,000 km/s covers both with a fifth to spare, and the draw caps the exempt classes
/// at it. Padding chooses cells and changes no generated output.
pub const UNBOUND_PAD_SPEED: KilometresPerSecond = KilometresPerSecond::new(3_000.0);

/// The speed the sphere is padded by when walking `layer`.
///
/// [`UNBOUND_PAD_SPEED`] for layer E, the only layer whose cells hold plan 08's unbound class, and
/// [`PAD_SPEED`] for every other layer (plan 08, Design note 27). It lives behind one function so
/// that raising one layer's moves no other layer's cells (plan 03, Design note 13).
#[must_use]
pub const fn pad_speed(layer: Layer) -> KilometresPerSecond {
    match layer {
        Layer::E => UNBOUND_PAD_SPEED,
        Layer::A | Layer::B | Layer::C | Layer::D | Layer::BrownDwarf | Layer::RoguePlanet => {
            PAD_SPEED
        }
    }
}

/// How far a sphere must be padded to hold everything that could reach it by `t` at `speed`,
/// light-years: `speed ÷ c × |t|`.
///
/// A light-year is `c` times a Julian year by definition, so this is exactly the speed in units of
/// `c` times the Julian years since the epoch: 0.3336 ly a century at [`PAD_SPEED`], and 3.336 ly
/// over the whole clock window.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::query::{PAD_SPEED, pad_for};
/// use hyperion_sim::time::UniverseTime;
///
/// let century = UniverseTime::from_julian_years(-100).expect("inside the window");
/// let pad = pad_for(century, PAD_SPEED);
/// // The pad depends on |t|, so a century before the epoch pads as far as a century after.
/// assert!((pad.value() - 0.333_6).abs() < 1e-4);
/// ```
#[must_use]
pub fn pad_for(t: UniverseTime, speed: KilometresPerSecond) -> LightYears {
    let beta = MetresPerSecond::from(speed).value() / SPEED_OF_LIGHT;
    LightYears::new(beta * t.since_epoch().as_julian_years_f64().abs())
}

/// The velocity a system has at the epoch, in the galactic frame.
///
/// Plan 08's draw on the reserved domain tag `system.velocity`, keyed by the system's ID
/// ([`draw_velocity`]), for a galaxy with its kinematic tables
/// ([`Galaxy::with_full_potential`]); zero for a galaxy built without them, which has no
/// velocities. A displaced record, once plan 08's P08.T12.d places them, has its class's law; until
/// then every record is a field record and takes its component's.
#[must_use]
pub fn epoch_velocity(galaxy: &Galaxy, record: &SystemRecord) -> GalacticVelocity {
    if galaxy.kinematics().is_some() {
        draw_velocity(galaxy, record)
    } else {
        GalacticVelocity::default()
    }
}

/// Where a system is at `t`: its epoch position plus its drift over the time since the epoch.
///
/// The drift is [`epoch_velocity`] times the span from the epoch to `t`, through plan 01's
/// coordinate arithmetic, so it is exact in whole light-years and rounds only in the offset inside a
/// light-year. At the epoch it is the epoch position itself, and the velocity is not drawn.
///
/// # Panics
///
/// If the drift would take the position out of the addressable cube. It cannot: plan 08 cuts
/// every velocity below [`PAD_SPEED`], and layer E's fastest below [`UNBOUND_PAD_SPEED`], which
/// over the clock window move a position by at most 10 ly, while every grid system lies inside
/// the root cube, 65,536 ly from the centre against the addressable range's 2³¹ ly.
#[must_use]
pub fn position_at(galaxy: &Galaxy, record: &SystemRecord, t: UniverseTime) -> GalacticPosition {
    if t == UniverseTime::EPOCH {
        return *record.epoch_position();
    }
    drifted(record, epoch_velocity(galaxy, record), t)
}

/// [`position_at`] with the velocity given.
#[must_use]
fn drifted(record: &SystemRecord, velocity: GalacticVelocity, t: UniverseTime) -> GalacticPosition {
    let elapsed = Seconds::new(t.since_epoch().as_seconds_f64());
    record
        .epoch_position()
        .translated(velocity.displacement_over(elapsed))
        .expect(
            "a speed under 3,000 km/s over the clock window moves a position by at most 10 ly, \
             which no cell of the root cube can leave the addressable range by",
        )
}

/// The hit a system makes in `sphere`, or `None` if it is outside the sphere at that time or not
/// born yet.
///
/// This is the query's per-system test, and the only place the two halves of a time argument meet:
/// the distance is taken at [`QuerySphere::time`] against the sphere's **unpadded**
/// [`radius`](QuerySphere::radius), since the pad exists only to choose cells, and a system whose
/// age at that time is not yet positive is not there at all (plan 03, Design note 7).
#[must_use]
pub fn hit_at(galaxy: &Galaxy, record: &SystemRecord, sphere: &QuerySphere) -> Option<SystemHit> {
    let t = sphere.time();
    match record.existence_at(t) {
        Existence::NoSystemYet => return None,
        Existence::Exists => {}
    }
    let position = position_at(galaxy, record, t);
    let distance = LightYears::from(sphere.centre().distance_to(&position));
    (distance.value() <= sphere.radius().value())
        .then(|| SystemHit::new(*record, position, distance))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::coords::LyCell;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{CellKey, SystemOrigin, generate_cell};
    use crate::galaxy::{Galaxy, Population};
    use crate::rng::Seed;
    use crate::time::{CLOCK_WINDOW_H, ClockWindow};
    use crate::units::{SolarMasses, Years};

    /// The seed of the galaxy these tests move systems in.
    const SEED: u64 = 0x0309_e000_0000_0000;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    }

    fn at(years: i64) -> UniverseTime {
        UniverseTime::from_julian_years(years).unwrap()
    }

    /// A record at the origin's light-year with the given age at the epoch.
    fn record(galaxy: &Galaxy, age_years: f64) -> SystemRecord {
        let key = CellKey::new(Layer::C, [0, 812, 0]).unwrap();
        SystemRecord::from_parts(
            key.candidate_id(1).unwrap(),
            GalacticPosition::new(LyCell::new([0, 26_000, 0]), [0.0; 3]).unwrap(),
            SystemOrigin::Grid(galaxy.fields().component_id(0).unwrap()),
            Population::OldThinDisc,
            SolarMasses::new(1.0),
            Years::new(age_years),
        )
    }

    #[test]
    fn motion_pads_a_third_of_a_light_year_a_century() {
        let pad = pad_for(at(100), PAD_SPEED).value();
        // 1,000 km/s ÷ 299,792.458 km/s × 100 yr.
        assert!((pad - 0.333_6).abs() < 5e-5, "{pad} ly");
        // It adds 0.67% to a 50 ly radius, which is 2% of the cells the query touches.
        let growth = 1.0 + pad / 50.0;
        let cells = growth * growth * growth - 1.0;
        assert!((cells - 0.020).abs() < 1e-3, "{cells} more cells at 50 ly");
        // |t|: the pad is the same on either side of the epoch, and zero at it.
        assert!((pad_for(at(-100), PAD_SPEED).value() - pad).abs() < f64::EPSILON);
        assert_same_bits(pad_for(UniverseTime::EPOCH, PAD_SPEED).value(), 0.0);
        // Over the whole clock window it is 3.336 ly, under 7% of a 50 ly sphere.
        let full = pad_for(ClockWindow::END, PAD_SPEED).value();
        assert!((full - 3.336).abs() < 5e-4, "{full} ly over H");
        assert!((full - 10.0 * pad).abs() < 1e-9);
        assert!((CLOCK_WINDOW_H.as_julian_years_f64() - 1_000.0).abs() < f64::EPSILON);
    }

    /// Plan 08, P08.T6: layer E pads at the unbound class's 3,000 km/s, every other layer at plan
    /// 03's 1,000 km/s; at |t| = H that is 10 ly, against layer E's 128 ly cells.
    #[test]
    fn motion_pads_layer_e_for_the_unbound_class() {
        for layer in Layer::ALL {
            let expected = if layer == Layer::E {
                UNBOUND_PAD_SPEED
            } else {
                PAD_SPEED
            };
            assert_eq!(pad_speed(layer), expected, "{layer:?}");
        }
        let pad = pad_for(ClockWindow::END, UNBOUND_PAD_SPEED).value();
        assert!((pad - 10.007).abs() < 1e-3, "{pad} ly");
    }

    #[test]
    fn motion_leaves_every_system_still_without_kinematic_tables() {
        let galaxy = galaxy();
        let mut cell = Vec::new();
        generate_cell(
            &galaxy,
            CellKey::new(Layer::C, [0, 812, 0]).unwrap(),
            &mut cell,
        );
        assert!(!cell.is_empty());
        for system in &cell {
            assert_eq!(epoch_velocity(&galaxy, system), GalacticVelocity::default());
            for t in [UniverseTime::EPOCH, ClockWindow::START, ClockWindow::END] {
                assert_eq!(&position_at(&galaxy, system, t), system.epoch_position());
            }
        }
    }

    #[test]
    fn motion_moves_a_record_by_velocity_times_time_symmetrically() {
        let galaxy = galaxy();
        let system = record(&galaxy, 1.0e9);
        // 100 km/s along +x: 0.0334 ly a century, 316 astronomical units.
        let velocity = GalacticVelocity::new([1.0e5, 0.0, 0.0]);
        for years in [100_i64, -100, 1_000, -1_000] {
            let moved = drifted(&system, velocity, at(years));
            let expected = 1.0e5
                * f64::from(i32::try_from(years).unwrap())
                * crate::units::consts::SECONDS_PER_JULIAN_YEAR;
            let [dx, dy, dz] = system.epoch_position().displacement_to(&moved).metres();
            assert!(
                (dx - expected).abs() < 1e-3 * expected.abs(),
                "{years} yr: moved {dx} m against {expected} m"
            );
            assert!(dy.abs() < 1.0 && dz.abs() < 1.0, "drift off the x axis");
        }
        // The same speed the other way undoes it.
        let there = drifted(&system, velocity, at(500));
        let back = drifted(&system, -velocity, at(500));
        let [ax, _, _] = system.epoch_position().displacement_to(&there).metres();
        let [bx, _, _] = system.epoch_position().displacement_to(&back).metres();
        assert!((ax + bx).abs() < 1.0, "{ax} against {bx}");
    }

    #[test]
    fn motion_drops_a_system_that_is_not_born_yet() {
        let galaxy = galaxy();
        let system = record(&galaxy, -200.0);
        let centre = *system.epoch_position();
        let sphere =
            |t| QuerySphere::new(centre, LightYears::new(10.0), t, LightYears::ZERO).unwrap();
        // Born at +200 yr: absent at +100 yr, present at +300 yr.
        assert_eq!(hit_at(&galaxy, &system, &sphere(at(100))), None);
        assert!(hit_at(&galaxy, &system, &sphere(at(300))).is_some());
        assert_eq!(hit_at(&galaxy, &system, &sphere(UniverseTime::EPOCH)), None);
    }

    #[test]
    fn motion_tests_the_distance_against_the_unpadded_radius() {
        let galaxy = galaxy();
        let system = record(&galaxy, 1.0e9);
        let position = *system.epoch_position();
        let near = GalacticPosition::new(LyCell::new([5, 26_000, 0]), [0.0; 3]).unwrap();
        // Five light-years away: inside a 6 ly sphere, outside a 4 ly one however it is padded.
        let inside =
            QuerySphere::new(near, LightYears::new(6.0), at(10), LightYears::ZERO).unwrap();
        let hit = hit_at(&galaxy, &system, &inside).expect("five light-years is inside six");
        assert_eq!(hit.record(), &system);
        assert_eq!(hit.position(), &position);
        assert!(
            (hit.distance().value() - 5.0).abs() < 1e-9,
            "{:?}",
            hit.distance()
        );
        let padded =
            QuerySphere::new(near, LightYears::new(4.0), at(10), LightYears::new(100.0)).unwrap();
        assert_eq!(hit_at(&galaxy, &system, &padded), None);
    }
}
