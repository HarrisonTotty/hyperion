//! The space-weathering maturity's time law, shared by the sim and the client
//! (decision-p14-t51d-weathering §1.4 and §2.1; plan 14, P14.T51.d).
//!
//! The maturity m is the background regolith's nanophase-iron (npFe⁰) content as a fraction of
//! the Moon's mature background's, per unit of `FeO_T`: 0 for fresh ground and 1 for the saturation
//! the Moon's mature background regolith reaches, the optical-maturity floor of an old lunar
//! surface. Ground exposed for a time t at a weathering rate k, the body's surface-reaching dose
//! rate relative to the Moon's present one (k = 1 for the Moon), matures as
//!
//! m(t) = 1 − exp(−k t ÷ `τ_L`),
//!
//! a first-order saturation, labelled: lunar optical-maturity profiles may fade otherwise, and the
//! Tycho and Copernicus bounds below are what it is held to (the ruling's Risks). `τ_L` is
//! [`LUNAR_MATURATION_TIME`]. Saturation is why both the rate and the maturity are needed: in
//! `f64`, m is exactly 1 past k t ÷ `τ_L` ≈ 37, so k cannot be recovered from m on an old surface.
//!
//! - [`body_maturity`] is a body's background maturity at its surface age, P14.T24.b's, which
//!   already carries its resurfacing (P14.T51.d's `weathering_rate` and `maturity`).
//! - [`maturity_after`] is ground exposed for a given time, such as a crater's ejecta, which
//!   R10.T1.c fades as min(`m_body`, `maturity_after`(`t_c`, k)).
//!
//! The darkening the maturity drives is R10.T1.d's (`material/weathering.rs`), not this module's.
//!
//! # Determinism
//!
//! Pure `f64` arithmetic and [`math::exp_m1`]; no draw, so no domain tag. The result at zero
//! exposure or zero rate is +0, never −0, as the field header's reserved `maturity` part requires.
//!
//! # Not on the record yet
//!
//! Nothing on the record calls this module at generator version 22: P14.T54.b (23 → 24) fills the
//! header's `maturity` from P14.T51.d's `hooks/modifiers.rs`, which calls it, and deletes the sim's
//! tripwire (`modifiers_nothing_on_the_record_reads_the_modifiers_yet`) that holds the line.

use hyperion_base::math;
use hyperion_base::units::Gigayears;

/// The lunar maturation time `τ_L`: 0.3277 Gyr, the time over which the Moon's ground matures by a
/// factor of e towards its mature background at the Moon's weathering rate (k = 1).
///
/// **The fit** (decision-p14-t51d-weathering §2.1). Grier et al. 2001 (JGR 106, 32847, abstract
/// read) class lunar rays by Clementine's optical-maturity parameter into craters as young as or
/// younger than Tycho, "inferred age of ~109 Myr", whose ejecta are immature, and craters older
/// than Copernicus, "~810 Myr", whose ejecta are mature. In m's terms, with X the lunar mature
/// background's Morris index `I_s/FeO` and Morris's classes, immature 0–29.0 and mature from 60.0
/// (Morris 1976, Proc. LPSC 7, 315, abstract read, which defines the index; Morris 1978, Proc.
/// LPSC 9, 2287, pp. 2289–2290, read):
///
/// - m(109 Myr) < 30 ÷ X, so `τ_L` > 0.109 Gyr ÷ −ln(1 − 30 ÷ X);
/// - m(810 Myr) > 60 ÷ X, so `τ_L` < 0.810 Gyr ÷ −ln(1 − 60 ÷ X).
///
/// At X = 75, a stand-in inside the mature class's 60 to above 100 (Morris 1978, pp. 2289–2290,
/// read), the bounds are 0.4 and 0.8 and give `τ_L` in
/// (0.2134, 0.5033) Gyr. `τ_L` is their geometric centre, √(0.2134 × 0.5033) = 0.3277 Gyr, which
/// keeps the same ratio margin, 1.536, on both sides: m(109 Myr) = 0.283 and m(810 Myr) = 0.916.
/// The fit holds both bounds for any X from about 66 to 106, outside which the Copernicus bound
/// (below) or Tycho's (above) passes it; the module's tests hold it at X = 75.
pub const LUNAR_MATURATION_TIME: Gigayears = Gigayears::new(0.3277);

/// The maturity, dimensionless in [+0, 1], of ground exposed for `exposure` at the weathering rate
/// `weathering_rate` (k, dimensionless, the Moon's 1): 1 − exp(−k t ÷ `τ_L`) (the [module](self)
/// documentation), with `τ_L` [`LUNAR_MATURATION_TIME`].
///
/// It is +0 at zero exposure or zero rate (never −0, whatever the sign of a zero exposure), never
/// falls as either grows, is below 1 while k t ÷ `τ_L` is under about 36.7, where `f64` can hold
/// 1 − exp(−x) apart from 1, and is exactly 1 beyond. It is written as −expm1(−x), which keeps a
/// young surface's small maturity to full precision.
///
/// # Panics
///
/// If the exposure or the rate is not finite and non-negative: the sim validates both before the
/// call, so a NaN or a negative value here is a bug.
///
/// # Examples
///
/// Tycho's ejecta, 109 Myr old, are immature, and Copernicus's, 810 Myr old, near the background;
/// a body weathered twice as fast matures in half the time.
///
/// ```
/// use hyperion_base::units::Gigayears;
/// use hyperion_surface::material::maturity_after;
///
/// assert!(maturity_after(Gigayears::new(0.109), 1.0) < 0.4);
/// assert!(maturity_after(Gigayears::new(0.810), 1.0) > 0.8);
/// let twice = maturity_after(Gigayears::new(0.05), 2.0);
/// assert!((twice - maturity_after(Gigayears::new(0.1), 1.0)).abs() < 1e-15);
/// assert!(maturity_after(Gigayears::ZERO, 1.0).is_sign_positive());
/// ```
#[must_use]
pub fn maturity_after(exposure: Gigayears, weathering_rate: f64) -> f64 {
    let t = exposure.value();
    assert!(
        t.is_finite() && t >= 0.0,
        "an exposure must be finite and non-negative, got {t} Gyr"
    );
    assert!(
        weathering_rate.is_finite() && weathering_rate >= 0.0,
        "a weathering rate must be finite and non-negative, got {weathering_rate}"
    );
    let x = weathering_rate * t / LUNAR_MATURATION_TIME.value();
    // A zero exposure of either sign, or a zero rate, is fresh ground: +0, which −expm1(−0) would
    // give as −0 for a negative zero.
    if x > 0.0 { -math::exp_m1(-x) } else { 0.0 }
}

/// A body's background maturity, dimensionless in [+0, 1], at its surface age `surface_age`
/// (P14.T24.b's, which already carries its resurfacing) and weathering rate `weathering_rate`:
/// [`maturity_after`] of the surface age (decision-p14-t51d-weathering §2.1).
///
/// A later steady-state factor, the balance of weathering against gardening, would multiply it;
/// that is a value change, and R10 never recomputes it.
///
/// # Panics
///
/// As [`maturity_after`].
#[must_use]
pub fn body_maturity(surface_age: Gigayears, weathering_rate: f64) -> f64 {
    maturity_after(surface_age, weathering_rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_testkit::float::assert_same_bits;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// The lunar mature background's `I_s/FeO`, X, at which the fit is held: 75, a stand-in inside
    /// Morris 1978's mature range (see [`LUNAR_MATURATION_TIME`]).
    const MATURE_BACKGROUND_IS_FEO: f64 = 75.0;

    /// Morris's upper edge of the immature class and lower edge of the mature one, `I_s/FeO`
    /// (Morris 1978, pp. 2289–2290, read).
    const IMMATURE_BELOW: f64 = 30.0;
    const MATURE_ABOVE: f64 = 60.0;

    /// Tycho's and Copernicus's ages, Gyr (Grier et al. 2001, abstract read).
    const TYCHO: f64 = 0.109;
    const COPERNICUS: f64 = 0.810;

    #[test]
    fn is_fresh_at_zero_exposure_or_rate() {
        for t in [0.0, -0.0] {
            for k in [0.0, 1.0, 1e3] {
                assert_same_bits(maturity_after(Gigayears::new(t), k), 0.0);
            }
        }
        for t in [0.0, 1e-9, 1.0, 13.8] {
            assert_same_bits(maturity_after(Gigayears::new(t), 0.0), 0.0);
            assert_same_bits(maturity_after(Gigayears::new(t), -0.0), 0.0);
        }
    }

    #[test]
    fn never_falls_and_stays_in_the_unit_interval() {
        for k in [1e-6, 0.01, 0.5, 1.0, 8.6, 1e3] {
            let mut previous = 0.0;
            for i in 0..=2_000 {
                let t = 14.0 * f64::from(i) / 2_000.0;
                let m = maturity_after(Gigayears::new(t), k);
                assert!(
                    m.is_finite() && (0.0..=1.0).contains(&m),
                    "k {k}, t {t}: {m}"
                );
                assert!(m >= previous, "k {k}: {m} at {t} Gyr after {previous}");
                previous = m;
            }
        }
        // And in the rate, at a fixed exposure.
        let mut previous = 0.0;
        for i in 0..=1_000 {
            let k = 50.0 * f64::from(i) / 1_000.0;
            let m = maturity_after(Gigayears::new(0.1), k);
            assert!(m >= previous, "{m} at k {k} after {previous}");
            previous = m;
        }
    }

    #[test]
    fn is_below_one_while_f64_can_hold_it() {
        // x = k t ÷ τ_L; 1 − e^(−x) is distinct from 1 up to x ≈ 36.7.
        for x in [1e-300, 1e-12, 1e-3, 0.5, 1.0, 10.0, 30.0, 36.0] {
            let t = x * LUNAR_MATURATION_TIME.value();
            let m = maturity_after(Gigayears::new(t), 1.0);
            assert!(m < 1.0, "x {x}: {m}");
            assert!(m > 0.0, "x {x}: {m}");
        }
        // A tiny exposure keeps its precision: 1 − e^(−x) ≈ x.
        let x = 1e-12;
        let m = maturity_after(Gigayears::new(x * LUNAR_MATURATION_TIME.value()), 1.0);
        assert!((m / x - 1.0).abs() < 1e-11, "{m}");
        // Saturated: the Moon's 4 Gyr is 12 τ_L, Mercury's k ≈ 9 about 100.
        assert!(maturity_after(Gigayears::new(4.0), 1.0) > 0.99999);
        assert_same_bits(maturity_after(Gigayears::new(4.0), 9.0), 1.0);
    }

    #[test]
    fn holds_tycho_immature_and_copernicus_mature() {
        let x = MATURE_BACKGROUND_IS_FEO;
        let tycho = maturity_after(Gigayears::new(TYCHO), 1.0);
        let copernicus = maturity_after(Gigayears::new(COPERNICUS), 1.0);
        assert!(tycho < IMMATURE_BELOW / x, "Tycho {tycho}");
        assert!(copernicus > MATURE_ABOVE / x, "Copernicus {copernicus}");
        // The fit: τ_L the geometric centre of the bounds, to the stated four figures.
        let low = TYCHO / -math::ln_1p(-IMMATURE_BELOW / x);
        let high = COPERNICUS / -math::ln_1p(-MATURE_ABOVE / x);
        let centre = (low * high).sqrt();
        let tau = LUNAR_MATURATION_TIME.value();
        assert!((low..high).contains(&tau), "{low} < {tau} < {high}");
        assert!((tau - centre).abs() < 5e-5, "{tau} against {centre}");
        eprintln!(
            "τ_L {tau} Gyr in ({low:.4}, {high:.4}), centre {centre:.6}: \
             m(Tycho) {tycho:.4}, m(Copernicus) {copernicus:.4}"
        );
    }

    #[test]
    fn a_body_s_maturity_is_its_surface_s_exposure() {
        for (t, k) in [(0.0, 1.0), (0.1, 0.5), (3.9, 1.0), (4.0, 8.6), (1e-6, 1e3)] {
            assert_same_bits(
                body_maturity(Gigayears::new(t), k),
                maturity_after(Gigayears::new(t), k),
            );
        }
    }

    // The refusals of a negative or NaN exposure and of an infinite rate are in the crate's
    // `tests/panics.rs` (`mod maturity`), where every `should_panic` test lives.
}
