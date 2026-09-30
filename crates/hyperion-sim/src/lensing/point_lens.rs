//! The point lens: its Einstein angle and the magnification of a point source (plan 12, P12.T4.b;
//! Design note 13).

use crate::units::consts::{GM_SUN, SPEED_OF_LIGHT};
use crate::units::{Metres, Radians, SolarMasses};

/// The magnification of a point source by a point lens at `u` Einstein radii from it:
/// (u² + 2) ÷ (u √(u² + 4)) (Paczyński 1986, ApJ 304, 1, eq. 5).
///
/// It is 1.34 at u = 1, the conventional threshold of an event, falls to 1 far from the lens, and
/// grows as 1 ÷ u towards it; at u = 0 a point source is infinitely magnified, which this returns.
/// A negative `u` is taken as its magnitude.
///
/// # Examples
///
/// ```
/// use hyperion_sim::lensing::point_lens_magnification;
///
/// assert!((point_lens_magnification(1.0) - 3.0 / 5.0_f64.sqrt()).abs() < 1e-15);
/// assert!(point_lens_magnification(100.0) < 1.000_001);
/// ```
#[must_use]
pub fn point_lens_magnification(u: f64) -> f64 {
    let u = u.abs();
    if u <= 0.0 {
        return f64::INFINITY;
    }
    let u_sq = u * u;
    (u_sq + 2.0) / (u * (u_sq + 4.0).sqrt())
}

/// The Einstein angle of a point lens of `mass` at `d_l` in front of a source at `d_s`: `θ_E²` = 4
/// G M ÷ c² × (`D_s` − `D_l`) ÷ (`D_l` `D_s`), in flat space, with `D_ls` = `D_s` − `D_l` as it is
/// at galactic distances (Paczyński 1986, eq. 2), and GM☉ in place of G M☉ (IAU 2015 B3).
///
/// Zero unless 0 < `D_l` < `D_s` and the mass is positive.
///
/// # Examples
///
/// ```
/// use hyperion_sim::lensing::einstein_angle;
/// use hyperion_sim::units::{Kiloparsecs, Metres, SolarMasses};
///
/// // A solar-mass lens half-way to the bulge: about a milliarcsecond.
/// let d_s = Metres::from(Kiloparsecs::new(8.0));
/// let theta = einstein_angle(SolarMasses::new(1.0), d_s * 0.5, d_s);
/// let mas = theta.value() * 206_264_806.2;
/// assert!((mas - 1.0).abs() < 0.05, "{mas} mas");
/// ```
#[must_use]
pub fn einstein_angle(mass: SolarMasses, d_l: Metres, d_s: Metres) -> Radians {
    let (l, s) = (d_l.value(), d_s.value());
    if !(mass.value() > 0.0 && l > 0.0 && l < s) {
        return Radians::ZERO;
    }
    let schwarzschild_twice = 4.0 * GM_SUN * mass.value() / (SPEED_OF_LIGHT * SPEED_OF_LIGHT);
    Radians::new((schwarzschild_twice * (s - l) / (l * s)).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::{AstronomicalUnits, Kiloparsecs};

    #[test]
    fn point_lens_magnification_is_the_textbook_curve() {
        assert!((point_lens_magnification(1.0) - 1.341_640_786_499_873_8).abs() < 1e-15);
        // A → 1 ÷ u towards the lens, and 1 + 2 ÷ u⁴ far from it, whose next term is −8 ÷ u⁶,
        // 1.25 × 10⁻⁷ at u = 20.
        let near = point_lens_magnification(1e-3);
        assert!((near * 1e-3 - 1.0).abs() < 1e-6, "{near}");
        let far = point_lens_magnification(20.0);
        assert!(
            (far - (1.0 + 2.0 / crate::math::powi(20.0, 4))).abs() < 2e-7,
            "{far}"
        );
        assert!(point_lens_magnification(0.0).is_infinite());
        assert!((point_lens_magnification(-0.5) - point_lens_magnification(0.5)).abs() < 1e-15);
        // Monotone: nearer is brighter.
        let mut last = f64::INFINITY;
        for k in 1..1_000 {
            let a = point_lens_magnification(f64::from(k) * 0.01);
            assert!(a < last);
            last = a;
        }
    }

    /// A solar mass half-way to a source 8 kpc away has an Einstein radius of about 4 au (the
    /// textbook's `R_E` ≈ 4 au and `θ_E` ≈ 1 mas for the bulge).
    #[test]
    fn einstein_radius_of_a_solar_mass_half_way_to_the_bulge_is_four_au() {
        let d_s = Metres::from(Kiloparsecs::new(8.0));
        let d_l = d_s * 0.5;
        let theta = einstein_angle(SolarMasses::new(1.0), d_l, d_s);
        let r_e = AstronomicalUnits::from(Metres::new(theta.value() * d_l.value())).value();
        assert!((r_e - 4.03).abs() < 0.05, "{r_e} au");
        // √M, and zero outside the source's distance.
        let heavy = einstein_angle(SolarMasses::new(4.0), d_l, d_s);
        assert!((heavy.value() / theta.value() - 2.0).abs() < 1e-12);
        assert_eq!(
            einstein_angle(SolarMasses::new(1.0), d_s, d_s),
            Radians::ZERO
        );
        assert_eq!(
            einstein_angle(SolarMasses::new(1.0), Metres::ZERO, d_s),
            Radians::ZERO
        );
    }
}
