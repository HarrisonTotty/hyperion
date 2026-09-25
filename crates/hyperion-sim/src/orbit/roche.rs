//! The Roche lobe of a star in a binary, after Eggleton (1983, ApJ 268, 368).

use crate::math;
use crate::units::Metres;

/// The volume-equivalent radius of the Roche lobe of the star of mass M₁ in a binary with a
/// companion of mass M₂ at separation `separation`: Eggleton's (1983) eq. (2),
///
/// ```text
/// r_L ÷ a = 0.49 q^(2/3) ÷ (0.6 q^(2/3) + ln(1 + q^(1/3))),   q = M₁ ÷ M₂,
/// ```
///
/// which Eggleton gives as accurate to better than 1% for every q in `(0, ∞)`. `q` is the mass of
/// the star whose lobe this is (the donor, when it fills it) over its companion's. The lobe is
/// defined for a circular orbit in corotation, whose separation is the semi-major axis. Plan 11
/// compares a star's radius with the lobe at periapsis (Design note 7), the usual approximation
/// for an eccentric binary and an extrapolation of the formula.
///
/// # Panics
///
/// If `q_donor_over_accretor` is not finite and positive.
///
/// # Examples
///
/// ```
/// use hyperion_sim::orbit::roche_lobe_radius;
/// use hyperion_sim::units::{AstronomicalUnits, Metres, SolarRadii};
///
/// // A red giant of 1.2 M☉ with a 0.6 M☉ white dwarf 1 au away fills its lobe at about 95 R☉.
/// let lobe = roche_lobe_radius(2.0, Metres::from(AstronomicalUnits::new(1.0)));
/// let in_solar_radii = SolarRadii::from(lobe).value();
/// assert!((94.0..96.0).contains(&in_solar_radii));
/// ```
#[must_use]
pub fn roche_lobe_radius(q_donor_over_accretor: f64, separation: Metres) -> Metres {
    let q = q_donor_over_accretor;
    assert!(
        q.is_finite() && q > 0.0,
        "the mass ratio {q} must be finite and positive"
    );
    let cube_root = math::cbrt(q);
    let two_thirds = cube_root * cube_root;
    separation * (0.49 * two_thirds / (0.6 * two_thirds + math::ln_1p(cube_root)))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    fn ratio(q: f64) -> f64 {
        roche_lobe_radius(q, Metres::new(1.0)).value()
    }

    #[test]
    fn equal_masses_have_lobes_of_0_379_of_the_separation() {
        // q = 1: 0.49 ÷ (0.6 + ln 2) = 0.378 921…
        assert!((ratio(1.0) - 0.49 / (0.6 + core::f64::consts::LN_2)).abs() < 1e-16);
        assert!((ratio(1.0) - 0.378_921).abs() < 1e-6);
        // The two lobes, back to back through the inner Lagrangian point, fit in the separation.
        for q in [0.01, 0.3, 1.0, 5.0, 100.0] {
            assert!(ratio(q) + ratio(1.0 / q) < 1.0, "q = {q}");
        }
    }

    #[test]
    fn the_lobe_grows_with_the_mass_ratio_between_its_limits() {
        let mut previous = 0.0;
        for k in -40..=40 {
            let q = math::exp10(f64::from(k) / 10.0);
            let r = ratio(q);
            assert!(r > previous, "q = {q}");
            previous = r;
        }
        // q → ∞: 0.49 ÷ 0.6. q → 0: 0.49 q^(1/3), since ln(1 + x) ≈ x.
        assert!((ratio(1e12) - 0.49 / 0.6).abs() < 1e-4);
        assert!((ratio(1e-12) / (0.49 * 1e-4) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn the_lobe_agrees_with_paczynskis_formula_where_that_holds() {
        // Paczyński (1971, ARA&A 9, 183): r_L ÷ a = 0.462 (q ÷ (1 + q))^(1/3) for q up to 0.8.
        for k in 0..=15 {
            let q = 0.05 + 0.05 * f64::from(k);
            let paczynski = 0.462 * math::cbrt(q / (1.0 + q));
            assert!(
                (ratio(q) / paczynski - 1.0).abs() < 0.03,
                "q = {q}: {} against {paczynski}",
                ratio(q)
            );
        }
    }

    /// Eggleton's claim, "better than 1%" for every q, against Roche lobes integrated here
    /// independently of his fit: the volume inside the critical equipotential through the inner
    /// Lagrangian point of a circular corotating binary, by rays from the star on a 400 × 200 grid
    /// in (cos θ, φ), each crossing found by bisection, and turned into the radius of a sphere of
    /// that volume (round 8's validation). At q = 1 it is 0.3799, the textbook value. The fit
    /// lies within 0.81% of every one, worst at q = 0.05.
    #[test]
    fn the_lobe_is_within_one_per_cent_of_the_integrated_lobe_volume() {
        let integrated = [
            (1e-3, 0.048_22),
            (0.01, 0.101_24),
            (0.05, 0.166_99),
            (0.1, 0.205_40),
            (0.2, 0.250_64),
            (0.5, 0.320_64),
            (1.0, 0.379_86),
            (2.0, 0.441_97),
            (5.0, 0.523_32),
            (10.0, 0.580_30),
            (100.0, 0.718_16),
            (1000.0, 0.781_65),
        ];
        for (q, lobe) in integrated {
            let error = ratio(q) / lobe - 1.0;
            assert!(error.abs() < 0.01, "q = {q}: {} against {lobe}", ratio(q));
        }
    }

    #[test]
    fn the_lobe_scales_with_the_separation() {
        assert_same_bits(
            roche_lobe_radius(0.4, Metres::new(2.0)).value(),
            2.0 * ratio(0.4),
        );
    }

    #[test]
    #[should_panic(expected = "must be finite and positive")]
    fn a_zero_mass_ratio_is_refused() {
        let _ = roche_lobe_radius(0.0, Metres::new(1.0));
    }
}
