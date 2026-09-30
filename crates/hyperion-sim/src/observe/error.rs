//! The stated error of a retarded reading: how far the neglected curvature of a drifting source's
//! galactic orbit can put it from where the straight line says (plan 12, P12.T1; Design note 3).

use super::retarded::{Motion, Retardation};
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::potential::PotentialTables;
use crate::math;
use crate::time::{Span, UniverseTime};
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{Arcseconds, LightYears, Metres, MetresPerSecond, Radians};

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

/// The stated curvature error of `r`, seen from `observer` (Design note 3; ruling 143.2).
///
/// A drifting source's line and its true galactic orbit agree in position and velocity at the
/// epoch, where plan 08 draws the velocity, so the gap between them grows with the span Δ = |`t_emit`
/// − epoch| from there, not with the light's age (the two differ by at most H = 1,000 years). The
/// stated error is
///
/// min(½ a Δ², |v| Δ + 2 `r_apo`),
///
/// with R and a = `v_c²` ÷ R taken at the epoch position (the anchor, not the apparent position),
/// `v_c` the circular speed of plan 02's potential tables, |v| the source's own speed and `r_apo`
/// its apocentre from its energy. The quadratic bounds the gap while the orbit's phase over Δ is
/// short, ∫₀^Δ (Δ − u) a du ≤ ½ `a_max` Δ²; the linear term bounds it after, since the orbit stays
/// within `r_apo` of the centre, so within 2 `r_apo` of the anchor, while the line leaves at |v|.
/// The angle is the length over the distance from the observer to the apparent position. A
/// [`Motion::Followed`] source's orbit is not neglected, so its error is zero.
///
/// **Where it is an estimate, not a bound** (ruling 143.2 as built):
///
/// - The quadratic needs the largest pull along the orbit over Δ, and a at the anchor is it only
///   for a near-circular orbit, whose pull is constant. On an eccentric orbit, as a nuclear-cluster
///   member's isotropic orbit is, the pull peaks at pericentre, so the quadratic is an estimate
///   there; where the linear term governs it is a bound again.
/// - a is the in-plane `v_c²` ÷ R at the anchor's **spherical** radius R, which is the disc's R in
///   the plane and gives a star high above the axis the pull of the mass inside it rather than
///   none (P12.T1 as built), not |∇Φ| at the point.
/// - `r_apo` is the largest r at which the in-plane potential, read as spherical, Φ(r, 0), is at
///   most the energy E = Φ(`x_e`) + ½ |v|². With the (R, z) grid E is exact in the model's
///   axisymmetric, static potential; for an oblate galaxy Φ(r, 0) is the least potential on the
///   sphere of radius r, so every point the orbit reaches, where Φ ≤ E, lies within that radius,
///   and `r_apo` is an upper bound on the apocentre. Without the grid (a galaxy built without
///   [`Galaxy::with_full_potential`]) E takes Φ(|`x_e`|, 0), which is below the true energy
///   above the plane, so `r_apo` is an estimate there. The root is found by bisection on Φ(r, 0),
///   which rises outward wherever `v_c²` > 0, and the upper end of the bracket is returned.
/// - A source unbound in that estimate (E ≥ 0) has no apocentre, and the quadratic alone is
///   stated.
///
/// The brainstorm's disc and nuclear-disc figures are unchanged, since the quadratic governs both.
///
/// # Panics
///
/// If the reading's line leaves the addressable range at the epoch, which nothing slower than
/// light can from a position in the cube.
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
        Motion::Drift => {
            let span = r
                .emitted()
                .checked_since(UniverseTime::EPOCH)
                .and_then(Span::checked_abs)
                .expect("a time on the clock is a span from the epoch the clock holds");
            curvature_bound(
                galaxy.potential(),
                &r.line_at_epoch(),
                r.velocity_then().speed(),
                span,
                observer.distance_to(r.apparent_position()),
            )
        }
    }
}

/// The bound for a drifting source anchored at `anchor` at the epoch, moving at `speed`, over the
/// span `delta` from the epoch, seen along a path of `path` ([`curvature_error`]).
#[must_use]
pub(crate) fn curvature_bound(
    potential: &PotentialTables,
    anchor: &GalacticPosition,
    speed: MetresPerSecond,
    delta: Span,
    path: Metres,
) -> CurvatureError {
    let [x, y, z] = anchor.to_light_years_f64();
    let radius_ly = (x * x + y * y + z * z).sqrt();
    let v = speed.value();
    let delta = delta.as_seconds_f64();
    let radius = radius_ly * METRES_PER_LIGHT_YEAR;
    let quadratic = if radius_ly > 0.0 {
        // (km/s)² to (m/s)².
        let v_circ_sq = potential.v_circ_sq(LightYears::new(radius_ly)) * 1e6;
        0.5 * (v_circ_sq / radius) * delta * delta
    } else {
        f64::INFINITY
    };
    // The apocentre is at least R, so the linear term is at least this: the root is found only
    // where the linear term can win.
    let length = if quadratic <= v * delta + 2.0 * radius {
        quadratic
    } else {
        let r_apo = apocentre_ly(potential, x, y, z, radius_ly, v);
        quadratic.min(v * delta + 2.0 * r_apo * METRES_PER_LIGHT_YEAR)
    };
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

/// The spherical radius below which the in-plane potential is not read: the tables' first point,
/// 2⁻⁴ ly, is well inside the black hole's sphere of influence, where no drifting source is.
const APOCENTRE_FLOOR_LY: f64 = 1e-6;

/// The apocentre, light-years, of a source at `(x, y, z)` ly, `radius_ly` from the centre, moving
/// at `speed` m/s: the largest r with Φ(r, 0) ≤ E ([`curvature_error`]); infinite if E ≥ 0.
fn apocentre_ly(
    potential: &PotentialTables,
    x: f64,
    y: f64,
    z: f64,
    radius_ly: f64,
    speed: f64,
) -> f64 {
    if radius_ly <= 0.0 && speed == 0.0 {
        // At rest at the centre itself: nowhere to go.
        return 0.0;
    }
    let in_plane = |r: f64| potential.potential_in_plane(LightYears::new(r));
    let floor = radius_ly.max(APOCENTRE_FLOOR_LY);
    let here = potential
        .potential(LightYears::new(math::hypot(x, y)), LightYears::new(z))
        .filter(|phi| phi.is_finite() && radius_ly > 0.0)
        .unwrap_or_else(|| in_plane(floor));
    // (m/s)² to (km/s)².
    let energy = here + 0.5 * speed * speed * 1e-6;
    if energy.is_nan() || energy >= 0.0 {
        return f64::INFINITY;
    }
    // Φ(r, 0) ≤ E at the source's own radius, since E ≥ Φ(x_e) ≥ Φ(|x_e|, 0).
    let mut low = floor;
    let mut high = 2.0 * floor;
    // Φ → 0⁻ outward, so a bound orbit's bracket closes within a few dozen doublings.
    while in_plane(high) <= energy {
        low = high;
        high *= 2.0;
        if !high.is_finite() {
            return f64::INFINITY;
        }
    }
    for _ in 0..80 {
        let middle = f64::midpoint(low, high);
        if in_plane(middle) <= energy {
            low = middle;
        } else {
            high = middle;
        }
        if high - low <= 1e-12 * high {
            break;
        }
    }
    high
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::*;
    use crate::Seed;
    use crate::coords::{GalacticVelocity, ROOT_HALF_WIDTH_LY};
    use crate::galaxy::params::GalaxyParams;
    use crate::observe::{Drift, Observer, Trajectory, retarded};
    use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

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
        Span::from_seconds_f64(y * SECONDS_PER_JULIAN_YEAR).unwrap()
    }

    fn ly(l: f64) -> Metres {
        Metres::from(LightYears::new(l))
    }

    /// The circular speed at `radius_ly`, m/s.
    fn v_circ(radius_ly: f64) -> MetresPerSecond {
        MetresPerSecond::new(
            galaxy()
                .potential()
                .v_circ(LightYears::new(radius_ly))
                .value()
                * 1e3,
        )
    }

    /// The brainstorm's disc figure: 0.53 ly and under half an arcsecond for a disc star at the
    /// solar radius seen after 227,000 years, the light time across the cube, within 25% since the
    /// figure is rounded and the potential is the model's own (P12.T1).
    #[test]
    fn curvature_error_of_a_disc_star_matches_the_brainstorms_figures() {
        let star = at_ly([26_000.0, 0.0, 0.0]);
        let v = v_circ(26_000.0);
        let error = curvature_bound(
            galaxy().potential(),
            &star,
            v,
            years(227_000.0),
            ly(227_000.0),
        );
        let length = error.length().value();
        assert!((length - 0.53).abs() <= 0.25 * 0.53, "{length} ly");
        assert!(error.angle().value() < 0.5, "{:?}", error.angle());
        // After 50,000 years it is 0.026 ly.
        let error = curvature_bound(
            galaxy().potential(),
            &star,
            v,
            years(50_000.0),
            ly(50_000.0),
        );
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
    /// raised in its next revision, which moves this pin. The quadratic governs throughout, so
    /// ruling 143.2's linear term leaves the figure where it was.
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
                let error = curvature_bound(galaxy().potential(), &star, v_circ(radius), age, path);
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
        let near_centre = Drift::new(
            at_ly([0.3, 0.1, 0.0]),
            GalacticVelocity::new([0.0, 50e3, 0.0]),
        );
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

    /// e ÷ R of a circular orbit against its tangent line after an orbital phase `phi`:
    /// √((1 − cos φ)² + (φ − sin φ)²), in forms that do not cancel at small φ.
    fn circular_gap(phi: f64) -> f64 {
        let one_minus_cos = 2.0 * math::powi(math::sin(0.5 * phi), 2);
        let phi_minus_sin = if phi < 1e-3 {
            math::powi(phi, 3) / 6.0 - math::powi(phi, 5) / 120.0
        } else {
            phi - math::sin(phi)
        };
        math::hypot(one_minus_cos, phi_minus_sin)
    }

    /// Ruling 143.2's test: for circular orbits from half a light-year to 60,000 ly and spans from
    /// ten years to the source horizon, the stated error is never above min(½ a Δ², |v| Δ + 2
    /// `r_apo`), with `r_apo` found here by an independent outward scan of the potential, and
    /// never below the exact gap between the circle and its tangent line.
    #[test]
    fn curvature_error_is_never_above_the_bound_nor_below_the_exact_circular_error() {
        let potential = galaxy().potential();
        let in_plane = |r: f64| potential.potential_in_plane(LightYears::new(r));
        let mut linear_governed = 0;
        for radius in [
            0.5, 3.0, 10.0, 30.0, 60.0, 300.0, 2_000.0, 8_000.0, 26_000.0, 60_000.0,
        ] {
            let v = v_circ(radius).value();
            let energy = in_plane(radius) + 0.5 * v * v * 1e-6;
            // The first radius of a 0.1% ladder outward at which the potential passes the energy.
            let mut scanned = radius;
            while in_plane(scanned) <= energy {
                scanned *= 1.001;
            }
            let r = radius * METRES_PER_LIGHT_YEAR;
            for age in [
                10.0, 1_000.0, 20_000.0, 50_000.0, 113_500.0, 227_000.0, 263_000.0,
            ] {
                let delta = years(age);
                let d = delta.as_seconds_f64();
                let star = at_ly([0.0, radius, 0.0]);
                let stated =
                    curvature_bound(potential, &star, MetresPerSecond::new(v), delta, ly(age))
                        .length()
                        .value()
                        * METRES_PER_LIGHT_YEAR;
                let quadratic = 0.5 * (v * v / r) * d * d;
                let linear = v * d + 2.0 * scanned * METRES_PER_LIGHT_YEAR;
                let bound = quadratic.min(linear);
                assert!(
                    stated <= bound * (1.0 + 1e-12),
                    "{radius} ly after {age} yr: {stated} m over the bound {bound} m"
                );
                let exact = r * circular_gap(v * d / r);
                assert!(
                    stated >= exact * (1.0 - 1e-9),
                    "{radius} ly after {age} yr: {stated} m under the circle's {exact} m"
                );
                if quadratic > linear {
                    linear_governed += 1;
                }
            }
        }
        assert!(
            linear_governed >= 10,
            "{linear_governed} cases in the linear regime"
        );
        // The science check's case, 10 ly from the centre after 10⁵ years: at the model's circular
        // speed there (the check took 150 km/s and 61 ly) the circle is about 50 ly off its line,
        // the quadratic far above it and plan 12's old cap of 2R = 20 ly far below it.
        let v = v_circ(10.0);
        let delta = years(1e5);
        let stated = curvature_bound(potential, &at_ly([10.0, 0.0, 0.0]), v, delta, ly(1e5))
            .length()
            .value();
        let r = 10.0 * METRES_PER_LIGHT_YEAR;
        let exact = 10.0 * circular_gap(v.value() * delta.as_seconds_f64() / r);
        assert!(
            (45.0..65.0).contains(&exact),
            "the circle is {exact} ly off its line"
        );
        eprintln!("10 ly after 1e5 yr: {stated} ly stated, the circle {exact} ly off its line");
        assert!(
            stated >= exact && stated < 3.0 * exact,
            "{stated} ly stated against {exact} ly"
        );
        // At the centre with no motion there is nothing to state.
        assert_eq!(
            curvature_bound(
                potential,
                &GalacticPosition::ORIGIN,
                MetresPerSecond::new(0.0),
                delta,
                ly(1e5)
            ),
            CurvatureError::ZERO
        );
    }

    /// A source unbound in the spherical estimate has no apocentre: the quadratic alone is stated,
    /// however far the line has run.
    #[test]
    fn curvature_error_of_an_unbound_source_is_the_quadratic() {
        let potential = galaxy().potential();
        // A light-year from the centre at 2,000 km/s, well over the escape speed there.
        let star = at_ly([0.0, 1.0, 0.0]);
        let delta = years(227_000.0);
        let fast = MetresPerSecond::new(2.0e6);
        let stated = curvature_bound(potential, &star, fast, delta, ly(1e4))
            .length()
            .value();
        let r = METRES_PER_LIGHT_YEAR;
        let d = delta.as_seconds_f64();
        let quadratic = 0.5 * (potential.v_circ_sq(LightYears::new(1.0)) * 1e6 / r) * d * d
            / METRES_PER_LIGHT_YEAR;
        let linear = (fast.value() * d + 2.0 * r) / METRES_PER_LIGHT_YEAR;
        assert!(quadratic > linear, "{quadratic} against {linear} ly");
        assert!(
            (stated / quadratic - 1.0).abs() < 1e-12,
            "{stated} against {quadratic} ly"
        );
    }

    /// Ruling 143.2: the error is anchored at the epoch position, with the source's own speed,
    /// over the span from the epoch to the emitted time, not over the light's age. A star 4 ly
    /// away seen at +1,000 years left its light 996 years after the epoch, and the stated error is
    /// that span's, not the four years'.
    #[test]
    fn curvature_error_is_anchored_at_the_epoch_over_the_span_from_it() {
        let potential = galaxy().potential();
        let epoch_position = at_ly([12.0, 5.0, -1.0]);
        let v = GalacticVelocity::new([-40e3, 150e3, 9e3]);
        let star = Drift::new(epoch_position, v);
        let late = UniverseTime::from_julian_years(1_000).unwrap();
        let observer_at = star
            .position_at(late)
            .translated(crate::coords::GalacticDisplacement::new([
                4.0 * METRES_PER_LIGHT_YEAR,
                0.0,
                0.0,
            ]))
            .unwrap();
        let observer = Observer::new(observer_at, late).unwrap();
        let seen = retarded(&observer, &star);
        let span = seen.emitted().since_epoch();
        assert!(
            (span.as_julian_years_f64() - 996.0).abs() < 0.01,
            "{span:?}"
        );
        assert!(seen.line_at_epoch().distance_to(&epoch_position).value() < 1.0);
        let stated = curvature_error(galaxy(), observer.position(), &seen);
        let expected = curvature_bound(
            potential,
            &epoch_position,
            v.speed(),
            span,
            observer.position().distance_to(seen.apparent_position()),
        );
        assert!((stated.length().value() / expected.length().value() - 1.0).abs() < 1e-9);
        // At the apparent position after only the light's age, the figure would be a
        // sixty-thousandth of it.
        let by_light_age = curvature_bound(
            potential,
            seen.apparent_position(),
            v.speed(),
            seen.light_age(),
            observer.position().distance_to(seen.apparent_position()),
        );
        assert!(by_light_age.length().value() < 1e-4 * stated.length().value());
        // Seen from far away, where Δ and the light's age are the same, nothing changes.
        let far = Observer::new(at_ly([0.0, 26_000.0, 0.0]), UniverseTime::EPOCH).unwrap();
        let seen = retarded(&far, &star);
        let stated = curvature_error(galaxy(), far.position(), &seen);
        let expected = curvature_bound(
            potential,
            &epoch_position,
            v.speed(),
            seen.light_age(),
            far.position().distance_to(seen.apparent_position()),
        );
        assert!((stated.length().value() / expected.length().value() - 1.0).abs() < 1e-9);
    }
}
