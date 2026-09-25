//! The first giant branch: the core mass–luminosity relation, the growth of the core in time, the
//! giant's radius, and the star at helium ignition (plan 06, P06.T6.a and T6.c; Hurley, Pols and
//! Tout 2000, MNRAS 315, 543, "HPT", sections 5.2 and 5.3).
//!
//! The `pub(crate)` items take and return unit newtypes in HPT's units (M☉, L☉, R☉, Myr from the
//! zero-age main sequence); the private helpers take the mass `m` as a bare `f64` in M☉. The end
//! of the Hertzsprung gap and equation 44 need parts of HPT sections 5.3 and 5.4, so they are here
//! too, for P06.T7 and T8 to use rather than rebuild: the core mass at the base of the asymptotic
//! giant branch (equation 66, the `m_c_bagb` of T8, T18 and T28), the blue loop's minimum radius
//! from `M_HeF` up (equation 55), the blue-phase fraction above `M_FGB` (equation 58) and the
//! asymptotic-giant radius (equation 74).

use crate::math;
use crate::units::{Megayears, SolarLuminosities, SolarMasses, SolarRadii};

use super::PhasePoint;
use super::calibration::{Calibration, RadiusCoeffs, RadiusInZ, ZBlend, power_from_ln};
use super::coeffs::{Side, ZCoeffs, lesser_side};
use super::ms;

/// c₁ of HPT equation 44.
const MC_BGB_C1: f64 = 9.209_25e-5;

/// c₂ of HPT equation 44.
const MC_BGB_C2: f64 = 5.402_216;

/// The mass range, M☉, over which p, q and D pass from their low-mass to their high-mass forms
/// (HPT section 5.2; see [`GiantBranch::new`] for the lower end).
const TRANSITION_MSUN: (f64, f64) = (2.0, 2.5);

/// The giant-branch relation of one star, L = min(B Mc^q, D Mc^p) (HPT equation 37), and the
/// hydrogen rate constant A′H that sets how fast the core grows along it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GiantBranch {
    p: f64,
    q: f64,
    b: f64,
    d: f64,
    a_h: f64,
    m_x: f64,
    l_x: f64,
}

impl GiantBranch {
    /// The relation for initial mass `m`.
    ///
    /// Low mass: p = 6, q = 3 and log D = D₀ = 5.37 + 0.135ζ; from 2.5 M☉: p = 5, q = 2 and
    /// log D = max(−1.0, 0.975 D₀ − 0.18 M, 0.5 D₀ − 0.06 M); each linear in M between (HPT
    /// section 5.2), log D towards 0.975 D₀ − 0.45. B = max(3 × 10⁴, 500 + 1.75 × 10⁴ M^0.6), and
    /// log A′H = max(−4.8, min(−5.7 + 0.8 M, −4.1 + 0.14 M)) in M☉ L☉⁻¹ Myr⁻¹ (HPT section 5.2,
    /// unnumbered, with its Table 1).
    ///
    /// The paper interpolates from `M_HeF`; the published SSE code from 2.0 M☉, which is
    /// `M_HeF` at Z = 0.02 (1.995) but not elsewhere (1.82–2.04). From `M_HeF` the giant
    /// branch of a 2 M☉ star at Z = 0.001 is up to 0.06 dex fainter at a given age than the code's
    /// and ignites helium 0.4 Myr later, beyond the 0.02 dex P06.T12.b validates to, so the code's
    /// 2.0 M☉ is used, pending the owner's confirmation.
    #[must_use]
    pub(crate) fn new(m: SolarMasses, c: &ZCoeffs) -> Self {
        let m = m.value();
        let d0 = 5.37 + 0.135 * c.zeta();
        let (low, top) = TRANSITION_MSUN;
        let (p, q, log_d) = if m <= low {
            (6.0, 3.0, d0)
        } else if m >= top {
            (
                5.0,
                2.0,
                (-1.0_f64).max((0.975 * d0 - 0.18 * m).max(0.5 * d0 - 0.06 * m)),
            )
        } else {
            let fraction = (m - low) / (top - low);
            (
                6.0 - fraction,
                3.0 - fraction,
                d0 - (d0 - (0.975 * d0 - 0.18 * top)) * fraction,
            )
        };
        let b_coefficient = 3e4_f64.max(500.0 + 1.75e4 * math::powf_positive(m, 0.6));
        let d_coefficient = math::exp10(log_d);
        let log_a_h = (-4.8_f64).max((-5.7 + 0.8 * m).min(-4.1 + 0.14 * m));
        // HPT equation 38: the two power laws cross at M_x = (B ÷ D)^(1 ÷ (p − q)).
        let m_x = math::powf_positive(b_coefficient / d_coefficient, 1.0 / (p - q));
        Self {
            p,
            q,
            b: b_coefficient,
            d: d_coefficient,
            a_h: math::exp10(log_a_h),
            m_x,
            l_x: d_coefficient * math::powf_positive(m_x, p),
        }
    }

    /// The core mass where the two power laws cross, `M_x` (HPT equation 38).
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn m_x(&self) -> SolarMasses {
        SolarMasses::new(self.m_x)
    }

    /// The luminosity where the two power laws cross, `L_x`.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn l_x(&self) -> SolarLuminosities {
        SolarLuminosities::new(self.l_x)
    }

    /// Luminosity from core mass (HPT equation 37): min(B Mc^q, D Mc^p).
    #[must_use]
    pub(crate) fn luminosity(&self, mc: SolarMasses) -> SolarLuminosities {
        let mc = mc.value();
        let high = || self.b * math::powf_positive(mc, self.q);
        let low = || self.d * math::powf_positive(mc, self.p);
        // Below M_x the low-luminosity law D Mc^p is the lesser, since p > q; only where that is
        // uncertain are both evaluated, so the result is `min`'s bit for bit (see
        // `coeffs::lesser_side`).
        SolarLuminosities::new(match lesser_side(mc, self.m_x, false) {
            Some(Side::First) => high(),
            Some(Side::Second) => low(),
            None => high().min(low()),
        })
    }

    /// Core mass from luminosity: the inverse of [`GiantBranch::luminosity`].
    #[must_use]
    pub(crate) fn core_mass(&self, l: SolarLuminosities) -> SolarMasses {
        let l = l.value();
        SolarMasses::new(if l <= self.l_x {
            math::powf_positive(l / self.d, 1.0 / self.p)
        } else {
            math::powf_positive(l / self.b, 1.0 / self.q)
        })
    }

    /// The timescales of the core's growth from `t_bgb`, where the luminosity is `l_bgb`: `t_inf,1`,
    /// `t_x` and `t_inf,2` (HPT equations 40–42).
    #[must_use]
    pub(crate) fn times(&self, t_bgb: Megayears, l_bgb: SolarLuminosities) -> GiantTimes {
        let (p, q) = (self.p, self.q);
        let (t_bgb, l_bgb) = (t_bgb.value(), l_bgb.value());
        let t_inf1 = t_bgb
            + math::powf_positive(self.d / l_bgb, (p - 1.0) / p) / ((p - 1.0) * self.a_h * self.d);
        let t_x = t_inf1 - (t_inf1 - t_bgb) * math::powf_positive(l_bgb / self.l_x, (p - 1.0) / p);
        let t_inf2 = t_x
            + math::powf_positive(self.b / self.l_x, (q - 1.0) / q)
                / ((q - 1.0) * self.a_h * self.b);
        GiantTimes {
            t_inf1: Megayears::new(t_inf1),
            t_x: Megayears::new(t_x),
            t_inf2: Megayears::new(t_inf2),
        }
    }

    /// The core mass at time `t`, from `t_BGB` on (HPT equation 39).
    #[must_use]
    pub(crate) fn core_mass_at(&self, times: &GiantTimes, t: Megayears) -> SolarMasses {
        let t = t.value();
        SolarMasses::new(if t <= times.t_x.value() {
            math::powf_positive(
                (self.p - 1.0) * self.a_h * self.d * (times.t_inf1.value() - t),
                1.0 / (1.0 - self.p),
            )
        } else {
            math::powf_positive(
                (self.q - 1.0) * self.a_h * self.b * (times.t_inf2.value() - t),
                1.0 / (1.0 - self.q),
            )
        })
    }

    /// The time at which the luminosity reaches `l` (HPT equation 43, at any L).
    #[must_use]
    pub(crate) fn time_of_luminosity(&self, times: &GiantTimes, l: SolarLuminosities) -> Megayears {
        let (p, q) = (self.p, self.q);
        let l = l.value();
        Megayears::new(if l <= self.l_x {
            times.t_inf1.value()
                - math::powf_positive(self.d / l, (p - 1.0) / p) / ((p - 1.0) * self.a_h * self.d)
        } else {
            times.t_inf2.value()
                - math::powf_positive(self.b / l, (q - 1.0) / q) / ((q - 1.0) * self.a_h * self.b)
        })
    }

    /// The same relation with the rate constant `rate_msun_per_lsun_myr` in place of A′H: `A_He`
    /// on the early asymptotic giant branch and for helium giants, `A_H,He` on the thermally pulsing
    /// one (HPT sections 5.4 and 6.1, P06.T8 and T9).
    #[must_use]
    pub(crate) const fn with_rate(self, rate_msun_per_lsun_myr: f64) -> Self {
        Self {
            a_h: rate_msun_per_lsun_myr,
            ..self
        }
    }

    /// The helium giants' relation for a helium star of mass `m` (HPT equation 84, P06.T9): p = 5,
    /// q = 3, B = 4.1 × 10⁴ and D = 5.5 × 10⁴ ÷ (1 + 0.4 M⁴), with the rate constant
    /// `rate_msun_per_lsun_myr`.
    #[must_use]
    pub(crate) fn helium_giant(m: SolarMasses, rate_msun_per_lsun_myr: f64) -> Self {
        let (p, q) = (5.0, 3.0);
        let b_coefficient = 4.1e4;
        let d_coefficient = 5.5e4 / (1.0 + 0.4 * math::powi(m.value(), 4));
        let m_x = math::powf_positive(b_coefficient / d_coefficient, 1.0 / (p - q));
        Self {
            p,
            q,
            b: b_coefficient,
            d: d_coefficient,
            a_h: rate_msun_per_lsun_myr,
            m_x,
            l_x: d_coefficient * math::powf_positive(m_x, p),
        }
    }

    /// The timescales of growth from `t0`, where the luminosity is `l0`, at any luminosity:
    /// [`GiantBranch::times`] up to `L_x`, and above it `t_inf,2` = `t0` + (B ÷ `l0`)^((q − 1) ÷ q)
    /// ÷ ((q − 1) A B) (HPT equation 72, for the thermally pulsing AGB of a star whose core starts
    /// above `M_x`), with `t_x` at `t0` and `t_inf,1` set so that the low-luminosity law meets the
    /// core there.
    #[must_use]
    pub(crate) fn times_from(&self, t0: Megayears, l0: SolarLuminosities) -> GiantTimes {
        if l0.value() <= self.l_x {
            return self.times(t0, l0);
        }
        let (p, q) = (self.p, self.q);
        let mc0 = self.core_mass(l0).value();
        GiantTimes {
            t_inf1: t0
                + Megayears::new(
                    math::powf_positive(mc0, 1.0 - p) / ((p - 1.0) * self.a_h * self.d),
                ),
            t_x: t0,
            t_inf2: t0
                + Megayears::new(
                    math::powf_positive(self.b / l0.value(), (q - 1.0) / q)
                        / ((q - 1.0) * self.a_h * self.b),
                ),
        }
    }
}

/// The timescales of one giant branch (HPT equations 40–42), built by [`GiantBranch::times`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[expect(
    clippy::struct_field_names,
    reason = "HPT's names t_inf,1, t_x and t_inf,2"
)]
pub(crate) struct GiantTimes {
    /// `t_inf,1`: where the low-luminosity power law's core mass would diverge.
    t_inf1: Megayears,
    /// `t_x`: where the core reaches `M_x`.
    t_x: Megayears,
    /// `t_inf,2`: where the high-luminosity power law's core mass would diverge.
    t_inf2: Megayears,
}

/// The giant's radius (HPT equation 46): A (L^b1 + b2 L^b3) with A = min(b4 M^−b5, b6 M^−b7).
#[must_use]
pub(crate) fn radius(m: SolarMasses, l: SolarLuminosities, c: &ZCoeffs) -> SolarRadii {
    RadiusLaw::giant(m, c).at(l)
}

/// The radius on the asymptotic giant branch (HPT equation 74): A (L^b1 + b2 L^b50).
///
/// From `M_HeF` up, b50 = b55 b3 and A = min(b51 M^−b52, b53 M^−b54); up to `M_HeF` − 0.2,
/// b50 = b3 and A = b56 + b57 M; both linear in M between.
#[must_use]
pub(crate) fn agb_radius(m: SolarMasses, l: SolarLuminosities, c: &ZCoeffs) -> SolarRadii {
    RadiusLaw::asymptotic(m, c).at(l)
}

/// A giant's radius at one mass as a function of luminosity: [`radius`] and [`agb_radius`] with
/// their mass dependence evaluated once, for the phases of P06.T7 and T8 that evaluate them along a
/// track.
///
/// At one of HPT's calibration metallicities it is the law A (L^b1 + b2 L^x) with the Appendix's
/// coefficients at Z. Between two, it is the law at each calibration metallicity of the stencil,
/// and ln R is the monotone cubic in log Z through their radii (galaxy-generation ruling 92; see
/// [`calibration`](super::calibration)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RadiusLaw(LawInZ);

/// A [`RadiusLaw`] at a calibration metallicity or between two.
#[derive(Debug, Clone, Copy, PartialEq)]
enum LawInZ {
    /// The law at Z, as printed.
    Printed(PrintedLaw),
    /// The laws at the nodes of the stencil (the first [`ZBlend::count`] places), and the cubic's
    /// weights at Z.
    Between { nodes: [LogLaw; 4], blend: ZBlend },
}

/// The radius law A (L^b1 + b2 L^x) at one metallicity and mass.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PrintedLaw {
    a: f64,
    b1: f64,
    b2: f64,
    x: f64,
}

impl PrintedLaw {
    /// Equation 46 at mass `m` (M☉): x = b3, A = min(b4 M^−b5, b6 M^−b7).
    #[must_use]
    fn giant(m: f64, k: &RadiusCoeffs) -> Self {
        Self {
            a: k.giant_scale.at(m),
            b1: k.b1,
            b2: k.b2,
            x: k.b3,
        }
    }

    /// Equation 74 at mass `m` (M☉; see [`agb_radius`]).
    #[must_use]
    fn asymptotic(m: f64, k: &RadiusCoeffs) -> Self {
        let m_hef = k.m_hef;
        let m1 = m_hef - 0.2;
        let high_a = |m: f64| k.agb_scale.at(m);
        let low_a = |m: f64| k.b56 + k.b57 * m;
        let (x, a) = if m >= m_hef {
            (k.b55 * k.b3, high_a(m))
        } else if m <= m1 {
            (k.b3, low_a(m))
        } else {
            let f = (m - m1) / 0.2;
            let low = low_a(m1);
            (
                k.b3 * (1.0 + (k.b55 - 1.0) * f),
                low + (high_a(m_hef) - low) * f,
            )
        };
        Self {
            a,
            b1: k.b1,
            b2: k.b2,
            x,
        }
    }
}

/// The radius law of one calibration metallicity at one mass with its scale as ln A, which is
/// what the cubic in ln R reads: ln R = ln A + ln(L^b1 + b2 L^x).
#[derive(Debug, Clone, Copy, PartialEq)]
struct LogLaw {
    ln_a: f64,
    b1: f64,
    b2: f64,
    x: f64,
}

impl LogLaw {
    /// The unused places of [`LawInZ::Between`].
    const UNUSED: Self = Self {
        ln_a: 0.0,
        b1: 0.0,
        b2: 0.0,
        x: 0.0,
    };

    /// Equation 46 at a mass whose logarithm is `ln_m`, at calibration metallicity `node`.
    #[must_use]
    fn giant(ln_m: f64, node: &Calibration) -> Self {
        let k = &node.coeffs;
        Self {
            ln_a: node.ln_giant_scale(ln_m),
            b1: k.b1,
            b2: k.b2,
            x: k.b3,
        }
    }

    /// Equation 74 at mass `m` (M☉), whose logarithm is `ln_m`, at calibration metallicity
    /// `node`: [`PrintedLaw::asymptotic`] with A in logarithms.
    #[must_use]
    fn asymptotic(m: f64, ln_m: f64, node: &Calibration) -> Self {
        let k = &node.coeffs;
        let m_hef = k.m_hef;
        let m1 = m_hef - 0.2;
        let low_a = |m: f64| k.b56 + k.b57 * m;
        let (x, ln_a) = if m >= m_hef {
            (k.b55 * k.b3, node.ln_agb_scale(ln_m))
        } else if m <= m1 {
            (k.b3, math::ln(low_a(m)))
        } else {
            let f = (m - m1) / 0.2;
            let low = low_a(m1);
            (
                k.b3 * (1.0 + (k.b55 - 1.0) * f),
                math::ln(low + (k.agb_scale.at(m_hef) - low) * f),
            )
        };
        Self {
            ln_a,
            b1: k.b1,
            b2: k.b2,
            x,
        }
    }
}

impl RadiusLaw {
    /// The first giant branch's law (HPT equation 46): x = b3, A = min(b4 M^−b5, b6 M^−b7).
    #[must_use]
    pub(crate) fn giant(m: SolarMasses, c: &ZCoeffs) -> Self {
        let m = m.value();
        match c.radius_in_z() {
            RadiusInZ::Calibrated => {
                Self(LawInZ::Printed(PrintedLaw::giant(m, &c.radius_coeffs())))
            }
            RadiusInZ::Between(blend) => {
                let ln_m = math::ln(m);
                Self::between(blend, |node| LogLaw::giant(ln_m, node))
            }
        }
    }

    /// The asymptotic giant branch's law (HPT equation 74; see [`agb_radius`]).
    #[must_use]
    pub(crate) fn asymptotic(m: SolarMasses, c: &ZCoeffs) -> Self {
        let m = m.value();
        match c.radius_in_z() {
            RadiusInZ::Calibrated => Self(LawInZ::Printed(PrintedLaw::asymptotic(
                m,
                &c.radius_coeffs(),
            ))),
            RadiusInZ::Between(blend) => {
                let ln_m = math::ln(m);
                Self::between(blend, |node| LogLaw::asymptotic(m, ln_m, node))
            }
        }
    }

    /// `law` at each node of `blend`'s stencil.
    #[must_use]
    fn between(blend: &ZBlend, law: impl Fn(&Calibration) -> LogLaw) -> Self {
        let mut nodes = [LogLaw::UNUSED; 4];
        for (node, calibration) in nodes.iter_mut().zip(blend.nodes()) {
            *node = law(calibration);
        }
        Self(LawInZ::Between {
            nodes,
            blend: *blend,
        })
    }

    /// The radius at luminosity `l`.
    ///
    /// Each power L^e is `exp(e ln L)` from one logarithm of L, which is
    /// [`math::powf_positive`] bit for bit.
    #[must_use]
    pub(crate) fn at(&self, l: SolarLuminosities) -> SolarRadii {
        let l = l.value();
        debug_assert!(
            l > 0.0 && l.is_finite(),
            "a giant's luminosity is positive: {l}"
        );
        let ln_l = math::ln(l);
        self.radius_of_powers(|_, e| power_from_ln(ln_l, e), |_, e| power_from_ln(ln_l, e))
    }

    /// [`RadiusLaw::at`] at the luminosity of `powers`, bit for bit, reading the powers of it
    /// that this law shares with the one `powers` was built from.
    #[must_use]
    pub(crate) fn at_powers(&self, powers: &LuminosityPowers) -> SolarRadii {
        let ln_l = powers.ln_l;
        let power = |exponent: f64, [cached, value]: [f64; 2]| {
            // The same exponent, to the bit, gives the same power.
            if exponent.total_cmp(&cached).is_eq() {
                value
            } else {
                power_from_ln(ln_l, exponent)
            }
        };
        self.radius_of_powers(|j, e| power(e, powers.b1[j]), |j, e| power(e, powers.x[j]))
    }

    /// The radius from L^b1 and L^x at each node, given as functions of the node's index and the
    /// exponent: the printed law, or e^(the cubic through each node's ln A + ln(L^b1 + b2 L^x)).
    ///
    /// The first three calibration metallicities share b1 = 0.54 (the Appendix's cap), and a node
    /// whose b1 is its predecessor's, to the bit, reuses its power.
    #[must_use]
    fn radius_of_powers(
        &self,
        l_b1: impl Fn(usize, f64) -> f64,
        l_x: impl Fn(usize, f64) -> f64,
    ) -> SolarRadii {
        match &self.0 {
            LawInZ::Printed(law) => {
                SolarRadii::new(law.a * (l_b1(0, law.b1) + law.b2 * l_x(0, law.x)))
            }
            LawInZ::Between { nodes, blend } => {
                let mut ln_r = [0.0; 4];
                let mut previous: Option<[f64; 2]> = None;
                for (j, (y, node)) in ln_r.iter_mut().zip(nodes).take(blend.count()).enumerate() {
                    let p_b1 = match previous {
                        Some([b1, p]) if b1.total_cmp(&node.b1).is_eq() => p,
                        _ => l_b1(j, node.b1),
                    };
                    previous = Some([node.b1, p_b1]);
                    *y = node.ln_a + math::ln(p_b1 + node.b2 * l_x(j, node.x));
                }
                SolarRadii::new(math::exp(blend.at(&ln_r)))
            }
        }
    }

    /// Each node's b1 and x, the first [`ZBlend::count`] (or one, at a calibration metallicity).
    #[must_use]
    fn exponents(&self) -> ([f64; 4], [f64; 4], usize) {
        match &self.0 {
            LawInZ::Printed(law) => ([law.b1, 0.0, 0.0, 0.0], [law.x, 0.0, 0.0, 0.0], 1),
            LawInZ::Between { nodes, blend } => (
                nodes.map(|node| node.b1),
                nodes.map(|node| node.x),
                blend.count(),
            ),
        }
    }
}

/// A fixed luminosity's powers L^b1 and L^x in a [`RadiusLaw`], at each of its nodes, which a
/// phase evaluates at every step of a track with the law at the current mass (plan 06's
/// integrator speed: its `pow` calls were a sixth of a giant's). The coefficient A of the law
/// follows the mass; b1 is a constant of the metallicity, and x is too except on the asymptotic
/// giant branch between a node's `M_HeF` − 0.2 and its `M_HeF` (between calibration metallicities,
/// each node has its own, so over about 1.62–2.04 M☉), where [`RadiusLaw::at_powers`] finds it
/// changed and evaluates it afresh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LuminosityPowers {
    /// ln L.
    ln_l: f64,
    /// b1 and L^b1 at each node of the law the powers were built from.
    b1: [[f64; 2]; 4],
    /// x and L^x at each node.
    x: [[f64; 2]; 4],
}

impl LuminosityPowers {
    /// The powers of `l` in `law`.
    #[must_use]
    pub(crate) fn new(law: &RadiusLaw, l: SolarLuminosities) -> Self {
        let l = l.value();
        debug_assert!(
            l > 0.0 && l.is_finite(),
            "a giant's luminosity is positive: {l}"
        );
        let ln_l = math::ln(l);
        let (b1, x, count) = law.exponents();
        let powers = |exponents: [f64; 4]| {
            let mut out = [[0.0; 2]; 4];
            for (slot, e) in out.iter_mut().zip(exponents).take(count) {
                *slot = [e, power_from_ln(ln_l, e)];
            }
            out
        };
        Self {
            ln_l,
            b1: powers(b1),
            x: powers(x),
        }
    }
}

/// The core mass at the base of the asymptotic giant branch (HPT section 5.3, equation 66):
/// (b36 M^b37 + b38)^¼.
#[must_use]
pub(crate) fn mc_bagb(m: SolarMasses, c: &ZCoeffs) -> SolarMasses {
    SolarMasses::new(math::powf_positive(
        c.b(36) * math::powf_positive(m.value(), c.b(37)) + c.b(38),
        0.25,
    ))
}

/// HPT equation 44 in M☉ for mass `m` in M☉: min(0.95 `Mc,BAGB`, (C + c₁ M^c₂)^¼), with C set so
/// that it meets the core mass–luminosity relation of `M_HeF` at luminosity `l_at_hef`.
#[must_use]
fn mc_intermediate(m: f64, l_at_hef: SolarLuminosities, c: &ZCoeffs) -> f64 {
    let m_hef = c.m_hef();
    let mc_at_hef = GiantBranch::new(m_hef, c).core_mass(l_at_hef).value();
    let constant =
        math::powi(mc_at_hef, 4) - MC_BGB_C1 * math::powf_positive(m_hef.value(), MC_BGB_C2);
    let cap = 0.95 * mc_bagb(SolarMasses::new(m), c).value();
    // `powf`, not `powf_positive`: below `M_HeF` the sum can be negative, and its NaN leaves the cap.
    cap.min(math::powf(
        constant + MC_BGB_C1 * math::powf_positive(m, MC_BGB_C2),
        0.25,
    ))
}

/// The core mass at the base of the giant branch from `M_HeF` up (HPT equation 44), which meets
/// the core mass–luminosity relation at `L_BGB`(`M_HeF`).
#[must_use]
pub(crate) fn mc_bgb(m: SolarMasses, c: &ZCoeffs) -> SolarMasses {
    SolarMasses::new(mc_intermediate(m.value(), ms::l_bgb(c.m_hef(), c), c))
}

/// The luminosity at helium ignition (HPT equation 49).
///
/// b9 M^b10 ÷ (1 + α₁ e^(15 (M − `M_HeF`))) below `M_HeF`, with α₁ set so that the two branches
/// meet there, and (b11 + b12 M^3.8) ÷ (b13 + M²) from `M_HeF` up.
#[must_use]
pub(crate) fn l_hei(m: SolarMasses, c: &ZCoeffs) -> SolarLuminosities {
    let m = m.value();
    let m_hef = c.m_hef().value();
    let high = |m: f64| (c.b(11) + c.b(12) * math::powf_positive(m, 3.8)) / (c.b(13) + m * m);
    SolarLuminosities::new(if m < m_hef {
        let at_hef = high(m_hef);
        let alpha1 = (c.b(9) * math::powf_positive(m_hef, c.b(10)) - at_hef) / at_hef;
        c.b(9) * math::powf_positive(m, c.b(10)) / (1.0 + alpha1 * math::exp(15.0 * (m - m_hef)))
    } else {
        high(m)
    })
}

/// The core mass at helium ignition (HPT section 5.3): from the core mass–luminosity relation at
/// [`l_hei`] below `M_HeF`, and equation 44 set to meet it at `M_HeF` from there up.
#[must_use]
pub(crate) fn mc_hei(m: SolarMasses, c: &ZCoeffs) -> SolarMasses {
    let m_hef = c.m_hef();
    if m.value() < m_hef.value() {
        GiantBranch::new(m, c).core_mass(l_hei(m, c))
    } else {
        SolarMasses::new(mc_intermediate(m.value(), l_hei(m_hef, c), c))
    }
}

/// The minimum radius of the blue loop from `M_HeF` up (HPT equation 55):
/// (b24 M + (b25 M)^b26 M^b28) ÷ (b27 + M^b28).
#[must_use]
pub(crate) fn r_mhe_intermediate(m: SolarMasses, c: &ZCoeffs) -> SolarRadii {
    let m = m.value();
    let m_b28 = math::powf_positive(m, c.b(28));
    SolarRadii::new(
        (c.b(24) * m + math::powf_positive(c.b(25) * m, c.b(26)) * m_b28) / (c.b(27) + m_b28),
    )
}

/// Below this, the blue-phase fraction of core helium burning is zero (the published SSE code's
/// `tblf` sets it to zero below 10⁻¹⁰).
pub(crate) const NO_BLUE_PHASE: f64 = 1e-10;

/// The blue loop's shape factor above `M_FGB` (HPT equation 58):
/// `f_bl`(M) = M^b48 (1 − `R_mHe` ÷ `R_AGB`(`L_HeI`))^b49, with the bracket held at or above 10⁻¹²
/// as the published SSE code holds it, so that the power stays real where the blue phase has
/// vanished.
#[must_use]
fn blue_shape(m: SolarMasses, c: &ZCoeffs) -> f64 {
    let ratio = r_mhe_intermediate(m, c) / agb_radius(m, l_hei(m, c), c);
    math::powf_positive(m.value(), c.b(48)) * math::powf_positive((1.0 - ratio).max(1e-12), c.b(49))
}

/// The fraction of core helium burning spent in the blue phase above `M_FGB` (HPT equation 58):
/// (1 − b47) `f_bl`(M) ÷ `f_bl`(`M_FGB`), held to 0–1 and zero below [`NO_BLUE_PHASE`].
#[must_use]
pub(crate) fn blue_fraction_massive(m: SolarMasses, c: &ZCoeffs) -> f64 {
    let tau = ((1.0 - c.b(47)) * blue_shape(m, c) / blue_shape(c.m_fgb(), c)).clamp(0.0, 1.0);
    if tau < NO_BLUE_PHASE { 0.0 } else { tau }
}

/// The radius at helium ignition (HPT equation 50).
///
/// The giant's radius at `L_HeI` up to `M_FGB`; the blue loop's minimum radius from
/// max(`M_FGB`, 12 M☉); and between them `R_mHe` (`R_GB`(`L_HeI`) ÷ `R_mHe`)^µ with
/// µ = log(M ÷ 12) ÷ log(`M_FGB` ÷ 12). Above `M_FGB`, a star without a blue phase (`τ_bl` = 0,
/// HPT section 5.3) ignites helium as a red supergiant, at `R_AGB`(`L_HeI`): the published SSE
/// code does so, and core helium burning, which then starts on the asymptotic-giant radius
/// (equation 64), needs it to begin where the Hertzsprung gap ends (design note 3). That covers
/// every star above `M_FGB` at Z ≳ 0.022, where 1 − b47 < 0, and elsewhere the masses where
/// `R_mHe` ≥ `R_AGB`(`L_HeI`) (from about 27 M☉ at Z = 0.02); the printed form would start core
/// helium burning with a fivefold jump in radius there. For the owner to confirm.
#[must_use]
pub(crate) fn r_hei(m: SolarMasses, c: &ZCoeffs) -> SolarRadii {
    r_hei_at(m, m, c)
}

/// [`r_hei`] for a star of initial mass `m0` whose current mass is `mt` (see [`IgnitionRadius`]).
///
/// Equal, bit for bit, to [`r_hei`] when `mt` is `m0`.
#[must_use]
pub(crate) fn r_hei_at(m0: SolarMasses, mt: SolarMasses, c: &ZCoeffs) -> SolarRadii {
    IgnitionRadius::new(m0, c).at(mt, c)
}

/// The radius at helium ignition (HPT equation 50, [`r_hei`]) of a star of one initial mass as a
/// function of its current mass, with the terms of the initial mass evaluated once.
///
/// HPT section 7.1 evaluate every radius formula at the current mass, so `R_GB` and `R_AGB` take
/// the current mass, while `L_HeI`, the blue-phase fraction that decides the regime, the regime's
/// bounds (`M_FGB`, 12 M☉) and equation 50's interpolation weight µ keep the initial mass, as the
/// published SSE code keeps them (`hrdiag`'s `texp`).
///
/// The blue loop's minimum radius `R_mHe` (equation 55) also keeps the initial mass, as in the
/// published SSE code (`hrdiag`'s `rminf(mass)` for stars above `M_FGB`), not the current mass the
/// paper's rule asks for. Above `M_FGB` a star that ignites helium inside the reach of the
/// Humphreys–Davidson limit loses its envelope to HPT's luminous-blue-variable wind, whose rate
/// goes as the cube of 10⁻⁵ R L^½ − 1: with `R_mHe` at the current mass the radius shrinks as the
/// mass goes, the wind chokes, and a 60 M☉ star at Z = 10⁻⁴ keeps some 25 M☉ of envelope through
/// core helium burning that SSE's loses within 10⁴ years of ignition, far beyond P06.T12.b's
/// tolerances (1% in mass, the remnant's kind). Ruling 40 of 2026-09-22 confirms SSE's form.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct IgnitionRadius {
    l_hei: SolarLuminosities,
    /// `L_HeI`'s powers in the giant's radius law, or in the asymptotic giant's for
    /// [`IgnitionForm::Asymptotic`].
    powers: LuminosityPowers,
    form: IgnitionForm,
}

/// Which branch of HPT equation 50 applies, by the initial mass.
#[derive(Debug, Clone, Copy, PartialEq)]
enum IgnitionForm {
    /// Up to `M_FGB`: the giant's radius at `L_HeI`.
    Giant,
    /// Above `M_FGB` with no blue phase: the asymptotic giant's radius at `L_HeI` ([`r_hei`]).
    Asymptotic,
    /// From 12 M☉ with a blue phase: `R_mHe` at the initial mass.
    Minimum(SolarRadii),
    /// Between `M_FGB` and 12 M☉ with a blue phase: `R_mHe` (`R_GB` ÷ `R_mHe`)^µ.
    Interpolated { r_mhe: f64, mu: f64 },
}

impl IgnitionRadius {
    /// The ignition radius of a star of initial mass `m0` at the metallicity of `c`.
    #[must_use]
    pub(crate) fn new(m0: SolarMasses, c: &ZCoeffs) -> Self {
        let m_fgb = c.m_fgb().value();
        let mass = m0.value();
        let form = if mass <= m_fgb {
            IgnitionForm::Giant
        } else if blue_fraction_massive(m0, c) <= 0.0 {
            IgnitionForm::Asymptotic
        } else if mass >= 12.0 {
            IgnitionForm::Minimum(r_mhe_intermediate(m0, c))
        } else {
            IgnitionForm::Interpolated {
                r_mhe: r_mhe_intermediate(m0, c).value(),
                mu: math::log10(mass / 12.0) / math::log10(m_fgb / 12.0),
            }
        };
        let l_hei = l_hei(m0, c);
        let law = match form {
            IgnitionForm::Asymptotic => RadiusLaw::asymptotic(m0, c),
            IgnitionForm::Giant | IgnitionForm::Minimum(_) | IgnitionForm::Interpolated { .. } => {
                RadiusLaw::giant(m0, c)
            }
        };
        Self {
            l_hei,
            powers: LuminosityPowers::new(&law, l_hei),
            form,
        }
    }

    /// The radius at ignition at current mass `mt`: [`radius`] or [`agb_radius`] at `L_HeI` as
    /// equation 50 takes them, bit for bit.
    #[must_use]
    pub(crate) fn at(&self, mt: SolarMasses, c: &ZCoeffs) -> SolarRadii {
        match self.form {
            IgnitionForm::Giant => RadiusLaw::giant(mt, c).at_powers(&self.powers),
            IgnitionForm::Asymptotic => RadiusLaw::asymptotic(mt, c).at_powers(&self.powers),
            IgnitionForm::Minimum(r_mhe) => r_mhe,
            IgnitionForm::Interpolated { r_mhe, mu } => {
                let giant = RadiusLaw::giant(mt, c).at_powers(&self.powers);
                SolarRadii::new(r_mhe * math::powf_positive(giant.value() / r_mhe, mu))
            }
        }
    }
}

/// A star of one mass below `M_FGB` on its first giant branch, from `t_BGB` to helium ignition at
/// `t_HeI` (HPT section 5.2).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FirstGiantBranch {
    /// Equation 46 at the star's mass and metallicity.
    radius_law: RadiusLaw,
    relation: GiantBranch,
    times: GiantTimes,
    t_bgb: Megayears,
    t_hei: Megayears,
    /// `Mc,BGB` and `Mc,HeI` for a non-degenerate core (`M_HeF` ≤ M), whose mass grows linearly in
    /// time between them (HPT equation 45); `None` for a degenerate core, which follows the core
    /// mass–luminosity relation.
    linear_core: Option<(SolarMasses, SolarMasses)>,
}

impl FirstGiantBranch {
    /// The giant branch of a star of mass `m`, below `M_FGB`, at the metallicity of `c`.
    ///
    /// # Panics
    ///
    /// In debug builds, if `m` is not below `M_FGB`: such a star ignites helium in the gap.
    #[must_use]
    pub(crate) fn new(m: SolarMasses, c: &ZCoeffs) -> Self {
        debug_assert!(
            m.value() < c.m_fgb().value(),
            "a star of {m:?} has no giant branch"
        );
        let relation = GiantBranch::new(m, c);
        let t_bgb = ms::t_bgb(m, c);
        let times = relation.times(t_bgb, ms::l_bgb(m, c));
        let t_hei = relation.time_of_luminosity(&times, l_hei(m, c));
        let linear_core = (m.value() >= c.m_hef().value()).then(|| (mc_bgb(m, c), mc_hei(m, c)));
        Self {
            radius_law: RadiusLaw::giant(m, c),
            relation,
            times,
            t_bgb,
            t_hei,
            linear_core,
        }
    }

    /// The time of helium ignition at the tip of the branch, `t_HeI` (HPT equation 43).
    #[must_use]
    pub(crate) const fn t_hei(&self) -> Megayears {
        self.t_hei
    }

    /// When the branch starts, `t_BGB`.
    #[must_use]
    pub(crate) const fn t_start(&self) -> Megayears {
        self.t_bgb
    }

    /// The core mass on the core mass–luminosity relation at `t` (HPT equation 39), which sets the
    /// luminosity; for a non-degenerate core it is the luminosity's clock, not the reported core.
    #[must_use]
    pub(crate) fn relation_core(&self, t: Megayears) -> SolarMasses {
        self.relation.core_mass_at(&self.times, t)
    }

    /// Whether the core the branch reports ([`FirstGiantBranch::core_mass`]) is the relation's
    /// ([`FirstGiantBranch::relation_core`]), bit for bit: below `M_HeF`, where the core is
    /// degenerate.
    #[must_use]
    pub(crate) const fn reports_relation_core(&self) -> bool {
        self.linear_core.is_none()
    }

    /// The core mass the branch reports at `t` (`FirstGiantBranch::at`'s).
    #[must_use]
    pub(crate) fn core_mass(&self, t: Megayears) -> SolarMasses {
        match self.linear_core {
            None => self.relation.core_mass_at(&self.times, t),
            Some((mc_bgb, mc_hei)) => {
                let tau = (t - self.t_bgb) / (self.t_hei - self.t_bgb);
                mc_bgb + (mc_hei - mc_bgb) * tau
            }
        }
    }

    /// Luminosity, radius and core mass at `t`, `t_BGB` ≤ t ≤ `t_HeI`.
    ///
    /// The luminosity follows the core mass–luminosity relation along equation 39's core; for a
    /// non-degenerate core that core is only the clock of the luminosity, and the reported core
    /// grows linearly from `Mc,BGB` to `Mc,HeI` (equation 45). The radius is equation 46.
    ///
    /// # Panics
    ///
    /// In debug builds, if `t` lies outside `t_BGB` ≤ t ≤ `t_HeI` by more than rounding.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn at(&self, t: Megayears) -> PhasePoint {
        self.point(t, &self.radius_law)
    }

    /// `FirstGiantBranch::at` for a star whose current mass `mt` has fallen below the mass the
    /// branch was built for: the radius is equation 46 at `mt` (HPT section 7.1), luminosity and
    /// core keep the initial mass. Equal, bit for bit, to `FirstGiantBranch::at` when `mt` is
    /// the branch's own mass.
    ///
    /// # Panics
    ///
    /// In debug builds, as `FirstGiantBranch::at`.
    #[must_use]
    pub(crate) fn at_mass(&self, t: Megayears, mt: SolarMasses, c: &ZCoeffs) -> PhasePoint {
        self.point(t, &RadiusLaw::giant(mt, c))
    }

    /// L, R and core at `t` with `radius_law` for equation 46.
    #[must_use]
    fn point(&self, t: Megayears, radius_law: &RadiusLaw) -> PhasePoint {
        let tau = (t - self.t_bgb) / (self.t_hei - self.t_bgb);
        debug_assert!(
            (-1e-9..=1.0 + 1e-9).contains(&tau),
            "the giant branch runs from t_BGB to t_HeI, not τ = {tau}"
        );
        let relation_core = self.relation.core_mass_at(&self.times, t);
        let luminosity = self.relation.luminosity(relation_core);
        let core_mass = match self.linear_core {
            None => relation_core,
            Some((mc_bgb, mc_hei)) => mc_bgb + (mc_hei - mc_bgb) * tau,
        };
        PhasePoint {
            luminosity,
            radius: radius_law.at(luminosity),
            core_mass,
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;

    use super::super::calibration::CALIBRATION_Z;
    use super::super::continuity::{assert_continuous_over, assert_no_jump};
    use super::*;
    use crate::units::MetalFraction;

    const REFERENCE_Z: [f64; 5] = [1e-4, 1e-3, 4e-3, 0.02, 0.03];

    fn coeffs(z: f64) -> ZCoeffs {
        ZCoeffs::new(MetalFraction::new(z))
    }

    fn mass(m: f64) -> SolarMasses {
        SolarMasses::new(m)
    }

    /// The relation's luminosity is the printed min(B Mc^q, D Mc^p), bit for bit, across the
    /// core masses, at `M_x` and its neighbours, for giant, early-AGB, pulsing and helium-giant
    /// relations. Both sides take the stellar formulae's power, `math::powf_positive`, whose domain
    /// is a positive core (ruling 77.1).
    #[test]
    fn the_relation_luminosity_is_the_printed_min_bit_for_bit() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            let relations = [0.5, 1.0, 1.99, 2.2, 2.5, 5.0, 12.0, 40.0]
                .map(|m| GiantBranch::new(mass(m), &c))
                .into_iter()
                .chain([0.4, 0.8, 2.0, 6.0].map(|m| GiantBranch::helium_giant(mass(m), 7.66e-5)));
            for relation in relations {
                let x = relation.m_x().value();
                let cores = (0..300)
                    .map(|i| math::exp10(-2.0 + 3.0 * f64::from(i) / 299.0))
                    .chain((-40..=40).map(|k| x * (1.0 + 1e-9 * f64::from(k))))
                    .chain([x, x.next_up(), x.next_down()]);
                for mc in cores {
                    let printed = (relation.b * math::powf_positive(mc, relation.q))
                        .min(relation.d * math::powf_positive(mc, relation.p));
                    let ours = relation.luminosity(mass(mc)).value();
                    assert_eq!(
                        bits(ours),
                        bits(printed),
                        "Z = {z}, Mc = {mc}: {relation:?}"
                    );
                }
            }
        }
    }

    /// A radius law at a fixed luminosity's powers is the law at that luminosity, bit for bit,
    /// whatever mass the law is at, including the asymptotic giant's band below `M_HeF`, where its
    /// exponent changes with the mass.
    #[test]
    fn a_radius_law_at_cached_powers_is_the_law_at_the_luminosity_bit_for_bit() {
        // The five reference metallicities and one in each interval between HPT's calibration
        // metallicities, where the law is the cubic through its nodes' (ruling 92).
        for z in REFERENCE_Z
            .into_iter()
            .chain([2e-4, 6e-4, 2.5e-3, 6.3e-3, 0.015, 0.025])
        {
            let c = coeffs(z);
            let m_hef = c.m_hef().value();
            for built_at in [0.8, m_hef - 0.1, m_hef, 5.0, 30.0] {
                for l in [3.0, 150.0, 2.4e3, 8.0e4] {
                    let l = SolarLuminosities::new(l);
                    let giant = LuminosityPowers::new(&RadiusLaw::giant(mass(built_at), &c), l);
                    let agb = LuminosityPowers::new(&RadiusLaw::asymptotic(mass(built_at), &c), l);
                    for i in 0..120 {
                        let mt = mass(math::exp10(-1.0 + 2.0 * f64::from(i) / 119.0));
                        let (g, a) = (RadiusLaw::giant(mt, &c), RadiusLaw::asymptotic(mt, &c));
                        assert_eq!(bits(g.at_powers(&giant).value()), bits(g.at(l).value()));
                        assert_eq!(bits(a.at_powers(&agb).value()), bits(a.at(l).value()));
                    }
                }
            }
        }
    }

    /// At each of HPT's calibration metallicities the radius laws are the Appendix's own, bit for
    /// bit: equation 46 as printed, and equation 74 as `agb_radius` evaluated it before ruling 92.
    #[test]
    fn the_radius_laws_are_the_printed_ones_at_the_calibration_metallicities() {
        for z in CALIBRATION_Z {
            let c = coeffs(z);
            let m_hef = c.m_hef().value();
            for m in [
                0.8,
                1.0,
                m_hef - 0.3,
                m_hef - 0.1,
                m_hef,
                3.0,
                7.0,
                20.0,
                60.0,
            ] {
                let low_a = c.b(56) + c.b(57) * m;
                let (x, a) = if m >= m_hef {
                    (c.b(55) * c.b(3), c.agb_radius_scale().at(m))
                } else if m <= m_hef - 0.2 {
                    (c.b(3), low_a)
                } else {
                    let f = (m - (m_hef - 0.2)) / 0.2;
                    let low = c.b(56) + c.b(57) * (m_hef - 0.2);
                    (
                        c.b(3) * (1.0 + (c.b(55) - 1.0) * f),
                        low + (c.agb_radius_scale().at(m_hef) - low) * f,
                    )
                };
                for l in [5.0, 100.0, 2.0e3, 3.0e4] {
                    let pow = |e: f64| math::powf_positive(l, e);
                    let giant = c.giant_radius_scale().at(m) * (pow(c.b(1)) + c.b(2) * pow(c.b(3)));
                    let agb = a * (pow(c.b(1)) + c.b(2) * pow(x));
                    let l = SolarLuminosities::new(l);
                    assert_eq!(
                        bits(radius(mass(m), l, &c).value()),
                        bits(giant),
                        "Z = {z}, M = {m}"
                    );
                    assert_eq!(
                        bits(agb_radius(mass(m), l, &c).value()),
                        bits(agb),
                        "Z = {z}, M = {m}"
                    );
                }
            }
        }
    }

    /// Both radius laws are continuous in Z across the range, at the calibration metallicities,
    /// where the cubic hands over to the node's own law, and across the Appendix's switches that
    /// the printed laws have between them (ruling 92).
    #[test]
    fn the_radius_laws_are_continuous_in_metallicity() {
        let (lo, hi) = (-4.0, math::log10(0.03));
        for (m, l) in [
            (1.0, 100.0),
            (1.0, 2.0e3),
            (1.4, 5.0e3),
            (2.0, 300.0),
            (5.0, 2.0e4),
        ] {
            let l = SolarLuminosities::new(l);
            let log_r = |law: fn(SolarMasses, SolarLuminosities, &ZCoeffs) -> SolarRadii| {
                move |log_z: f64| math::log10(law(mass(m), l, &coeffs(math::exp10(log_z))).value())
            };
            assert_no_jump(&format!("R_GB({m}, {l:?})"), log_r(radius), lo, hi, 1e-9);
            assert_no_jump(
                &format!("R_AGB({m}, {l:?})"),
                log_r(agb_radius),
                lo,
                hi,
                1e-9,
            );
        }
    }

    /// The red giant at 1 M☉ and 100 L☉ grows cooler with metallicity at every [Fe/H] from −2.2
    /// to +0.3 (ruling 92: metal-poor giants are hotter), where HPT's printed law made it 4,180 K at
    /// [Fe/H] −0.9 and 4,950 K at −0.6; and the giant at 2,000 L☉ and the asymptotic giant of
    /// 1.4 M☉ at 5,000 L☉ grow larger with it. Above [Fe/H] +0.176, Z is held at 0.03.
    #[test]
    fn metal_poor_giants_are_hotter() {
        let t_eff = |l: f64, r: SolarRadii| 5_772.0 * math::powf(l / (r.value() * r.value()), 0.25);
        let mut previous: Option<[f64; 3]> = None;
        for k in 0..=200 {
            let fe_h = -2.2 + 0.0125 * f64::from(k);
            let c = coeffs((0.02 * math::exp10(fe_h)).clamp(1e-4, 0.03));
            let now = [
                t_eff(100.0, radius(mass(1.0), SolarLuminosities::new(100.0), &c)),
                radius(mass(1.0), SolarLuminosities::new(2.0e3), &c).value(),
                agb_radius(mass(1.4), SolarLuminosities::new(5.0e3), &c).value(),
            ];
            if let Some([t, r_gb, r_agb]) = previous {
                assert!(
                    now[0] <= t,
                    "T_eff rises from {t} to {} K at [Fe/H] {fe_h}",
                    now[0]
                );
                assert!(
                    now[1] >= r_gb,
                    "R_GB falls from {r_gb} to {} at [Fe/H] {fe_h}",
                    now[1]
                );
                assert!(
                    now[2] >= r_agb,
                    "R_AGB falls from {r_agb} to {} at [Fe/H] {fe_h}",
                    now[2]
                );
            }
            previous = Some(now);
        }
    }

    /// Masses from 0.5 to 100 M☉, evenly in log mass.
    fn masses(n: u32) -> Vec<f64> {
        (0..n)
            .map(|i| math::exp10(-0.3 + 2.3 * f64::from(i) / f64::from(n - 1)))
            .collect()
    }

    #[test]
    fn luminosity_and_core_mass_are_inverses() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for m in masses(40) {
                let relation = GiantBranch::new(mass(m), &c);
                for k in 1..200 {
                    let mc = mass(0.1 + 1.3 * f64::from(k) / 200.0);
                    let back = relation.core_mass(relation.luminosity(mc));
                    assert!(
                        (back / mc - 1.0).abs() < 1e-10,
                        "M = {m}, Mc = {mc:?}: {back:?}"
                    );
                }
            }
        }
    }

    /// Equation 39's two branches meet at `t_x`, where both give the core `M_x`, for every star
    /// with a giant branch (below `M_FGB`).
    #[test]
    fn the_core_grows_continuously_through_t_x() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for m in masses(40).into_iter().filter(|&m| m < c.m_fgb().value()) {
                let m = mass(m);
                let g = GiantBranch::new(m, &c);
                let times = g.times(ms::t_bgb(m, &c), ms::l_bgb(m, &c));
                let t_x = times.t_x.value();
                let low = math::powf(
                    (g.p - 1.0) * g.a_h * g.d * (times.t_inf1.value() - t_x),
                    1.0 / (1.0 - g.p),
                );
                let high = math::powf(
                    (g.q - 1.0) * g.a_h * g.b * (times.t_inf2.value() - t_x),
                    1.0 / (1.0 - g.q),
                );
                assert!(
                    (low / g.m_x - 1.0).abs() < 1e-9,
                    "p branch at M = {m:?}, Z = {z}"
                );
                assert!(
                    (high / g.m_x - 1.0).abs() < 1e-9,
                    "q branch at M = {m:?}, Z = {z}"
                );
                let at_t_x = g.core_mass_at(&times, times.t_x).value();
                assert!(
                    (at_t_x / g.m_x - 1.0).abs() < 1e-9,
                    "core_mass_at(t_x) at M = {m:?}"
                );
                // Round trips through both regimes, before and after t_x.
                let before = f64::midpoint(times.t_x.value(), ms::t_bgb(m, &c).value());
                for t in [before, times.t_x.value() * (1.0 + 1e-7)] {
                    let t = Megayears::new(t);
                    let back =
                        g.time_of_luminosity(&times, g.luminosity(g.core_mass_at(&times, t)));
                    assert!(
                        (back / t - 1.0).abs() < 1e-9,
                        "round trip at M = {m:?}, Z = {z}"
                    );
                }
            }
        }
    }

    /// The branch ends at `t_HeI` with the ignition luminosity and core mass, which core helium
    /// burning starts from.
    #[test]
    fn the_branch_ends_at_helium_ignition() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for m in masses(60).into_iter().filter(|&m| m < c.m_fgb().value()) {
                let m = mass(m);
                let branch = FirstGiantBranch::new(m, &c);
                let tip = branch.at(branch.t_hei());
                let l = l_hei(m, &c).value();
                assert!(
                    (tip.luminosity.value() / l - 1.0).abs() < 1e-9,
                    "L at {m:?}, Z = {z}"
                );
                let mc = mc_hei(m, &c).value();
                assert!(
                    (tip.core_mass.value() / mc - 1.0).abs() < 1e-9,
                    "Mc at {m:?}, Z = {z}"
                );
                assert!(
                    branch.t_hei() > ms::t_bgb(m, &c),
                    "t_HeI ≤ t_BGB at {m:?}, Z = {z}"
                );
            }
        }
    }

    /// The tip of the giant branch of a 1 M☉ star at Z = 0.02: HPT's equation 49 gives 2,752 L☉
    /// (the published SSE code agrees), with a core of 0.477 M☉.
    #[test]
    fn a_solar_mass_star_ignites_helium_near_2_750_solar_luminosities_with_a_half_solar_mass_core()
    {
        let c = coeffs(0.02);
        let l = l_hei(mass(1.0), &c).value();
        let mc = mc_hei(mass(1.0), &c).value();
        assert!((l - 2_752.0).abs() < 1.0, "L_HeI = {l}");
        assert!((mc - 0.47).abs() < 0.02, "Mc,HeI = {mc}");
    }

    /// In HPT's fits a 1 M☉ star's tip luminosity rises with metallicity, from 1,933 L☉ at
    /// Z = 0.0001 to 2,814 L☉ at 0.03, swept at 200 metallicities.
    #[test]
    fn the_solar_mass_tip_luminosity_rises_with_metallicity() {
        let (lo, hi) = (math::log10(1e-4), math::log10(0.03));
        let mut last = 0.0;
        for i in 0..200 {
            let z = math::exp10(lo + (hi - lo) * f64::from(i) / 199.0).clamp(1e-4, 0.03);
            let l = l_hei(mass(1.0), &coeffs(z)).value();
            assert!(l > last, "L_HeI falls at Z = {z}");
            last = l;
        }
    }

    /// The parameters of the relation, the landmark luminosities and radii, and the core masses
    /// have no jump in mass, across `M_HeF`, 2.0–2.5 M☉ and `M_FGB`.
    #[test]
    fn landmarks_are_continuous_in_mass() {
        type Landmark = fn(SolarMasses, &ZCoeffs) -> f64;
        let landmarks: [(&str, Landmark); 6] = [
            ("M_x", |m, c| GiantBranch::new(m, c).m_x().value()),
            ("L_x", |m, c| GiantBranch::new(m, c).l_x().value()),
            ("L_HeI", |m, c| l_hei(m, c).value()),
            ("Mc_BAGB", |m, c| mc_bagb(m, c).value()),
            ("R_mHe", |m, c| r_mhe_intermediate(m, c).value()),
            ("R_AGB(L_HeI)", |m, c| agb_radius(m, l_hei(m, c), c).value()),
        ];
        let log_masses: Vec<f64> = masses(1_000).into_iter().map(math::log10).collect();
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for (name, f) in landmarks {
                let what = format!("{name} at Z = {z}");
                let log_f = |log_m: f64| math::log10(f(mass(math::exp10(log_m)), &c));
                assert_continuous_over(&what, log_f, &log_masses, 0.05, 1e-6);
            }
        }
    }

    /// The core mass at the base of the giant branch and at ignition meet the core
    /// mass–luminosity relation at `M_HeF`, where equation 44 takes over.
    #[test]
    fn core_masses_meet_the_relation_at_the_helium_flash_mass() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            let m_hef = c.m_hef();
            let relation = GiantBranch::new(m_hef, &c);
            let at_bgb = relation.core_mass(ms::l_bgb(m_hef, &c)).value();
            let at_hei = relation.core_mass(l_hei(m_hef, &c)).value();
            let bgb = mc_bgb(m_hef, &c).value();
            let capped = 0.95 * mc_bagb(m_hef, &c).value() < at_bgb;
            assert!(
                (bgb - at_bgb).abs() < 1e-9 || capped,
                "Mc,BGB at M_HeF, Z = {z}: {bgb} against {at_bgb}"
            );
            let below = mc_hei(mass(m_hef.value() * (1.0 - 1e-12)), &c).value();
            assert!((mc_hei(m_hef, &c).value() - at_hei).abs() < 1e-9, "Z = {z}");
            assert!(
                (below - at_hei).abs() < 1e-6,
                "Mc,HeI jumps at M_HeF, Z = {z}"
            );
        }
    }

    /// Luminosity, radius and core mass on the giant branch from the published SSE code (see
    /// [`sse`](super::super) for the run; `hrdiag` with no mass loss), to 10⁻⁹, at points where
    /// its small-envelope perturbation (HPT section 6.3) does not act. `Mc` agrees to 10⁻⁷: SSE's
    /// c₁ of equation 44 is 0.09796164⁴ = 9.2092484 × 10⁻⁵, which the paper rounds to 9.20925 × 10⁻⁵.
    #[test]
    fn matches_the_published_sse_code() {
        for &(z, m, t, l_sse, r_sse, mc_sse) in SSE_GB {
            let c = coeffs(z);
            let point = FirstGiantBranch::new(mass(m), &c).at(Megayears::new(t));
            let (l, r, mc) = (
                point.luminosity.value(),
                point.radius.value(),
                point.core_mass.value(),
            );
            assert!(
                (l / l_sse - 1.0).abs() < 1e-9,
                "L at Z = {z}, M = {m}: {l} against {l_sse}"
            );
            assert!(
                (r / r_sse - 1.0).abs() < 1e-9,
                "R at Z = {z}, M = {m}: {r} against {r_sse}"
            );
            assert!(
                (mc / mc_sse - 1.0).abs() < 1e-7,
                "Mc at Z = {z}, M = {m}: {mc} against {mc_sse}"
            );
        }
    }

    /// (Z, M, t Myr, L, R, Mc) from SSE's `hrdiag` on the first giant branch.
    const SSE_GB: &[(f64, f64, f64, f64, f64, f64)] = &[
        (
            0.02,
            1.0,
            11_942.630_337_812_952,
            5.560_803_095_458_498,
            3.360_690_661_115_715,
            0.169_507_760_538_893_8,
        ),
        (
            0.02,
            1.0,
            12_302.545_224_705_946,
            149.607_961_255_819_84,
            23.996_722_404_709_75,
            0.293_421_849_887_913_53,
        ),
        (
            0.0001,
            0.8,
            13_913.027_343_024_94,
            34.027_274_992_255_14,
            7.818_995_912_268_948_5,
            0.258_272_224_646_063,
        ),
        (
            0.001,
            2.0,
            801.821_611_815_865_7,
            112.686_363_732_665_27,
            13.819_134_079_527_865,
            0.317_132_463_219_475_3,
        ),
        (
            0.03,
            5.0,
            105.644_535_669_076_13,
            900.114_377_974_370_3,
            49.502_824_816_771_344,
            0.860_626_408_924_415_6,
        ),
        (
            0.004,
            1.75,
            1_358.107_160_512_302_7,
            146.371_915_916_895_72,
            17.329_863_812_353_647,
            0.303_135_257_140_434_8,
        ),
    ];
}
