//! Gravitational-wave inspiral of a binary, after Peters (1964, Phys. Rev. 136, B1224).
//!
//! A binary of point masses loses orbital energy and angular momentum to gravitational waves.
//! Peters's secular equations (his eqs. 5.6 and 5.7) are
//!
//! ```text
//! da/dt = −(64/5) G³ m₁ m₂ (m₁ + m₂) ÷ (c⁵ a³ (1 − e²)^(7/2)) × (1 + 73/24 e² + 37/96 e⁴)
//! de/dt = −(304/15) e G³ m₁ m₂ (m₁ + m₂) ÷ (c⁵ a⁴ (1 − e²)^(5/2)) × (1 + 121/304 e²)
//! ```
//!
//! With β = (64/5) G³ m₁ m₂ (m₁ + m₂) ÷ c⁵, a circular orbit of separation a₀ merges after
//! Tc = a₀⁴ ÷ 4β (eq. 5.10). An eccentric one follows a(e) = c₀ e^(12/19) ÷ (1 − e²) ×
//! (1 + 121/304 e²)^(870/2299) (eq. 5.11) and merges after
//!
//! ```text
//! T = (12/19) (c₀⁴ ÷ β) ∫₀^e₀ e^(29/19) (1 + 121/304 e²)^(1181/2299) ÷ (1 − e²)^(3/2) de   (eq. 5.14)
//! ```
//!
//! This module writes T = Tc × F(e₀) and takes F by a fixed quadrature (plan 11, P11.T3.a). The
//! integrand's endpoint singularity at e = 1 is removed by the substitution t = e ÷ √(1 − e²),
//! under which de ÷ (1 − e²)^(3/2) = dt and the integrand is bounded and smooth; its fractional
//! power at t = 0 is removed by t = t₁ u¹⁹ on the first panel. The panels are fixed: `u` on
//! `[0, 1]` for t up to ¼, t itself on `[¼, 4]`, and ln t in panels of at most 3 beyond, each by
//! the 32-point Gauss–Legendre rule. F agrees with a direct Runge–Kutta integration of the two
//! equations above to better than 10⁻⁹, the unit test's bound (about 10⁻¹⁴ where the
//! integration's own step error allows), and tends to Peters's (768/425) (1 − e₀²)^(7/2) as
//! e₀ → 1, slowly: 1 − F ÷ that ≈ 2.06 √(1 − e₀).
//!
//! Masses enter through the nominal solar mass parameter GM☉ ([`GM_SUN`]), so G itself, known only
//! to 2 × 10⁻⁵, never does.

use super::Eccentricity;
use crate::galaxy::quad::gl32;
use crate::math;
use crate::units::consts::{GM_SUN, SPEED_OF_LIGHT};
use crate::units::{Metres, Seconds, SolarMasses, Years};

/// 121/304, the e² coefficient of Peters's de/dt and of eq. 5.11.
const E2_COEFFICIENT: f64 = 121.0 / 304.0;

/// 1181/2299, the exponent of (1 + 121/304 e²) in the integrand of eq. 5.14.
const INTEGRAND_EXPONENT: f64 = 1181.0 / 2299.0;

/// −4 × 870/2299, the exponent of (1 + 121/304 e²) in (c₀ ÷ a₀)⁴.
const C0_EXPONENT: f64 = -3480.0 / 2299.0;

/// Where the first panel, in u with t = t₁ u¹⁹, ends in t.
const FIRST_PANEL_END: f64 = 0.25;

/// Where the second panel, in t, ends; beyond it the panels are in ln t.
const SECOND_PANEL_END: f64 = 4.0;

/// The widest panel in ln t. Six panels cover every eccentricity below 1 in `f64`.
const LOG_PANEL_WIDTH: f64 = 3.0;

/// The time for a binary of masses `m1` and `m2` on an orbit of semi-major axis `a` and
/// eccentricity `e` to merge by gravitational-wave emission alone (Peters 1964, eq. 5.14).
///
/// Point masses: the time until the separation reaches zero, not until the stars touch. Since the
/// time left goes as a⁴, contact comes only decades earlier for two white dwarfs (about 10–60
/// years for 0.6–0.9 M☉ each, with radii from Nauenberg's 1972 mass–radius relation), which is
/// nothing beside the delays this function serves.
///
/// # Panics
///
/// If a mass or the semi-major axis is not finite and positive.
///
/// # Examples
///
/// ```
/// use hyperion_sim::orbit::{Eccentricity, peters_merger_time};
/// use hyperion_sim::units::{Metres, SolarMasses};
///
/// // The Hulse–Taylor pulsar: 1.44 and 1.39 M☉, a = 1.95 × 10⁹ m, e = 0.617. About 300 Myr left.
/// let t = peters_merger_time(
///     SolarMasses::new(1.44),
///     SolarMasses::new(1.39),
///     Metres::new(1.95e9),
///     Eccentricity::new(0.617)?,
/// );
/// assert!((2.9e8..3.2e8).contains(&t.value()));
/// # Ok::<(), hyperion_sim::orbit::BuildOrbitError>(())
/// ```
#[must_use]
pub fn peters_merger_time(m1: SolarMasses, m2: SolarMasses, a: Metres, e: Eccentricity) -> Years {
    let a = a.value();
    assert!(
        a.is_finite() && a > 0.0,
        "the semi-major axis {a} m must be finite and positive"
    );
    let circular = (a * a) * (a * a) / (4.0 * beta(m1, m2));
    Years::from(Seconds::new(circular * eccentricity_factor(e.value())))
}

/// The separation of a circular binary of masses `m1` and `m2` that merges by gravitational-wave
/// emission after `t`: a = (4βt)^(1/4), the inverse of [`peters_merger_time`] at e = 0.
///
/// This is what a Type Ia entry that has drawn its delay needs (plan 09): the separation after the
/// common envelope from which the inspiral takes the rest of the delay.
///
/// # Panics
///
/// If a mass is not finite and positive, or `t` is negative or not finite.
///
/// # Examples
///
/// ```
/// use hyperion_sim::orbit::peters_separation_for;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// // Two 0.7 M☉ white dwarfs a thousand years before they merge are about 32,000 km apart.
/// let a = peters_separation_for(SolarMasses::new(0.7), SolarMasses::new(0.7), Years::new(1e3));
/// assert!((3.1e7..3.3e7).contains(&a.value()));
/// ```
#[must_use]
pub fn peters_separation_for(m1: SolarMasses, m2: SolarMasses, t: Years) -> Metres {
    let seconds = Seconds::from(t).value();
    assert!(
        seconds.is_finite() && seconds >= 0.0,
        "the time to merger {seconds} s must be finite and not negative"
    );
    Metres::new((4.0 * beta(m1, m2) * seconds).sqrt().sqrt())
}

/// Peters's β = (64/5) G³ m₁ m₂ (m₁ + m₂) ÷ c⁵, m⁴ s⁻¹.
fn beta(m1: SolarMasses, m2: SolarMasses) -> f64 {
    let (m1, m2) = (m1.value(), m2.value());
    assert!(
        m1.is_finite() && m1 > 0.0 && m2.is_finite() && m2 > 0.0,
        "the masses {m1} and {m2} M☉ must be finite and positive"
    );
    let (gm1, gm2) = (GM_SUN * m1, GM_SUN * m2);
    let c2 = SPEED_OF_LIGHT * SPEED_OF_LIGHT;
    64.0 / 5.0 * gm1 * gm2 * (gm1 + gm2) / (c2 * c2 * SPEED_OF_LIGHT)
}

/// F(e) = T ÷ Tc, the eccentric merger time in units of the circular one at the same
/// semi-major axis: 1 at e = 0, falling as (768/425) (1 − e²)^(7/2) towards e = 1.
///
/// F = (48/19) (c₀ ÷ a₀)⁴ I(e) with I the integral of eq. 5.14 and
/// (c₀ ÷ a₀)⁴ = (1 − e²)⁴ e^(−48/19) (1 + 121/304 e²)^(−3480/2299). On the first panel alone
/// (t₀ ≤ ¼, e below 0.243) the powers of e cancel analytically, which keeps e = 0 exact.
fn eccentricity_factor(e: f64) -> f64 {
    let one_minus_e2 = (1.0 - e) * (1.0 + e);
    let t0 = e / one_minus_e2.sqrt();
    let enhancement = math::powf(1.0 + E2_COEFFICIENT * e * e, C0_EXPONENT);
    if t0 <= FIRST_PANEL_END {
        return 48.0 / 19.0 * math::powf(one_minus_e2, 52.0 / 19.0) * enhancement * first_panel(t0);
    }
    let mut integral = math::powf(FIRST_PANEL_END, 48.0 / 19.0) * first_panel(FIRST_PANEL_END)
        + gl32(integrand, FIRST_PANEL_END, t0.min(SECOND_PANEL_END));
    if t0 > SECOND_PANEL_END {
        let (start, end) = (math::ln(SECOND_PANEL_END), math::ln(t0));
        // t₀ < 2²⁷ for every e below 1 in f64, so ln t₀ − ln 4 < 17.3 and there are at most 6
        // panels; none when ln t₀ rounds to ln 4, which leaves that empty panel out.
        let panels = ((end - start) / LOG_PANEL_WIDTH).ceil();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole number from 0 to 6, as the comment above shows"
        )]
        let count = panels as u32;
        let width = (end - start) / panels;
        let mut left = start;
        for panel in 1..=count {
            let right = if panel < count {
                start + f64::from(panel) * width
            } else {
                end
            };
            integral += gl32(
                |s| {
                    let t = math::exp(s);
                    integrand(t) * t
                },
                left,
                right,
            );
            left = right;
        }
    }
    48.0 / 19.0
        * math::powf(one_minus_e2, 4.0)
        * math::powf(e, -48.0 / 19.0)
        * enhancement
        * integral
}

/// eq. 5.14's integrand in t = e ÷ √(1 − e²): e^(29/19) (1 + 121/304 e²)^(1181/2299).
fn integrand(t: f64) -> f64 {
    let e2 = t * t / (1.0 + t * t);
    math::powf(e2, 29.0 / 38.0) * math::powf(1.0 + E2_COEFFICIENT * e2, INTEGRAND_EXPONENT)
}

/// ∫₀^t₁ integrand dt ÷ t₁^(48/19), by t = t₁ u¹⁹: 19 ∫₀¹ u⁴⁷ (1 + t²)^(−29/38)
/// (1 + 121/304 e²)^(1181/2299) du, analytic in u.
fn first_panel(t1: f64) -> f64 {
    gl32(
        |u| {
            let t = t1 * math::powi(u, 19);
            let one_plus_t2 = 1.0 + t * t;
            let e2 = t * t / one_plus_t2;
            19.0 * math::powi(u, 47)
                * math::powf(one_plus_t2, -29.0 / 38.0)
                * math::powf(1.0 + E2_COEFFICIENT * e2, INTEGRAND_EXPONENT)
        },
        0.0,
        1.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbit::{KeplerElements, Orientation};
    use crate::units::{GravitationalParameter, Radians};

    /// F by a direct fourth-order Runge–Kutta integration of Peters's da/dt and de/dt, in units
    /// with β = 1 and a₀ = 1, so Tc = ¼: an independent route to the same number.
    fn factor_by_integration(e0: f64) -> f64 {
        let rates = |a: f64, e: f64| {
            let one_minus_e2 = (1.0 - e) * (1.0 + e);
            let e2 = e * e;
            let da = -(1.0 + 73.0 / 24.0 * e2 + 37.0 / 96.0 * e2 * e2)
                / (a * a * a * math::powf(one_minus_e2, 3.5));
            let de = -19.0 / 12.0 * e * (1.0 + E2_COEFFICIENT * e2)
                / (a * a * a * a * math::powf(one_minus_e2, 2.5));
            (da, de)
        };
        let (mut a, mut e, mut t) = (1.0_f64, e0, 0.0_f64);
        while a > 2e-3 {
            let (k1a, k1e) = rates(a, e);
            let dt = 1e-4 * a / k1a.abs();
            let (k2a, k2e) = rates(a + 0.5 * dt * k1a, e + 0.5 * dt * k1e);
            let (k3a, k3e) = rates(a + 0.5 * dt * k2a, e + 0.5 * dt * k2e);
            let (k4a, k4e) = rates(a + dt * k3a, e + dt * k3e);
            a += dt / 6.0 * (k1a + 2.0 * k2a + 2.0 * k3a + k4a);
            e = (e + dt / 6.0 * (k1e + 2.0 * k2e + 2.0 * k3e + k4e)).max(0.0);
            t += dt;
        }
        // What is left, from a nearly circular orbit: a⁴/4 × F(e) with F ≈ 1 − 3.651 e².
        (t + 0.25 * (a * a) * (a * a) * (1.0 - 3.651 * e * e)) / 0.25
    }

    #[test]
    fn the_quadrature_matches_a_direct_integration_of_peters_equations() {
        for e0 in [0.01, 0.1, 0.24, 0.3, 0.5, 0.7, 0.9, 0.97, 0.99] {
            let quadrature = eccentricity_factor(e0);
            let integrated = factor_by_integration(e0);
            assert!(
                ((quadrature - integrated) / integrated).abs() < 1e-9,
                "e = {e0}: {quadrature:e} by quadrature, {integrated:e} by integration"
            );
        }
    }

    #[test]
    fn the_factor_is_one_for_a_circle_and_continuous_across_its_panels() {
        assert!((eccentricity_factor(0.0) - 1.0).abs() < 1e-15);
        // The small-e expansion, F ≈ 1 − 3.651 e², from the same integral expanded by hand.
        let small = 1e-4;
        assert!((eccentricity_factor(small) - (1.0 - 3.651_163 * small * small)).abs() < 1e-13);
        // Either side of the panel switches: t₀ = ¼ at e = 0.2425, t₀ = 4 at e = 0.9701.
        // Two units in the last place either side move F by about 10⁻¹³ of itself at e = 0.97.
        for edge in [0.25_f64, 4.0] {
            let e_edge = edge / (1.0 + edge * edge).sqrt();
            let below = eccentricity_factor(e_edge * (1.0 - 4e-16));
            let above = eccentricity_factor(e_edge * (1.0 + 4e-16));
            assert!(((above - below) / below).abs() < 1e-12, "at t₀ = {edge}");
        }
    }

    #[test]
    fn a_very_eccentric_orbit_follows_peters_asymptote() {
        // Peters (1964), below eq. 5.14: T → (768/425) Tc (1 − e₀²)^(7/2) as e₀ → 1.
        // The approach is slow, 1 − T ÷ T_asymptote ≈ 2.06 √(1 − e₀) (measured here from 10⁻³ to
        // 10⁻⁸), which is why a fit such as Mandel's needs its e¹⁰⁰⁰ term.
        let mut previous = 0.0;
        for e0 in [
            0.999,
            0.999_9,
            0.999_99,
            0.999_999,
            0.999_999_9,
            0.999_999_99,
        ] {
            let one_minus_e2: f64 = (1.0 - e0) * (1.0 + e0);
            let asymptote = 768.0 / 425.0 * math::powf(one_minus_e2, 3.5);
            let ratio = eccentricity_factor(e0) / asymptote;
            let scaled_gap = (1.0 - ratio) / (1.0 - e0).sqrt();
            assert!(ratio > previous && ratio < 1.0, "e = {e0}: ratio {ratio}");
            assert!(
                (1.9..2.1).contains(&scaled_gap),
                "e = {e0}: gap {scaled_gap} √(1 − e)"
            );
            previous = ratio;
        }
    }

    #[test]
    fn the_factor_agrees_with_mandels_fit() {
        // Mandel (2021, RNAAS 5, 223, eq. 5): T ≈ Tc (1 − e²)^(7/2) (1 + 0.27 e¹⁰ + 0.33 e²⁰ +
        // 0.2 e¹⁰⁰⁰), within 3% for e₀ from 0 to 0.99999.
        let near_one = [0.998, 0.999, 0.9995, 0.9999, 0.999_95, 0.999_99];
        for e in (0..=100)
            .map(|k| 0.99 * f64::from(k) / 100.0)
            .chain(near_one)
        {
            let fit = math::powf((1.0 - e) * (1.0 + e), 3.5)
                * (1.0
                    + 0.27 * math::powi(e, 10)
                    + 0.33 * math::powi(e, 20)
                    + 0.2 * math::powi(e, 1000));
            let factor = eccentricity_factor(e);
            assert!(
                (fit / factor - 1.0).abs() < 0.03,
                "e = {e}: fit {fit:e}, {factor:e}"
            );
        }
    }

    #[test]
    fn a_separation_and_a_merger_time_invert_each_other() {
        let (m1, m2) = (SolarMasses::new(1.35), SolarMasses::new(0.62));
        for years in [1.0, 1e3, 1e6, 1e9, 1.3e10] {
            let a = peters_separation_for(m1, m2, Years::new(years));
            let back = peters_merger_time(m1, m2, a, Eccentricity::CIRCULAR);
            assert!(
                (back.value() / years - 1.0).abs() < 1e-14,
                "{years} yr: {back:?}"
            );
        }
        assert!(peters_separation_for(m1, m2, Years::new(0.0)).value().abs() < 1e-300);
        // Symmetric in the masses.
        let a = Metres::new(3e9);
        let e = Eccentricity::new(0.4).unwrap();
        assert!(
            (peters_merger_time(m1, m2, a, e).value() / peters_merger_time(m2, m1, a, e).value()
                - 1.0)
                .abs()
                < 1e-15
        );
    }

    #[test]
    fn two_white_dwarfs_a_thousand_years_before_merging_are_80_to_100_s_apart() {
        // The brainstorm's "Events in time": for a Type Ia by merger, "two white dwarfs spiralling
        // together, 80–100 s apart in period a thousand years before the end". A merger Type Ia's
        // progenitors are near or above the Chandrasekhar mass together: pairs of 0.6–0.9 M☉ each,
        // 83–98 s here.
        for (m1, m2) in [(0.7, 0.7), (0.8, 0.6), (0.8, 0.8), (0.9, 0.8), (0.9, 0.9)] {
            let (m1, m2) = (SolarMasses::new(m1), SolarMasses::new(m2));
            let a = peters_separation_for(m1, m2, Years::new(1_000.0));
            let orbit = KeplerElements::from_semi_major_axis(
                a,
                GravitationalParameter::from_solar_masses(m1 + m2),
                Eccentricity::CIRCULAR,
                Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap(),
                Radians::ZERO,
            )
            .unwrap();
            let period = orbit.period().value();
            assert!(
                (80.0..=100.0).contains(&period),
                "{m1:?} + {m2:?}: a = {a:?}, period {period} s"
            );
        }
    }

    /// Peters's (1964, eq. 5.10) circular merger time, T = (5 ÷ 256) c⁵ a⁴ ÷ (G³ m₁ m₂ (m₁ +
    /// m₂)), written out from the paper: the eccentric quadrature at e = 0 and the inverse agree
    /// with it to rounding.
    #[test]
    fn a_circular_orbit_merges_at_peters_closed_form() {
        for (m1, m2, a) in [(0.6, 0.6, 3.0e7), (1.4, 1.3, 2.0e9), (10.0, 0.08, 7.0e10)] {
            let (gm1, gm2) = (GM_SUN * m1, GM_SUN * m2);
            let c5 = math::powf(SPEED_OF_LIGHT, 5.0);
            let closed = 5.0 / 256.0 * c5 * math::powf(a, 4.0) / (gm1 * gm2 * (gm1 + gm2));
            let (s1, s2) = (SolarMasses::new(m1), SolarMasses::new(m2));
            let t = Seconds::from(peters_merger_time(
                s1,
                s2,
                Metres::new(a),
                Eccentricity::CIRCULAR,
            ));
            assert!(
                (t.value() / closed - 1.0).abs() < 1e-12,
                "{t:?} against {closed} s"
            );
            let back = peters_separation_for(s1, s2, Years::from(Seconds::new(closed)));
            assert!(
                (back.value() / a - 1.0).abs() < 1e-12,
                "{back:?} against {a} m"
            );
        }
    }

    #[test]
    fn the_hulse_taylor_pulsar_merges_in_about_three_hundred_million_years() {
        // Weisberg and Huang (2016, ApJ 829, 55): m_p = 1.438, m_c = 1.390 M☉, P_b = 0.322 997 d,
        // e = 0.617 134. Kepler's third law gives a = 1.950 × 10⁹ m; the merger time is about
        // 3.0 × 10⁸ yr.
        let (m1, m2) = (SolarMasses::new(1.438), SolarMasses::new(1.390));
        let orbit = KeplerElements::from_period(
            Seconds::new(0.322_997_448_918 * 86_400.0),
            GravitationalParameter::from_solar_masses(m1 + m2),
            Eccentricity::new(0.617_134).unwrap(),
            Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap(),
            Radians::ZERO,
        )
        .unwrap();
        assert!((orbit.semi_major_axis().value() / 1.950e9 - 1.0).abs() < 2e-3);
        let t = peters_merger_time(m1, m2, orbit.semi_major_axis(), orbit.eccentricity());
        assert!((2.9e8..3.1e8).contains(&t.value()), "{t:?}");
    }

    #[test]
    #[should_panic(expected = "must be finite and positive")]
    fn a_massless_star_is_refused() {
        let _ = peters_separation_for(SolarMasses::ZERO, SolarMasses::new(1.0), Years::new(1.0));
    }
}
