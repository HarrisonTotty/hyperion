//! The main sequence: its timescales, its end points, and luminosity and radius along it (plan 06,
//! P06.T5; Hurley, Pols and Tout 2000, MNRAS 315, 543, "HPT", section 5.1 and 5.1.1).
//!
//! The `pub(crate)` functions take and return unit newtypes: masses in M☉, times in Myr from the
//! zero-age main sequence, luminosities in L☉ and radii in R☉, HPT's units. The private helpers
//! that evaluate one coefficient take the mass `m` as a bare `f64` in M☉, HPT's symbol, and return
//! the dimensionless coefficient. Every function is a closed form in the mass it is given, which
//! the track integrator (P06.T10) holds as the star's effective initial mass.
//!
//! The published SSE code, which the paper documents, settles misprints; where the printed form is
//! a different fit rather than a misprint, the code's form is used only when the difference
//! exceeds the tolerances P06.T12.b validates the backbone to. The doc comment of each function
//! concerned names the case.

// The track integrator of P06.T10 is the first caller outside tests.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the track integrator of P06.T10 is the first caller"
    )
)]

use crate::math;
use crate::units::{Megayears, SolarLuminosities, SolarMasses, SolarRadii};

use super::PhasePoint;
use super::coeffs::ZCoeffs;
use super::zams;

/// ε of HPT equation 15: the hook's smooth transition takes the last 1% of `t_hook`.
const HOOK_EPSILON: f64 = 0.01;

/// c₁ of HPT equation 9a.
const C1: f64 = -8.672_073e-2;

/// c₂ of HPT equation 10.
const C2: f64 = 9.301_992;

/// c₃ of HPT equation 10.
const C3: f64 = 4.637_345;

/// The time from the zero-age main sequence to the base of the giant branch (HPT equation 4):
/// (a1 + a2 M⁴ + a3 M^5.5 + M⁷) ÷ (a4 M² + a5 M⁷), Myr.
///
/// For stars above `M_FGB` it is the time to helium ignition, the end of the Hertzsprung gap.
#[must_use]
pub(crate) fn t_bgb(m: SolarMasses, c: &ZCoeffs) -> Megayears {
    Megayears::new(t_bgb_myr(m.value(), c))
}

/// [`t_bgb`] in Myr for a mass `m` in M☉.
#[must_use]
fn t_bgb_myr(m: f64, c: &ZCoeffs) -> f64 {
    let m2 = m * m;
    let m4 = m2 * m2;
    let m7 = m4 * m2 * m;
    (c.a(1) + c.a(2) * m4 + c.a(3) * m4 * m * m.sqrt() + m7) / (c.a(4) * m2 + c.a(5) * m7)
}

/// The fraction µ of `t_bgb` at which the hook ends (HPT equation 7):
/// max(0.5, 1.0 − 0.01 max(a6 ÷ M^a7, a8 + a9 ÷ M^a10)).
#[must_use]
fn hook_fraction(m: f64, c: &ZCoeffs) -> f64 {
    let first = c.a(6) / math::powf(m, c.a(7));
    let second = c.a(8) + c.a(9) / math::powf(m, c.a(10));
    0.5_f64.max(1.0 - 0.01 * first.max(second))
}

/// The fraction x of `t_bgb` that the main sequence lasts at least (HPT equation 6), in the
/// published SSE code's form: max(0.95, max(0.95 − (10/3)(Z − 0.01), min(0.99, 0.98 − (100/7)
/// (Z − 0.001)))).
///
/// The paper prints a different fit, max(0.95, min(0.95 − 0.03 (ζ + 0.30103), 0.99)). The two
/// agree for Z ≤ 0.0003, at Z = 0.001 and for Z ≥ 0.01; between 0.001 and 0.01 the code's x is
/// larger, by up to 0.008 near Z = 0.004 (0.970 against 0.962, 0.8% of the main-sequence
/// lifetime), and between 0.0003 and 0.001 smaller, by up to 0.002. At Z = 0.004 the printed form
/// moves late main-sequence luminosities by 0.02–0.06 dex against the code, beyond the 0.02 dex
/// P06.T12.b validates to (where a systematic excess is to be fixed in the formulae), so the
/// code's form is used, pending the owner's confirmation.
#[must_use]
fn ms_fraction(c: &ZCoeffs) -> f64 {
    let z = c.z().value();
    0.95_f64.max(
        (0.95 - (10.0 / 3.0) * (z - 0.01)).max(0.99_f64.min(0.98 - (100.0 / 7.0) * (z - 0.001))),
    )
}

/// `t_hook` and `t_MS` in Myr for a mass `m` in M☉ (HPT equations 5–7): µ `t_bgb`, and
/// max(`t_hook`, x `t_bgb`).
#[must_use]
fn timescales_myr(m: f64, c: &ZCoeffs) -> (f64, f64) {
    let t_bgb = t_bgb_myr(m, c);
    let t_hook = hook_fraction(m, c) * t_bgb;
    (t_hook, t_hook.max(ms_fraction(c) * t_bgb))
}

/// The time at which the main-sequence hook ends, `t_hook` = µ `t_bgb` (HPT equation 7), Myr.
#[must_use]
pub(crate) fn t_hook(m: SolarMasses, c: &ZCoeffs) -> Megayears {
    Megayears::new(timescales_myr(m.value(), c).0)
}

/// The main-sequence lifetime, max(`t_hook`, x `t_bgb`) (HPT equation 5), Myr.
#[must_use]
pub(crate) fn t_ms(m: SolarMasses, c: &ZCoeffs) -> Megayears {
    Megayears::new(timescales_myr(m.value(), c).1)
}

/// The luminosity at the end of the main sequence (HPT equation 8):
/// (a11 M³ + a12 M⁴ + a13 M^(a16 + 1.8)) ÷ (a14 + a15 M⁵ + M^a16).
#[must_use]
pub(crate) fn l_tms(m: SolarMasses, c: &ZCoeffs) -> SolarLuminosities {
    let m = m.value();
    let m3 = m * m * m;
    let m_a16 = math::powf(m, c.a(16));
    SolarLuminosities::new(
        (c.a(11) * m3 + c.a(12) * m3 * m + c.a(13) * m_a16 * math::powf(m, 1.8))
            / (c.a(14) + c.a(15) * m3 * m * m + m_a16),
    )
}

/// HPT equation 9a, the terminal radius in R☉ at low mass `m` in M☉:
/// (a18 + a19 M^a21) ÷ (a20 + M^a22).
#[must_use]
fn r_tms_low(m: f64, c: &ZCoeffs) -> f64 {
    (c.a(18) + c.a(19) * math::powf(m, c.a(21))) / (c.a(20) + math::powf(m, c.a(22)))
}

/// HPT equation 9b, the terminal radius in R☉ at high mass `m` in M☉:
/// (c₁ M³ + a23 M^a26 + a24 M^(a26 + 1.5)) ÷ (a25 + M⁵).
#[must_use]
fn r_tms_high(m: f64, c: &ZCoeffs) -> f64 {
    let m3 = m * m * m;
    let m_a26 = math::powf(m, c.a(26));
    (C1 * m3 + c.a(23) * m_a26 + c.a(24) * m_a26 * m * m.sqrt()) / (c.a(25) + m3 * m * m)
}

/// The radius at the end of the main sequence (HPT equations 9a and 9b).
///
/// Equation 9a up to a17 (1.25–1.6 M☉ depending on Z), equation 9b from M∗ = a17 + 0.1, and a
/// straight line between. Below 0.5 M☉, where equation 9a is extrapolated, the radius is held to at least 1.5
/// times the zero-age radius, as the paper states. The published SSE code applies the hold up to
/// a17, but between 0.5 M☉ and a17 it never binds, which a test checks across the range of Z.
#[must_use]
pub(crate) fn r_tms(m: SolarMasses, c: &ZCoeffs) -> SolarRadii {
    let a17 = c.a(17);
    let m_star = a17 + 0.1;
    let mass = m.value();
    SolarRadii::new(if mass <= a17 {
        let r = r_tms_low(mass, c);
        if mass < 0.5 {
            r.max(1.5 * zams::radius(m, c).value())
        } else {
            r
        }
    } else if mass >= m_star {
        r_tms_high(mass, c)
    } else {
        let low = r_tms_low(a17, c);
        let high = r_tms_high(m_star, c);
        low + (high - low) * (mass - a17) / 0.1
    })
}

/// The luminosity at the base of the giant branch (HPT equation 10):
/// (a27 M^a31 + a28 M^c₂) ÷ (a29 + a30 M^c₃ + M^a32).
#[must_use]
pub(crate) fn l_bgb(m: SolarMasses, c: &ZCoeffs) -> SolarLuminosities {
    let m = m.value();
    SolarLuminosities::new(
        (c.a(27) * math::powf(m, c.a(31)) + c.a(28) * math::powf(m, C2))
            / (c.a(29) + c.a(30) * math::powf(m, C3) + math::powf(m, c.a(32))),
    )
}

/// The luminosity hook's amplitude ΔL at mass `m` in M☉ (HPT equation 16).
#[must_use]
fn delta_l(m: f64, c: &ZCoeffs) -> f64 {
    let m_hook = c.m_hook().value();
    let a33 = c.a(33);
    let high = |m: f64| (c.a(34) / math::powf(m, c.a(35))).min(c.a(36) / math::powf(m, c.a(37)));
    if m <= m_hook {
        0.0
    } else if m < a33 {
        high(a33) * math::powf((m - m_hook) / (a33 - m_hook), 0.4)
    } else {
        high(m)
    }
}

/// The radius hook's amplitude ΔR at mass `m` in M☉ (HPT equation 17).
#[must_use]
fn delta_r(m: f64, c: &ZCoeffs) -> f64 {
    let m_hook = c.m_hook().value();
    let (a42, a43) = (c.a(42), c.a(43));
    let high = |m: f64| {
        (c.a(38) + c.a(39) * m * m * m * m.sqrt()) / (c.a(40) * m * m * m + math::powf(m, c.a(41)))
            - 1.0
    };
    if m <= m_hook {
        0.0
    } else if m <= a42 {
        a43 * ((m - m_hook) / (a42 - m_hook)).sqrt()
    } else if m < 2.0 {
        a43 + (high(2.0) - a43) * math::powf((m - a42) / (2.0 - a42), c.a(44))
    } else {
        high(m)
    }
}

/// The exponent η of the β term of the luminosity at mass `m` in M☉ (HPT equation 18): 10, except
/// at Z ≤ 0.0009, where it rises linearly from 10 at 1.0 M☉ to 20 at 1.1 M☉.
#[must_use]
fn eta(m: f64, c: &ZCoeffs) -> f64 {
    if c.z().value() > 0.0009 || m <= 1.0 {
        10.0
    } else if m >= 1.1 {
        20.0
    } else {
        10.0 + 100.0 * (m - 1.0)
    }
}

/// The luminosity α coefficient at mass `m` in M☉ (HPT equations 19a and 19b).
#[must_use]
fn alpha_l(m: f64, c: &ZCoeffs) -> f64 {
    let high = |m: f64| {
        (c.a(45) + c.a(46) * math::powf(m, c.a(48)))
            / (math::powf(m, 0.4) + c.a(47) * math::powf(m, 1.9))
    };
    let (a49, a50, a51, a52, a53) = (c.a(49), c.a(50), c.a(51), c.a(52), c.a(53));
    if m >= 2.0 {
        high(m)
    } else if m < 0.5 {
        a49
    } else if m < 0.7 {
        a49 + 5.0 * (0.3 - a49) * (m - 0.5)
    } else if m < a52 {
        0.3 + (a50 - 0.3) * (m - 0.7) / (a52 - 0.7)
    } else if m < a53 {
        a50 + (a51 - a50) * (m - a52) / (a53 - a52)
    } else {
        a51 + (high(2.0) - a51) * (m - a53) / (2.0 - a53)
    }
}

/// The luminosity β coefficient at mass `m` in M☉ (HPT equation 20): max(0, a54 − a55 M^a56), and
/// above a57, while positive, falling linearly to zero over 0.1 M☉ from its value at a57.
#[must_use]
fn beta_l(m: f64, c: &ZCoeffs) -> f64 {
    let base = |m: f64| 0.0_f64.max(c.a(54) - c.a(55) * math::powf(m, c.a(56)));
    let beta = base(m);
    let a57 = c.a(57);
    if m > a57 && beta > 0.0 {
        let b = base(a57);
        0.0_f64.max(b - 10.0 * (m - a57) * b)
    } else {
        beta
    }
}

/// The radius α coefficient at mass `m` in M☉ (HPT equations 21a and 21b); equation 21a is
/// [`ZCoeffs::alpha_r_power_law`].
#[must_use]
fn alpha_r(m: f64, c: &ZCoeffs) -> f64 {
    let (a62, a63, a64, a66, a67, a68) = (c.a(62), c.a(63), c.a(64), c.a(66), c.a(67), c.a(68));
    if m < 0.5 {
        a62
    } else if m < 0.65 {
        a62 + (a63 - a62) * (m - 0.5) / 0.15
    } else if m < a68 {
        a63 + (a64 - a63) * (m - 0.65) / (a68 - 0.65)
    } else if m < a66 {
        a64 + (c.alpha_r_power_law(a66) - a64) * (m - a68) / (a66 - a68)
    } else if m <= a67 {
        c.alpha_r_power_law(m)
    } else {
        c.alpha_r_power_law(a67) + c.a(65) * (m - a67)
    }
}

/// The radius β coefficient βR = β′R − 1 at mass `m` in M☉ (HPT equations 22a and 22b).
///
/// Equation 22b's second branch is 1.06 + (a72 − 1.06)(M − 1.0) ÷ (a74 − 1.0): the paper prints
/// the denominator as a74 − 1.06 (in the journal and the preprint), which would not reach a72 at
/// a74; the published SSE code has a74 − 1.0, a misprint settled.
#[must_use]
fn beta_r(m: f64, c: &ZCoeffs) -> f64 {
    let power_law = |m: f64| c.a(69) * m * m * m * m.sqrt() / (c.a(70) + math::powf(m, c.a(71)));
    let (a72, a74) = (c.a(72), c.a(74));
    let beta_prime = if m <= 1.0 {
        1.06
    } else if m < a74 {
        1.06 + (a72 - 1.06) * (m - 1.0) / (a74 - 1.0)
    } else if m < 2.0 {
        a72 + (power_law(2.0) - a72) * (m - a74) / (2.0 - a74)
    } else if m <= 16.0 {
        power_law(m)
    } else {
        power_law(16.0) + c.a(73) * (m - 16.0)
    };
    beta_prime - 1.0
}

/// The radius γ coefficient at mass `m` in M☉ (HPT equation 23), never negative.
///
/// The low-mass branch takes a76 + a77 |M − a78|^a79, as the published SSE code does: the paper
/// has no absolute value, and it changes nothing, since a79 is 2 wherever a78 is positive, but it
/// keeps a negative base away from a fractional power. The last branch, C − 10 (M − a75) C, ends
/// at a75 + 0.1, where it reaches zero and γ is zero beyond (the journal's equation 23; the arXiv
/// preprint misprints the bound as a75 + 1.0).
#[must_use]
fn gamma(m: f64, c: &ZCoeffs) -> f64 {
    let (a75, a80) = (c.a(75), c.a(80));
    if m > a75 + 0.1 {
        return 0.0;
    }
    let low = |m: f64| c.a(76) + c.a(77) * math::powf((m - c.a(78)).abs(), c.a(79));
    let b = 0.0_f64.max(low(1.0));
    let value = if m <= 1.0 {
        low(m)
    } else if m <= a75 {
        b + (a80 - b) * math::powf((m - 1.0) / (a75 - 1.0), c.a(81))
    } else {
        let c_value = if a75 > 1.0 { a80 } else { b };
        c_value - 10.0 * (m - a75) * c_value
    };
    value.max(0.0)
}

/// The hydrogen mass fraction of the models HPT fit, X = 0.76 − 3Z (Pols et al. 1998, MNRAS 298,
/// 525).
#[must_use]
fn hydrogen(c: &ZCoeffs) -> f64 {
    0.76 - 3.0 * c.z().value()
}

/// A star of one mass on the main sequence: every mass-dependent quantity of HPT section 5.1.1,
/// evaluated once, and luminosity and radius at any age along it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MainSequence {
    t_ms: f64,
    t_hook: f64,
    l_zams: f64,
    r_zams: f64,
    log_l_tms: f64,
    log_r_tms: f64,
    alpha_l: f64,
    beta_l: f64,
    eta: f64,
    delta_l: f64,
    alpha_r: f64,
    beta_r: f64,
    gamma: f64,
    delta_r: f64,
    r_degenerate: f64,
}

impl MainSequence {
    /// The main sequence of a star of mass `m` (0.1–100 M☉) at the metallicity of `c`.
    #[must_use]
    pub(crate) fn new(m: SolarMasses, c: &ZCoeffs) -> Self {
        let l_zams = zams::luminosity(m, c).value();
        let r_zams = zams::radius(m, c).value();
        let mass = m.value();
        let (t_hook, t_ms) = timescales_myr(mass, c);
        // HPT equation 24: low-mass stars are partly degenerate (Tout et al. 1997), so the radius
        // is held above 0.0258 (1 + X)^(5/3) M^(−1/3). The floor binds only near 0.1 M☉ and is
        // applied at every mass, which changes nothing above.
        let r_degenerate = 0.0258 * math::powf(1.0 + hydrogen(c), 5.0 / 3.0) / math::cbrt(mass);
        Self {
            t_ms,
            t_hook,
            l_zams,
            r_zams,
            log_l_tms: math::log10(l_tms(m, c).value() / l_zams),
            log_r_tms: math::log10(r_tms(m, c).value() / r_zams),
            alpha_l: alpha_l(mass, c),
            beta_l: beta_l(mass, c),
            eta: eta(mass, c),
            delta_l: delta_l(mass, c),
            alpha_r: alpha_r(mass, c),
            beta_r: beta_r(mass, c),
            gamma: gamma(mass, c),
            delta_r: delta_r(mass, c),
            r_degenerate,
        }
    }

    /// The main-sequence lifetime, Myr.
    #[must_use]
    pub(crate) const fn t_ms(&self) -> Megayears {
        Megayears::new(self.t_ms)
    }

    /// Luminosity and radius at `t` from the zero-age main sequence, 0 ≤ t ≤ `t_ms` (HPT
    /// equations 11–15); the core mass is zero, as HPT define none on the main sequence.
    ///
    /// # Panics
    ///
    /// In debug builds, if `t` lies outside 0 ≤ t ≤ `t_ms` by more than rounding: past `t_ms` the
    /// polynomials in τ extrapolate.
    #[must_use]
    pub(crate) fn at(&self, t: Megayears) -> PhasePoint {
        let t = t.value();
        let tau = t / self.t_ms;
        debug_assert!(
            (-1e-12..=1.0 + 1e-9).contains(&tau),
            "the main sequence runs for τ from 0 to 1, not {tau}"
        );
        let tau1 = (t / self.t_hook).min(1.0);
        let tau2 = 0.0_f64.max(
            1.0_f64.min((t - (1.0 - HOOK_EPSILON) * self.t_hook) / (HOOK_EPSILON * self.t_hook)),
        );
        let tau_squared = tau * tau;
        let tau_cubed = tau_squared * tau;
        let log_l = self.alpha_l * tau
            + self.beta_l * math::powf(tau, self.eta)
            + (self.log_l_tms - self.alpha_l - self.beta_l) * tau_squared
            - self.delta_l * (tau1 * tau1 - tau2 * tau2);
        let tau_10 = math::powi(tau, 10);
        let tau_40 = math::powi(tau_10, 4);
        let log_r = self.alpha_r * tau
            + self.beta_r * tau_10
            + self.gamma * tau_40
            + (self.log_r_tms - self.alpha_r - self.beta_r - self.gamma) * tau_cubed
            - self.delta_r * (tau1 * tau1 * tau1 - tau2 * tau2 * tau2);
        PhasePoint {
            luminosity: SolarLuminosities::new(self.l_zams * math::exp10(log_l)),
            radius: SolarRadii::new((self.r_zams * math::exp10(log_r)).max(self.r_degenerate)),
            core_mass: SolarMasses::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::continuity::{assert_continuous_over, assert_no_jump, suspect_intervals};
    use super::*;
    use crate::units::MetalFraction;

    const REFERENCE_Z: [f64; 5] = [1e-4, 1e-3, 4e-3, 0.02, 0.03];

    /// A coefficient of the evolution at a mass in M☉.
    type Coefficient = fn(f64, &ZCoeffs) -> f64;

    /// An end point of the main sequence at a mass, in its unit.
    type EndPoint = fn(SolarMasses, &ZCoeffs) -> f64;

    /// Z at 200 points evenly in log Z from 0.0001 to 0.03.
    fn z_sweep() -> impl Iterator<Item = f64> {
        let (lo, hi) = (math::log10(1e-4), math::log10(0.03));
        (0..200).map(move |i| math::exp10(lo + (hi - lo) * f64::from(i) / 199.0).clamp(1e-4, 0.03))
    }

    fn coeffs(z: f64) -> ZCoeffs {
        ZCoeffs::new(MetalFraction::new(z))
    }

    fn mass(m: f64) -> SolarMasses {
        SolarMasses::new(m)
    }

    /// log₁₀ M from 0.1 to 100 M☉, evenly spaced.
    fn log_mass_sweep(n: u32) -> Vec<f64> {
        (0..n)
            .map(|i| -1.0 + 3.0 * f64::from(i) / f64::from(n - 1))
            .collect()
    }

    /// log₁₀ L and log₁₀ R of a main sequence at fraction τ of it.
    fn log_lr(ms: &MainSequence, tau: f64) -> [f64; 2] {
        let point = ms.at(Megayears::new(tau * ms.t_ms));
        [
            math::log10(point.luminosity.value()),
            math::log10(point.radius.value()),
        ]
    }

    #[test]
    fn the_sun_leaves_the_main_sequence_after_about_eleven_gigayears() {
        let t = t_ms(mass(1.0), &coeffs(0.02)).value();
        assert!((t / 11_000.0 - 1.0).abs() < 0.03, "t_ms = {t} Myr");
    }

    /// The brainstorm's B/C boundary: a 0.75 M☉ star is still on the main sequence after 13.8 Gyr
    /// at every metallicity, swept at 200 points over the fitted range.
    #[test]
    fn a_three_quarter_solar_mass_star_outlives_the_universe_at_every_metallicity() {
        for z in z_sweep() {
            let t = t_ms(mass(0.75), &coeffs(z)).value();
            assert!(t > 13_800.0, "t_ms(0.75) = {t} Myr at Z = {z}");
        }
    }

    #[test]
    fn timescales_fall_with_mass_and_keep_their_order() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            let mut last = f64::INFINITY;
            for log_m in log_mass_sweep(200) {
                let m = mass(math::exp10(log_m));
                let bgb = t_bgb(m, &c).value();
                let hook = t_hook(m, &c).value();
                let ms = t_ms(m, &c).value();
                assert!(ms < last, "t_ms rises at M = {m:?}, Z = {z}");
                assert!(hook <= ms && ms < bgb, "order at M = {m:?}, Z = {z}");
                last = ms;
            }
        }
    }

    #[test]
    fn terminal_luminosity_exceeds_zero_age_and_end_points_are_continuous_in_mass() {
        let ends: [(&str, EndPoint); 3] = [
            ("L_TMS", |m, c| l_tms(m, c).value()),
            ("R_TMS", |m, c| r_tms(m, c).value()),
            ("L_BGB", |m, c| l_bgb(m, c).value()),
        ];
        for z in REFERENCE_Z {
            let c = coeffs(z);
            let log_masses = log_mass_sweep(200);
            for &log_m in &log_masses {
                let m = mass(math::exp10(log_m));
                assert!(
                    l_tms(m, &c).value() > zams::luminosity(m, &c).value(),
                    "L_TMS ≤ L_ZAMS at M = {m:?}, Z = {z}"
                );
            }
            for (name, f) in ends {
                let what = format!("{name} at Z = {z}");
                let log_f = |log_m: f64| math::log10(f(mass(math::exp10(log_m)), &c));
                assert_continuous_over(&what, log_f, &log_masses, 0.05, 1e-6);
            }
        }
    }

    /// Each coefficient of equations 12–23 swept over mass on its own, so that a slip inside one
    /// of their piecewise branches shows at its own scale: every interval that changes by more
    /// than a hundredth of the coefficient's range over the sweep is bisected down to the
    /// resolution of `f64`, across `M_hook`, a33, a42, a52, a53, a57, a66–a68, a74, a75 and the
    /// fixed boundaries at 0.5, 0.65, 0.7, 1.0, 1.1, 2.0 and 16.0 M☉.
    #[test]
    fn every_coefficient_of_the_evolution_is_continuous_in_mass() {
        let coefficients: [(&str, Coefficient); 8] = [
            ("αL", alpha_l),
            ("βL", beta_l),
            ("η", eta),
            ("ΔL", delta_l),
            ("αR", alpha_r),
            ("βR", beta_r),
            ("γ", gamma),
            ("ΔR", delta_r),
        ];
        let masses: Vec<f64> = log_mass_sweep(2_000).into_iter().map(math::exp10).collect();
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for (name, f) in coefficients {
                let (lo, hi) = masses
                    .iter()
                    .map(|&m| f(m, &c))
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                        (lo.min(v), hi.max(v))
                    });
                let what = format!("{name} at Z = {z}");
                let coarse = 0.01 * (hi - lo).max(1e-3);
                assert_continuous_over(&what, |m| f(m, &c), &masses, coarse, 1e-9);
            }
        }
    }

    /// The published SSE code holds `R_TMS` to 1.5 `R_ZAMS` up to a17, the paper only below
    /// 0.5 M☉; between them the hold never binds, so the two agree.
    #[test]
    fn the_terminal_radius_hold_never_binds_between_half_a_solar_mass_and_a17() {
        for z in [1e-4, 3e-4, 1e-3, 4e-3, 0.01, 0.02, 0.03] {
            let c = coeffs(z);
            for i in 0..=200 {
                let m = 0.5 + (c.a(17) - 0.5) * f64::from(i) / 200.0;
                let hold = 1.5 * zams::radius(mass(m), &c).value();
                assert!(
                    r_tms_low(m, &c) > hold,
                    "the hold binds at M = {m}, Z = {z}"
                );
            }
        }
    }

    #[test]
    fn the_track_starts_at_the_zero_age_and_ends_at_the_terminal_point() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for log_m in log_mass_sweep(60) {
                let m = mass(math::exp10(log_m));
                let ms = MainSequence::new(m, &c);
                let start = ms.at(Megayears::ZERO);
                let end = ms.at(ms.t_ms());
                let floor = |r: f64| r.max(ms.r_degenerate);
                let l0 = start.luminosity.value() / ms.l_zams;
                let r0 = start.radius.value() / floor(ms.r_zams);
                let l1 = end.luminosity / l_tms(m, &c);
                let r1 = end.radius.value() / floor(r_tms(m, &c).value());
                for (name, ratio) in [("L(0)", l0), ("R(0)", r0), ("L(1)", l1), ("R(1)", r1)] {
                    assert!(
                        (ratio - 1.0).abs() < 1e-9,
                        "{name} at {m:?}, Z = {z}: {ratio}"
                    );
                }
            }
        }
    }

    /// log L and log R swept over a fine mass grid at fixed fractions τ of the main sequence, and
    /// over τ at each mass; every value is finite, and every interval that changes by more than a
    /// coarse bound, or departs from its neighbours (see [`suspect_intervals`]), is bisected down
    /// to the resolution of `f64`, so that only a jump fails, down to about 10⁻⁴ dex.
    #[test]
    fn luminosity_and_radius_are_continuous_in_age_and_mass() {
        let taus: Vec<f64> = (0..=40).map(|i| f64::from(i) / 40.0).collect();
        for z in REFERENCE_Z {
            let c = coeffs(z);
            let log_masses = log_mass_sweep(600);
            let table: Vec<(MainSequence, Vec<[f64; 2]>)> = log_masses
                .iter()
                .map(|&log_m| {
                    let ms = MainSequence::new(mass(math::exp10(log_m)), &c);
                    let row: Vec<[f64; 2]> = taus.iter().map(|&t| log_lr(&ms, t)).collect();
                    assert!(
                        row.iter().flatten().all(|v| v.is_finite()),
                        "log M = {log_m}"
                    );
                    (ms, row)
                })
                .collect();
            for k in 0..2 {
                for (j, &tau) in taus.iter().enumerate() {
                    let column: Vec<f64> = table.iter().map(|(_, row)| row[j][k]).collect();
                    for i in suspect_intervals(&column, 0.05) {
                        let f = |log_m: f64| {
                            log_lr(&MainSequence::new(mass(math::exp10(log_m)), &c), tau)[k]
                        };
                        let what = format!("quantity {k} in mass at τ = {tau}, Z = {z}");
                        assert_no_jump(&what, f, log_masses[i], log_masses[i + 1], 1e-6);
                    }
                }
                for ((ms, row), log_m) in table.iter().zip(&log_masses) {
                    let values: Vec<f64> = row.iter().map(|v| v[k]).collect();
                    for j in suspect_intervals(&values, 0.05) {
                        let what = format!("quantity {k} in τ at log M = {log_m}, Z = {z}");
                        let f = |t: f64| log_lr(ms, t)[k];
                        assert_no_jump(&what, f, taus[j], taus[j + 1], 1e-6);
                    }
                }
            }
        }
    }

    #[test]
    fn the_sun_at_4_57_gyr_has_one_solar_luminosity_and_one_solar_radius() {
        let point = MainSequence::new(mass(1.0), &coeffs(0.02)).at(Megayears::new(4_570.0));
        let (l, r) = (point.luminosity.value(), point.radius.value());
        assert!((l - 1.0).abs() < 0.05, "L = {l}");
        assert!((r - 1.0).abs() < 0.03, "R = {r}");
    }

    /// Luminosity and radius at fixed mass from the published SSE code (see
    /// [`sse`](super::super) for the run; `hrdiag` on the main sequence, with no mass loss), to
    /// 10⁻⁹.
    #[test]
    fn matches_the_published_sse_code() {
        for &(z, m, t, l_sse, r_sse) in SSE_MS {
            let point = MainSequence::new(mass(m), &coeffs(z)).at(Megayears::new(t));
            let (l, r) = (point.luminosity.value(), point.radius.value());
            assert!(
                (l / l_sse - 1.0).abs() < 1e-9,
                "L at Z = {z}, M = {m}, t = {t}: {l} against {l_sse}"
            );
            assert!(
                (r / r_sse - 1.0).abs() < 1e-9,
                "R at Z = {z}, M = {m}, t = {t}: {r} against {r_sse}"
            );
        }
    }

    /// (Z, M, t Myr, L, R) from SSE's `hrdiag`, at τ from 0.3 to 0.995, across the hook, and at
    /// Z = 0.0001 from 1.1 M☉, where the β term's exponent η is 20 (equation 18).
    const SSE_MS: &[(f64, f64, f64, f64, f64)] = &[
        (
            0.02,
            1.0,
            4_940.649_810_985_633,
            0.988_071_044_469_351_6,
            0.996_321_214_790_472_2,
        ),
        (
            0.02,
            5.0,
            101.026_039_958_736_77,
            1_153.731_125_165_928,
            6.891_787_242_716_354_5,
        ),
        (
            0.0001,
            0.8,
            8_125.207_968_326_566,
            1.055_569_824_769_747_6,
            0.835_844_887_282_063_7,
        ),
        (
            0.004,
            1.25,
            3_475.638_865_290_959,
            9.286_627_589_801_604,
            2.430_038_488_710_929,
        ),
        (
            0.03,
            40.0,
            1.345_378_623_572_403_5,
            284_167.046_720_452,
            12.144_594_884_631_417,
        ),
        (
            0.001,
            2.0,
            778.912_422_906_840_9,
            61.400_298_596_525_17,
            2.586_839_384_135_179,
        ),
        (
            0.02,
            0.3,
            202_068.431_778_815_8,
            0.015_650_917_374_506_305,
            0.337_220_917_011_308_7,
        ),
        (
            0.0001,
            60.0,
            3.335_852_708_331_565_4,
            906_375.039_893_422_3,
            13.637_737_414_000_46,
        ),
        (
            0.02,
            1.5,
            2_711.630_026_278_984,
            7.580_769_093_396_833,
            2.511_663_065_916_609_5,
        ),
        (
            0.0001,
            1.1,
            2_555.291_577_356_671,
            4.206_524_436_394_703,
            1.009_485_395_494_931_2,
        ),
        (
            0.0001,
            1.1,
            4_134.268_919_968_576,
            10.626_679_284_748_201,
            1.548_704_914_314_370_5,
        ),
        (
            0.0001,
            1.25,
            1_695.853_967_183_741_6,
            7.078_190_695_245_577,
            1.065_083_796_165_719_1,
        ),
        (
            0.0001,
            1.25,
            2_732.655_681_717_877,
            16.900_715_983_651_516,
            1.713_113_275_733_314_7,
        ),
    ];
}
