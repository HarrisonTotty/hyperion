//! The stated error of a retarded reading: how far the neglected curvature of a drifting source's
//! galactic orbit can put it from where the straight line says (plan 12, P12.T1; Design note 3).

use super::retarded::{Motion, Retardation};
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::potential::PotentialTables;
use crate::math;
use crate::time::Span;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{Arcseconds, LightYears, Metres, Radians};

/// The bound on a retarded reading's error from neglected curvature, as a length at the source
/// and as the angle it subtends at the observer (Design note 3).
///
/// It rides with every observed record. At Milky Way values it is under half an arcsecond for a
/// disc star seen from across the cube and up to 13″ for the nuclear disc (brainstorm, "What a
/// sensor sees is the past").
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct CurvatureError {
    length: LightYears,
    angle: Arcseconds,
}

impl CurvatureError {
    /// No error: a source whose orbit is followed in full.
    pub const ZERO: Self = Self {
        length: LightYears::ZERO,
        angle: Arcseconds::ZERO,
    };

    /// The bound as a length at the source.
    #[must_use]
    pub const fn length(&self) -> LightYears {
        self.length
    }

    /// The bound as the angle it subtends at the observer.
    #[must_use]
    pub const fn angle(&self) -> Arcseconds {
        self.angle
    }
}

/// The stated curvature error of `r`, seen from `observer` (Design note 3).
///
/// A drifting source's true path curves under the galaxy's pull, of order a = `v_c²` ÷ R at its
/// galactocentric radius R, where `v_c` is the circular speed of plan 02's potential tables. Over
/// the light's age s the straight line is out by about ½ a s², capped at 2R as Design note 3
/// specifies where an orbit is short against the light time. The angle is that length over the
/// distance from the observer to the apparent position. A [`Motion::Followed`] source's orbit is
/// not neglected, so its error is zero.
///
/// **Provisional (P12.T1 finding).** The cap is the plan's and is not a bound: the straight line
/// leaves while the orbit stays within R of the centre, so past an orbital phase of about 2.5
/// radians over the light's age the gap grows as v s + 2R. It matters only for sources within a
/// few tens of light-years of the centre outside the Kepler regime; the disc's and the nuclear
/// disc's figures do not reach it. min(½ a s², `v_c` s + 2R) would bound it, and awaits a ruling.
///
/// R is taken as the spherical galactocentric radius of the apparent position, with the in-plane
/// circular speed at that radius, which is the disc's R in the plane and gives a halo star high
/// above the axis the pull of the mass inside it rather than none (P12.T1 as built, provisional).
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::observe::{Drift, Observer, curvature_error, retarded};
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::from_params(Seed::new(1), GalaxyParams::milky_way_like())?;
/// let here = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("in range")?;
/// let star = GalacticPosition::from_light_years([0.0, -26_000.0, 0.0]).ok_or("in range")?;
/// let observer = Observer::new(here, UniverseTime::EPOCH)?;
/// let seen = retarded(&observer, &Drift::new(star, GalacticVelocity::default()));
/// // A disc star across the galaxy, 52,000 years ago: about a tenth of an arcsecond.
/// let error = curvature_error(&galaxy, observer.position(), &seen);
/// assert!(error.angle().value() < 0.2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn curvature_error(
    galaxy: &Galaxy,
    observer: &GalacticPosition,
    r: &Retardation,
) -> CurvatureError {
    match r.motion() {
        Motion::Followed => CurvatureError::ZERO,
        Motion::Drift => curvature_bound(
            galaxy.potential(),
            r.apparent_position(),
            r.light_age(),
            observer.distance_to(r.apparent_position()),
        ),
    }
}

/// The bound for a drifting source at `source` over `light_age`, seen along a path of `path`.
#[must_use]
pub(crate) fn curvature_bound(
    potential: &PotentialTables,
    source: &GalacticPosition,
    light_age: Span,
    path: Metres,
) -> CurvatureError {
    let [x, y, z] = source.to_light_years_f64();
    let radius_ly = (x * x + y * y + z * z).sqrt();
    if radius_ly <= 0.0 {
        // At the centre itself the orbit's size, 2R, is zero.
        return CurvatureError::ZERO;
    }
    // (km/s)² to (m/s)².
    let v_circ_sq = potential.v_circ_sq(LightYears::new(radius_ly)) * 1e6;
    let radius = radius_ly * METRES_PER_LIGHT_YEAR;
    let acceleration = v_circ_sq / radius;
    let s = light_age.as_seconds_f64();
    let length = (0.5 * acceleration * s * s).min(2.0 * radius);
    let angle = if path.value() > 0.0 {
        Arcseconds::from(Radians::new(math::atan2(length, path.value())))
    } else {
        Arcseconds::ZERO
    };
    CurvatureError {
        length: LightYears::new(length / METRES_PER_LIGHT_YEAR),
        angle,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::*;
    use crate::Seed;
    use crate::coords::{GalacticVelocity, ROOT_HALF_WIDTH_LY};
    use crate::galaxy::params::GalaxyParams;
    use crate::observe::{Drift, Observer, Trajectory, retarded};
    use crate::time::UniverseTime;

    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params(Seed::new(0x1201_e000), GalaxyParams::milky_way_like())
                .expect("the Milky Way fixture's gas is mostly neutral")
        })
    }

    fn at_ly(ly: [f64; 3]) -> GalacticPosition {
        GalacticPosition::from_light_years(ly).unwrap()
    }

    fn years(y: f64) -> Span {
        Span::from_seconds_f64(y * crate::units::consts::SECONDS_PER_JULIAN_YEAR).unwrap()
    }

    fn ly(l: f64) -> Metres {
        Metres::from(LightYears::new(l))
    }

    /// The brainstorm's disc figure: 0.53 ly and under half an arcsecond for a disc star at the
    /// solar radius seen after 227,000 years, the light time across the cube, within 25% since the
    /// figure is rounded and the potential is the model's own (P12.T1).
    #[test]
    fn curvature_error_of_a_disc_star_matches_the_brainstorms_figures() {
        let star = at_ly([26_000.0, 0.0, 0.0]);
        let error = curvature_bound(galaxy().potential(), &star, years(227_000.0), ly(227_000.0));
        let length = error.length().value();
        assert!((length - 0.53).abs() <= 0.25 * 0.53, "{length} ly");
        assert!(error.angle().value() < 0.5, "{:?}", error.angle());
        // After 50,000 years it is 0.026 ly.
        let error = curvature_bound(galaxy().potential(), &star, years(50_000.0), ly(50_000.0));
        let length = error.length().value();
        assert!((length - 0.026).abs() <= 0.25 * 0.026, "{length} ly");
    }

    /// The brainstorm's nuclear-disc figure: up to 13″ seen from a corner of the cube, its inner
    /// edge near 100 ly at 100 km/s. The plan asks for the maximum over the nuclear disc within
    /// 10–16″.
    ///
    /// **Acceptance: the formula at the model's `v_c`** (ruling 143.1). The model's potential gives
    /// `v_c` = 76 km/s at 100 ly (94 at 30 ly, 93 at 200 ly), not the brainstorm's 100 km/s, so
    /// the maximum is pinned at the model's 7.5″ ± 10%; the brainstorm's 13″ is `v_c` = 100 km/s
    /// in the same formula, which the second half checks. Plan 02's nuclear potential is to be
    /// raised in its next revision, which moves this pin.
    #[test]
    fn curvature_error_of_the_nuclear_disc_follows_the_models_circular_speed() {
        let half = f64::from(ROOT_HALF_WIDTH_LY) - 1.0;
        let corner = at_ly([half, half, half]);
        let mut worst = 0.0_f64;
        // Radii from the inner edge out through the disc's 290 ly scale length, around the plane.
        for step in 0..=36 {
            let radius = 100.0 * math::powi(1.05, step);
            for quarter in 0..8 {
                let phi = f64::from(quarter) * core::f64::consts::FRAC_PI_4;
                let star = at_ly([radius * math::cos(phi), radius * math::sin(phi), 0.0]);
                let path = corner.distance_to(&star);
                let age = super::super::light_time(path);
                let error = curvature_bound(galaxy().potential(), &star, age, path);
                worst = worst.max(error.angle().value());
            }
        }
        assert!(
            (worst - 7.5).abs() <= 0.75,
            "the worst is {worst}\" (provisional pin)"
        );
        // The brainstorm's figure is the same formula at 100 km/s and 100 ly: 13″.
        let star = at_ly([100.0, 0.0, 0.0]);
        let path = corner.distance_to(&star);
        let s = super::super::light_time(path).as_seconds_f64();
        let r = 100.0 * METRES_PER_LIGHT_YEAR;
        let length = 0.5 * (1e5 * 1e5 / r) * s * s;
        let arcsec = Arcseconds::from(Radians::new(length / path.value())).value();
        assert!((arcsec - 13.0).abs() < 0.5, "{arcsec}\" at 100 km/s");
    }

    /// A followed orbit states no error; a drifting source seen from nearby states almost none.
    #[test]
    fn curvature_error_is_zero_for_a_followed_orbit() {
        #[derive(Debug)]
        struct Followed(Drift);
        impl Trajectory for Followed {
            fn position_at(&self, t: UniverseTime) -> GalacticPosition {
                self.0.position_at(t)
            }
            fn velocity_at(&self, t: UniverseTime) -> GalacticVelocity {
                self.0.velocity_at(t)
            }
            fn motion(&self) -> Motion {
                Motion::Followed
            }
        }
        let observer = Observer::new(at_ly([26_000.0, 0.0, 0.0]), UniverseTime::EPOCH).unwrap();
        let near_centre = Drift::new(at_ly([0.3, 0.1, 0.0]), GalacticVelocity::default());
        let followed = retarded(&observer, &Followed(near_centre));
        assert_eq!(followed.motion(), Motion::Followed);
        assert_eq!(
            curvature_error(galaxy(), observer.position(), &followed),
            CurvatureError::ZERO
        );
        let drifting = retarded(&observer, &near_centre);
        assert!(
            curvature_error(galaxy(), observer.position(), &drifting).length() > LightYears::ZERO
        );
    }

    /// The cap: the error never passes the orbit's size, 2R, however old the light.
    #[test]
    fn curvature_error_is_capped_at_the_orbits_size() {
        let potential = galaxy().potential();
        for radius in [0.5, 3.0, 10.0, 60.0, 300.0, 2_000.0, 26_000.0, 60_000.0] {
            for age in [10.0, 1_000.0, 50_000.0, 227_000.0] {
                let star = at_ly([0.0, radius, 0.0]);
                let error = curvature_bound(potential, &star, years(age), ly(age));
                assert!(
                    error.length().value() <= 2.0 * radius * (1.0 + 1e-12),
                    "{radius} ly after {age} yr: {:?}",
                    error.length()
                );
            }
        }
        // Ten light-years from the centre after 100,000 years the quadratic is far past the cap.
        let error = curvature_bound(potential, &at_ly([10.0, 0.0, 0.0]), years(1e5), ly(1e5));
        assert!(
            (error.length().value() - 20.0).abs() < 1e-9,
            "{:?}",
            error.length()
        );
        assert_eq!(
            curvature_bound(potential, &GalacticPosition::ORIGIN, years(1e5), ly(1e5)),
            CurvatureError::ZERO
        );
    }
}
