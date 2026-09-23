//! The zero-age main sequence: luminosity and radius as rational functions of mass (plan 06,
//! P06.T4.c).
//!
//! Tout, Pols, Eggleton and Han (1996, MNRAS 281, 257), equations 1 and 2, with the coefficients of
//! their Tables 1 and 2 evaluated at Z by [`ZCoeffs`]; fitted over 0.1–100 M☉ and Z = 0.0001–0.03,
//! to 3% in L and 1.2% in R at Z = 0.02 (their section 3). These are the starting points of every
//! main-sequence track of Hurley, Pols and Tout (2000, section 5.1).
//!
//! The half-integer powers are formed from integer powers and one square root, all exact IEEE
//! operations, so the values are the same on every target; the form is output.

use crate::units::{SolarLuminosities, SolarMasses, SolarRadii};

use super::coeffs::ZCoeffs;

/// Powers of M that the two fits use, built by repeated multiplication in a fixed order.
struct Powers {
    root: f64,
    m2: f64,
    m3: f64,
    m5: f64,
    m6: f64,
    m7: f64,
    m8: f64,
    m9: f64,
    m11: f64,
    m18: f64,
    m19: f64,
}

impl Powers {
    #[must_use]
    fn of(m: f64) -> Self {
        let m2 = m * m;
        let m3 = m2 * m;
        let m5 = m3 * m2;
        let m6 = m3 * m3;
        let m7 = m6 * m;
        let m8 = m7 * m;
        let m9 = m8 * m;
        let m11 = m9 * m2;
        let m18 = m9 * m9;
        let m19 = m18 * m;
        Self {
            root: m.sqrt(),
            m2,
            m3,
            m5,
            m6,
            m7,
            m8,
            m9,
            m11,
            m18,
            m19,
        }
    }
}

/// The zero-age main-sequence luminosity of a star of mass `m` (0.1–100 M☉; the fit extends
/// smoothly beyond).
///
/// Tout et al. (1996) equation 1: L = (α M^5.5 + β M^11) ÷ (γ + M³ + δ M⁵ + ε M⁷ + ζ M⁸ + η M^9.5).
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::sse::{ZCoeffs, zams};
/// use hyperion_sim::units::{MetalFraction, SolarMasses};
///
/// let solar = ZCoeffs::new(MetalFraction::new(0.02));
/// // The zero-age Sun was about 30% fainter than today's.
/// let l = zams::luminosity(SolarMasses::new(1.0), &solar).value();
/// assert!((l - 0.698).abs() < 0.01);
/// ```
#[must_use]
pub fn luminosity(m: SolarMasses, coeffs: &ZCoeffs) -> SolarLuminosities {
    let [alpha, beta, gamma, delta, epsilon, zeta, eta] = *coeffs.zams_l();
    let p = Powers::of(m.value());
    let numerator = alpha * p.m5 * p.root + beta * p.m11;
    let denominator =
        gamma + p.m3 + delta * p.m5 + epsilon * p.m7 + zeta * p.m8 + eta * p.m9 * p.root;
    SolarLuminosities::new(numerator / denominator)
}

/// The zero-age main-sequence radius of a star of mass `m` (0.1–100 M☉).
///
/// Tout et al. (1996) equation 2: R = (θ M^2.5 + ι M^6.5 + κ M^11 + λ M^19 + µ M^19.5) ÷
/// (ν + ξ M² + ο M^8.5 + M^18.5 + π M^19.5).
#[must_use]
pub fn radius(m: SolarMasses, coeffs: &ZCoeffs) -> SolarRadii {
    let [theta, iota, kappa, lambda, mu, nu, xi, omicron, pi] = *coeffs.zams_r();
    let p = Powers::of(m.value());
    let numerator = theta * p.m2 * p.root
        + iota * p.m6 * p.root
        + kappa * p.m11
        + lambda * p.m19
        + mu * p.m19 * p.root;
    let denominator =
        nu + xi * p.m2 + omicron * p.m8 * p.root + p.m18 * p.root + pi * p.m19 * p.root;
    SolarRadii::new(numerator / denominator)
}

#[cfg(test)]
mod tests {
    use super::super::continuity::assert_continuous_over;
    use super::*;
    use crate::math;
    use crate::units::MetalFraction;

    const REFERENCE_Z: [f64; 5] = [1e-4, 1e-3, 4e-3, 0.02, 0.03];

    /// Masses from 0.1 to 100 M☉, evenly in log mass.
    fn mass_sweep(n: u32) -> impl Iterator<Item = f64> {
        (0..n).map(move |i| math::exp10(-1.0 + 3.0 * f64::from(i) / f64::from(n - 1)))
    }

    #[test]
    fn a_solar_mass_starts_at_0_70_solar_luminosities_and_0_89_solar_radii() {
        let c = ZCoeffs::new(MetalFraction::new(0.02));
        let l = luminosity(SolarMasses::new(1.0), &c).value();
        let r = radius(SolarMasses::new(1.0), &c).value();
        assert!((l - 0.70).abs() < 0.01, "L = {l}");
        assert!((r - 0.89).abs() < 0.01, "R = {r}");
    }

    /// L rises with mass at every metallicity, and neither L nor R jumps: a Lipschitz bound in
    /// log–log over 2,000 steps from 0.1 to 100 M☉, and the jump detector of
    /// [`continuity`](super::super::continuity), which sees a jump of 10⁻⁴ dex.
    #[test]
    fn luminosity_rises_and_both_are_continuous_in_mass() {
        for z in REFERENCE_Z {
            let c = ZCoeffs::new(MetalFraction::new(z));
            let points: Vec<(f64, f64, f64)> = mass_sweep(2_000)
                .map(|m| {
                    let m = SolarMasses::new(m);
                    let l = luminosity(m, &c).value();
                    let r = radius(m, &c).value();
                    assert!(l > 0.0 && r > 0.0, "Z = {z}, M = {m:?}: L = {l}, R = {r}");
                    (math::log10(m.value()), math::log10(l), math::log10(r))
                })
                .collect();
            for pair in points.windows(2) {
                let ((m0, l0, r0), (m1, l1, r1)) = (pair[0], pair[1]);
                let step = m1 - m0;
                assert!(l1 > l0, "Z = {z}: L falls between log M = {m0} and {m1}");
                assert!((l1 - l0) < 6.0 * step, "Z = {z}: L jumps at log M = {m0}");
                assert!(
                    (r1 - r0).abs() < 3.0 * step,
                    "Z = {z}: R jumps at log M = {m0}"
                );
            }
            let log_masses: Vec<f64> = points.iter().map(|p| p.0).collect();
            let at = |log_m: f64| SolarMasses::new(math::exp10(log_m));
            let log_l = |log_m: f64| math::log10(luminosity(at(log_m), &c).value());
            let log_r = |log_m: f64| math::log10(radius(at(log_m), &c).value());
            assert_continuous_over(&format!("log L at Z = {z}"), log_l, &log_masses, 0.05, 1e-9);
            assert_continuous_over(&format!("log R at Z = {z}"), log_r, &log_masses, 0.05, 1e-9);
        }
    }

    /// Zero-age values from the published SSE code (see [`sse`](super) for the run: `lums(1)` of
    /// its `star` and the radius of its `hrdiag` at age 0), which evaluates the same fits.
    #[test]
    fn matches_the_published_sse_code() {
        let sse = [
            (0.02, 1.0, 0.697_716_569_145_151_8, 0.888_249_450_297_512_1),
            (0.02, 10.0, 5_551.885_929_612_93, 3.941_905_620_259_646),
            (
                1e-4,
                0.3,
                0.020_309_767_883_924_735,
                0.281_195_609_176_307_6,
            ),
            (0.03, 60.0, 527_473.711_971_112_6, 13.158_605_773_507_12),
        ];
        for (z, m, l_sse, r_sse) in sse {
            let c = ZCoeffs::new(MetalFraction::new(z));
            let l = luminosity(SolarMasses::new(m), &c).value();
            let r = radius(SolarMasses::new(m), &c).value();
            assert!(
                (l / l_sse - 1.0).abs() < 1e-12,
                "L at Z = {z}, M = {m}: {l} against {l_sse}"
            );
            assert!(
                (r / r_sse - 1.0).abs() < 1e-12,
                "R at Z = {z}, M = {m}: {r} against {r_sse}"
            );
        }
    }
}
