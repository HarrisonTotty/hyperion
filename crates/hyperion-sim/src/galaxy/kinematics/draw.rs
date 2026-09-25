//! The velocity draw and the escape cut (plan 08, P08.T5 and Design notes 6 and 7).

use super::{EllipsoidAxes, VelocityEllipsoid};
use crate::coords::GalacticVelocity;
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::galaxy::query::PAD_SPEED;
use crate::galaxy::{Galaxy, PointLy};
use crate::math;
use crate::rng::{ObjectKey, Stream, tags};
use crate::units::{KilometresPerSecond, LightYears};

/// How many draws the escape cut makes before it scales the last one below the cut (plan 08,
/// Design note 7).
pub const ESCAPE_CUT_ATTEMPTS: u32 = 16;

/// Where the last draw is scaled to when every attempt reached the cut: 0.99 of it.
const ESCAPE_CUT_SCALE: f64 = 0.99;

/// Words per attempt: a `standard_normal_pair` and a `standard_normal`, two words each by plan
/// 01's Box–Muller.
const WORDS_PER_ATTEMPT: u64 = 4;

/// Metres per second in a kilometre per second.
const M_PER_KM: f64 = 1_000.0;

/// A system's velocity and how many attempts of the escape cut it took.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VelocityDraw {
    velocity: GalacticVelocity,
    attempts: u32,
    cut: KilometresPerSecond,
}

impl VelocityDraw {
    /// The velocity at the epoch, in the galactic frame.
    #[must_use]
    pub fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The attempts taken, 1 to [`ESCAPE_CUT_ATTEMPTS`]; when every attempt reached the cut, the
    /// last was scaled to 0.99 of it and this is [`ESCAPE_CUT_ATTEMPTS`].
    #[must_use]
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// The cut: the lesser of the local escape speed and plan 03's padding speed.
    #[must_use]
    pub fn cut(&self) -> KilometresPerSecond {
        self.cut
    }
}

/// The velocity of `record` at the epoch, in the galactic frame: its component's law at its
/// position ([`KinematicTables::ellipsoid`](super::KinematicTables::ellipsoid)), Gaussian in the
/// ellipsoid's axes, cut below the local escape speed (plan 08, P08.T5).
///
/// # Panics
///
/// If `galaxy` holds no kinematic tables: it must be built with
/// [`Galaxy::with_full_potential`].
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::kinematics::draw_velocity;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::id::Layer;
///
/// let galaxy = Galaxy::from_params(Seed::new(3), GalaxyParams::milky_way_like())?
///     .with_full_potential();
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
/// // A disc star near the Sun's radius moves at about the circular speed.
/// let v = draw_velocity(&galaxy, &cell[0]).speed().value() / 1e3;
/// assert!((150.0..320.0).contains(&v), "{v} km/s");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn draw_velocity(galaxy: &Galaxy, record: &SystemRecord) -> GalacticVelocity {
    draw(galaxy, record).velocity()
}

/// [`draw_velocity`] with its attempt count and cut, for the tests of the cut.
///
/// Attempt `k` seeks the `system.velocity` stream, keyed by the system's ID, to word `4k` and
/// takes a `standard_normal_pair` and a `standard_normal`: the three components along the
/// ellipsoid's axes, which are rotated to the galactic axes through plan 01's named directions at
/// the epoch position (on the z axis, the azimuth of +x). A draw whose speed reaches the cut, the
/// lesser of the escape speed there and [`PAD_SPEED`], is drawn again, up to
/// [`ESCAPE_CUT_ATTEMPTS`]; the last is then scaled to 0.99 of the cut. So no velocity reaches
/// its padding speed, even near the black hole, where the escape speed passes 1,000 km/s.
///
/// # Panics
///
/// As [`draw_velocity`].
#[must_use]
pub fn draw(galaxy: &Galaxy, record: &SystemRecord) -> VelocityDraw {
    let tables = galaxy
        .kinematics()
        .expect("velocities need the kinematic tables, which Galaxy::with_full_potential builds");
    let SystemOrigin::Grid(component) = record.origin();
    let p = PointLy::from(record.epoch_position());
    let ellipsoid = tables.ellipsoid(component, &p);
    let basis = basis(record, &p, ellipsoid.axes());
    let r_cyl = math::hypot(p.x, p.y);
    let escape = galaxy
        .potential()
        .escape_speed(LightYears::new(r_cyl), LightYears::new(p.z))
        .map_or(f64::INFINITY, KilometresPerSecond::value);
    let cut = if escape.is_nan() {
        PAD_SPEED.value()
    } else {
        escape.min(PAD_SPEED.value())
    };
    let mut stream = Stream::open(
        galaxy.seed(),
        tags::SYSTEM_VELOCITY,
        ObjectKey::from(record.id()),
    );
    let mut last = [0.0; 3];
    for k in 0..ESCAPE_CUT_ATTEMPTS {
        stream.seek(WORDS_PER_ATTEMPT * u64::from(k));
        let v = attempt(&mut stream, &ellipsoid, &basis);
        let speed = norm(v);
        if speed < cut {
            return VelocityDraw {
                velocity: to_metres_per_second(v),
                attempts: k + 1,
                cut: KilometresPerSecond::new(cut),
            };
        }
        last = v;
    }
    let scale = ESCAPE_CUT_SCALE * cut / norm(last);
    VelocityDraw {
        velocity: to_metres_per_second(last.map(|c| c * scale)),
        attempts: ESCAPE_CUT_ATTEMPTS,
        cut: KilometresPerSecond::new(cut),
    }
}

/// One attempt: three normals along the ellipsoid's axes, in galactic components, km/s.
fn attempt(stream: &mut Stream, ellipsoid: &VelocityEllipsoid, basis: &[[f64; 3]; 3]) -> [f64; 3] {
    let (n0, n1) = stream.standard_normal_pair();
    let n2 = stream.standard_normal();
    let (mean, sigma) = (ellipsoid.mean(), ellipsoid.sigma());
    let local = [
        mean[0].value() + sigma[0].value() * n0,
        mean[1].value() + sigma[1].value() * n1,
        mean[2].value() + sigma[2].value() * n2,
    ];
    let mut v = [0.0; 3];
    for (axis, &component) in basis.iter().zip(&local) {
        for (sum, &unit) in v.iter_mut().zip(axis) {
            *sum += component * unit;
        }
    }
    v
}

/// The ellipsoid's three unit vectors in galactic components: rimward, spinward and north for
/// cylindrical axes; outward, +θ and spinward for spherical ones.
fn basis(record: &SystemRecord, p: &PointLy, axes: EllipsoidAxes) -> [[f64; 3]; 3] {
    let (rimward, spinward) = match record.epoch_position().directions() {
        Some(d) => (d.rimward().components(), d.spinward().components()),
        None => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    };
    match axes {
        EllipsoidAxes::Cylindrical => [rimward, spinward, [0.0, 0.0, 1.0]],
        EllipsoidAxes::Spherical => {
            let r_cyl = math::hypot(p.x, p.y);
            let r = math::hypot(r_cyl, p.z);
            let (sin_theta, cos_theta) = if r > 0.0 {
                (r_cyl / r, p.z / r)
            } else {
                (0.0, 1.0)
            };
            let outward = [sin_theta * rimward[0], sin_theta * rimward[1], cos_theta];
            let polar = [cos_theta * rimward[0], cos_theta * rimward[1], -sin_theta];
            [outward, polar, spinward]
        }
    }
}

/// `|v|`.
fn norm(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// km/s to plan 01's metres per second.
fn to_metres_per_second(v: [f64; 3]) -> GalacticVelocity {
    GalacticVelocity::new(v.map(|c| c * M_PER_KM))
}
