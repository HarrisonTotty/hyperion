//! Core helium burning: the horizontal branch and red clump of low-mass stars, and the blue loops
//! of intermediate-mass and massive stars (plan 06, P06.T7; Hurley, Pols and Tout 2000, MNRAS 315,
//! 543, "HPT", section 5.3).
//!
//! Three regimes, split at the critical masses of [`ZCoeffs`]: below `M_HeF` helium ignited in a
//! flash and the whole phase is "blue", starting on the zero-age horizontal branch, whose place
//! depends on the envelope mass through µ (equation 52); from `M_HeF` to `M_FGB` the star first
//! descends the giant branch to a minimum luminosity and then makes its blue loop; from `M_FGB` up
//! it ignites helium in the Hertzsprung gap, burns first blue and then as a red supergiant. HPT
//! declare the formulae discontinuous between the last two regimes at `M_FGB` for Z > 0.002.
//!
//! The `pub(crate)` items take and return unit newtypes in HPT's units (M☉, L☉, R☉, Myr from the
//! zero-age main sequence). Equation numbers are the journal's, which for this section are the
//! arXiv preprint's too.
//!
//! The comparison tests use the SSE build of [`sse`](super), run again on 2026-09-23 for these
//! phases: `hrdiag` at constant mass on finer age grids, for helium stars from 0.3 to 50 M☉, and at
//! the hand-over to a helium star, and a second time with the branch that applies the
//! small-envelope perturbation of HPT section 6.3 (equations 97–100) disabled, for rows where SSE
//! applies it; each test says which run it reads.

use crate::math;
use crate::units::{Megayears, SolarLuminosities, SolarMasses, SolarRadii};

use super::PhasePoint;
use super::coeffs::ZCoeffs;
use super::gb::{self, FirstGiantBranch, LuminosityPowers, RadiusLaw};
use super::helium;
use super::ms;

/// The exponent of M ÷ `M_FGB` in the intermediate-mass branch of equation 58, from the published
/// SSE code (its `zdata.h`, the coefficient that `zcnsts` stores as `gbp(63)`); see
/// [`blue_fraction`] for why it replaces the printed 0.414.
const BLUE_FRACTION_EXPONENT: f64 = 0.480_542_8;

/// The exponent of µ in the denominator of equation 53, as printed; the published SSE code carries
/// one more digit, 1.647903, which moves `L_ZAHB` by under 10⁻⁵ of itself.
const ZAHB_EXPONENT: f64 = 1.6479;

/// The time of helium ignition, `t_HeI`, Myr from the zero-age main sequence: the tip of the giant
/// branch below `M_FGB` (HPT equation 43), the end of the Hertzsprung gap, `t_BGB`, from there up.
#[must_use]
pub(crate) fn t_hei(m: SolarMasses, c: &ZCoeffs) -> Megayears {
    if m.value() < c.m_fgb().value() {
        FirstGiantBranch::new(m, c).t_hei()
    } else {
        ms::t_bgb(m, c)
    }
}

/// The minimum luminosity of core helium burning from `M_HeF` to `M_FGB`, reached at the start of
/// the blue loop (HPT equation 51): `L_HeI` (b14 + c M^(b15 + 0.1)) ÷ (b16 + M^b15), with
/// c = b17 ÷ `M_FGB`^0.1 + (b16 b17 − b14) ÷ `M_FGB`^(b15 + 0.1), so that it is b17 `L_HeI` at
/// `M_FGB`.
#[must_use]
pub(crate) fn l_min_he(m: SolarMasses, c: &ZCoeffs) -> SolarLuminosities {
    let mass = m.value();
    let m_fgb = c.m_fgb().value();
    let (b14, b15, b16, b17) = (c.b(14), c.b(15), c.b(16), c.b(17));
    let c_fit = b17 / math::powf_positive(m_fgb, 0.1)
        + (b16 * b17 - b14) / math::powf_positive(m_fgb, b15 + 0.1);
    gb::l_hei(m, c)
        * ((b14 + c_fit * math::powf_positive(mass, b15 + 0.1))
            / (b16 + math::powf_positive(mass, b15)))
}

/// The luminosity of the zero-age horizontal branch of a star of mass `m` below `M_HeF` with a
/// helium core of `mc` (HPT equation 53; see [`ZeroAgeHorizontalBranch::luminosity`]).
#[cfg(test)]
#[must_use]
pub(crate) fn l_zahb(m: SolarMasses, mc: SolarMasses, c: &ZCoeffs) -> SolarLuminosities {
    ZeroAgeHorizontalBranch::new(m, c).luminosity(mc)
}

/// The radius of the zero-age horizontal branch of a star of mass `m` below `M_HeF` with a helium
/// core of `mc` (HPT equation 54; see [`ZeroAgeHorizontalBranch::radius`]).
#[cfg(test)]
#[must_use]
pub(crate) fn r_zahb(m: SolarMasses, mc: SolarMasses, c: &ZCoeffs) -> SolarRadii {
    ZeroAgeHorizontalBranch::new(m, c).radius(mc)
}

/// The zero-age horizontal branch of a star of one mass below `M_HeF` as a function of its core
/// mass (HPT equations 52–54), with the mass and metallicity evaluated once.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ZeroAgeHorizontalBranch {
    mass: f64,
    m_hef: f64,
    /// `L_min,He`(`M_HeF`), L☉.
    l_min_hef: f64,
    /// b18–b23.
    b: [f64; 6],
    giant: RadiusLaw,
}

impl ZeroAgeHorizontalBranch {
    /// The branch of a star of mass `m` at the metallicity of `c`.
    #[must_use]
    fn new(m: SolarMasses, c: &ZCoeffs) -> Self {
        Self {
            mass: m.value(),
            m_hef: c.m_hef().value(),
            l_min_hef: l_min_he(c.m_hef(), c).value(),
            b: core::array::from_fn(|i| c.b(18 + i)),
            giant: RadiusLaw::giant(m, c),
        }
    }

    /// The same branch for a star of mass `m`: what [`ZeroAgeHorizontalBranch::new`] gives at `m`,
    /// bit for bit, reusing the terms that depend on the metallicity alone.
    #[must_use]
    fn at_mass(&self, m: SolarMasses, c: &ZCoeffs) -> Self {
        Self {
            mass: m.value(),
            giant: RadiusLaw::giant(m, c),
            ..*self
        }
    }

    /// µ of HPT equation 52 for a core of `mc`, (M − Mc) ÷ (`M_HeF` − Mc), held to the paper's
    /// range 0–1.
    #[must_use]
    fn envelope_fraction(&self, mc: SolarMasses) -> f64 {
        let mc = mc.value();
        ((self.mass - mc) / (self.m_hef - mc)).clamp(0.0, 1.0)
    }

    /// µ^`exponent`: [`math::powf_positive`] for a positive µ, and [`math::powf`] at the envelope's
    /// end, µ = 0, which is outside its domain.
    #[must_use]
    fn envelope_power(mu: f64, exponent: f64) -> f64 {
        if mu > 0.0 {
            math::powf_positive(mu, exponent)
        } else {
            math::powf(mu, exponent)
        }
    }

    /// `L_ZAHB` (HPT equation 53):
    /// `L_ZHe`(Mc) + (1 + b20) ÷ (1 + b20 µ^1.6479) × b18 µ^b19 ÷ (1 + α₂ e^(15 (M − `M_HeF`))),
    /// with α₂ = (b18 + `L_ZHe`(Mc) − `L_min,He`(`M_HeF`)) ÷ (`L_min,He`(`M_HeF`) − `L_ZHe`(Mc)).
    ///
    /// As the envelope vanishes (µ → 0) it meets the naked helium star of the core's mass (equation
    /// 77); at `M_HeF` (µ = 1) it meets [`l_min_he`] whatever the core.
    #[must_use]
    fn luminosity(&self, mc: SolarMasses) -> SolarLuminosities {
        let mu = self.envelope_fraction(mc);
        let l_zhe = helium::zams_luminosity(mc).value();
        let [b18, b19, b20, ..] = self.b;
        let alpha2 = (b18 + l_zhe - self.l_min_hef) / (self.l_min_hef - l_zhe);
        SolarLuminosities::new(
            l_zhe
                + (1.0 + b20) * b18 * Self::envelope_power(mu, b19)
                    / ((1.0 + b20 * Self::envelope_power(mu, ZAHB_EXPONENT))
                        * (1.0 + alpha2 * math::exp(15.0 * (self.mass - self.m_hef)))),
        )
    }

    /// `R_ZAHB` (HPT equation 54): (1 − f) `R_ZHe`(Mc) + f `R_GB`(`L_ZAHB`), with
    /// f = (1 + b21) µ^b22 ÷ (1 + b21 µ^b23), smaller than the giant's radius at the same
    /// luminosity.
    #[must_use]
    fn radius(&self, mc: SolarMasses) -> SolarRadii {
        let mu = self.envelope_fraction(mc);
        let [.., b21, b22, b23] = self.b;
        let f = (1.0 + b21) * Self::envelope_power(mu, b22)
            / (1.0 + b21 * Self::envelope_power(mu, b23));
        helium::zams_radius(mc) * (1.0 - f) + self.giant.at(self.luminosity(mc)) * f
    }
}

/// The blue loop's minimum radius below `M_HeF` (HPT equation 55, its second form):
/// `R_GB`(`L_ZAHB`) (`R_mHe`(`M_HeF`) ÷ `R_GB`(`L_ZAHB`(`M_HeF`)))^(M ÷ `M_HeF`), for a star of mass
/// `m` whose horizontal branch starts at `l_zahb`.
///
/// `L_ZAHB`(`M_HeF`) is `L_min,He`(`M_HeF`) whatever the core (equation 53 at µ = 1), so no core
/// mass enters the constant; the published SSE code evaluates it with the core at the base of the
/// giant branch, which agrees to rounding. `at_hef` is that constant, from
/// [`r_mhe_low_constant`].
#[must_use]
fn r_mhe_low_with(
    m: SolarMasses,
    l_zahb: &LuminosityPowers,
    at_hef: f64,
    c: &ZCoeffs,
) -> SolarRadii {
    RadiusLaw::giant(m, c).at_powers(l_zahb)
        * math::powf_positive(at_hef, m.value() / c.m_hef().value())
}

/// `R_mHe`(`M_HeF`) ÷ `R_GB`(`L_ZAHB`(`M_HeF`)), the constant of [`r_mhe_low_with`].
#[must_use]
fn r_mhe_low_constant(c: &ZCoeffs) -> f64 {
    let m_hef = c.m_hef();
    gb::r_mhe_intermediate(m_hef, c) / gb::radius(m_hef, l_min_he(m_hef, c), c)
}

/// The luminosity at the base of the asymptotic giant branch, where core helium burning ends (HPT
/// equation 56).
///
/// b29 M^b30 ÷ (1 + α₃ e^(15 (M − `M_HeF`))) below `M_HeF`, with α₃ set so that the two branches
/// meet there, and (b31 + b32 M^(b33 + 1.8)) ÷ (b34 + M^b33) from `M_HeF` up.
#[must_use]
pub(crate) fn l_bagb(m: SolarMasses, c: &ZCoeffs) -> SolarLuminosities {
    let mass = m.value();
    let m_hef = c.m_hef().value();
    let high = |m: f64| {
        (c.b(31) + c.b(32) * math::powf_positive(m, c.b(33) + 1.8))
            / (c.b(34) + math::powf_positive(m, c.b(33)))
    };
    SolarLuminosities::new(if mass < m_hef {
        let at_hef = high(m_hef);
        let alpha3 = (c.b(29) * math::powf_positive(m_hef, c.b(30)) - at_hef) / at_hef;
        c.b(29) * math::powf_positive(mass, c.b(30))
            / (1.0 + alpha3 * math::exp(15.0 * (mass - m_hef)))
    } else {
        high(mass)
    })
}

/// The lifetime of core helium burning, `t_He` (HPT equation 57).
///
/// Below `M_HeF`: (b39 + (`t_HeMS`(Mc) − b39)(1 − µ)^b40)(1 + α₄ e^(15 (M − `M_HeF`))), with the core
/// at helium ignition, so that it meets the helium main-sequence lifetime of the core (equation 79)
/// as the envelope vanishes, and α₄ = (`t_He`(`M_HeF`) − b39) ÷ b39. From `M_HeF` up:
/// `t_BGB` (b41 M^b42 + b43 M⁵) ÷ (b44 + M⁵). The factor 1 − µ is formed as
/// (`M_HeF` − M) ÷ (`M_HeF` − Mc), which equals it.
#[must_use]
pub(crate) fn t_he(m: SolarMasses, c: &ZCoeffs) -> Megayears {
    if m.value() < c.m_hef().value() {
        t_he_low(m, gb::mc_hei(m, c), c)
    } else {
        t_he_high(m, c)
    }
}

/// Equation 57 from `M_HeF` up: `t_BGB` (b41 M^b42 + b43 M⁵) ÷ (b44 + M⁵).
#[must_use]
fn t_he_high(m: SolarMasses, c: &ZCoeffs) -> Megayears {
    let m5 = math::powi(m.value(), 5);
    ms::t_bgb(m, c)
        * ((c.b(41) * math::powf_positive(m.value(), c.b(42)) + c.b(43) * m5) / (c.b(44) + m5))
}

/// Equation 57 below `M_HeF` for a star of mass `m` with a core of `mc` at ignition.
#[must_use]
fn t_he_low(m: SolarMasses, mc: SolarMasses, c: &ZCoeffs) -> Megayears {
    let (mass, m_hef) = (m.value(), c.m_hef().value());
    let complement = ((m_hef - mass) / (m_hef - mc.value())).clamp(0.0, 1.0);
    let b39 = c.b(39);
    let alpha4 = (t_he_high(c.m_hef(), c).value() - b39) / b39;
    let t_hems = helium::main_sequence_lifetime(mc).value();
    Megayears::new(
        // `powf`, not `powf_positive`, here and for `depth`, `τ_bl` and λ below: the base can be 0.
        (b39 + (t_hems - b39) * math::powf(complement, c.b(40)))
            * (1.0 + alpha4 * math::exp(15.0 * (mass - m_hef))),
    )
}

/// The fraction of core helium burning spent in the blue phase, `τ_bl` (HPT equation 58), held to
/// 0–1 and zero below [`gb::NO_BLUE_PHASE`].
///
/// One below `M_HeF`. From `M_HeF` to `M_FGB`: b45 (M ÷ `M_FGB`)^x + `a_bl` (log(M ÷ `M_FGB`) ÷
/// log(`M_HeF` ÷ `M_FGB`))^b46, with `a_bl` = 1 − b45 (`M_HeF` ÷ `M_FGB`)^x so that it is one at
/// `M_HeF` (the paper writes the ratio as a product of powers of two negative logarithms). From
/// `M_FGB` up, [`gb::blue_fraction_massive`].
///
/// The paper prints x = 0.414; the published SSE code has x = 0.4805428. The printed exponent
/// moves `τ_bl` by up to 0.035 (4 M☉ at Z = 0.001), and with it the radius at a given age during
/// core helium burning by up to 0.12 dex and the luminosity by up to 0.06 dex against the code (5
/// M☉ at Z = 0.001), beyond the 0.02 dex P06.T12.b validates to, so the code's exponent is used,
/// pending the owner's confirmation.
#[must_use]
pub(crate) fn blue_fraction(m: SolarMasses, c: &ZCoeffs) -> f64 {
    let (mass, m_hef, m_fgb) = (m.value(), c.m_hef().value(), c.m_fgb().value());
    if mass < m_hef {
        return 1.0;
    }
    if mass >= m_fgb {
        return gb::blue_fraction_massive(m, c);
    }
    let b45 = c.b(45);
    let a_bl = 1.0 - b45 * math::powf_positive(m_hef / m_fgb, BLUE_FRACTION_EXPONENT);
    let depth = (math::log10(mass / m_fgb) / math::log10(m_hef / m_fgb)).max(0.0);
    let tau = (b45 * math::powf_positive(mass / m_fgb, BLUE_FRACTION_EXPONENT)
        + a_bl * math::powf(depth, c.b(46)))
    .clamp(0.0, 1.0);
    if tau < gb::NO_BLUE_PHASE { 0.0 } else { tau }
}

/// A star of one mass burning helium in its core, from helium ignition at `t_HeI` to the base of
/// the asymptotic giant branch at `t_HeI` + `t_He` (HPT section 5.3).
///
/// The phase runs in the relative age τ = (t − `t_HeI`) ÷ `t_He`. The blue phase lies between
/// `τ_x` and `τ_y`: the whole phase below `M_HeF` (`τ_x` = 0, `τ_y` = 1), its end from `M_HeF` to
/// `M_FGB` (`τ_x` = 1 − `τ_bl`, `τ_y` = 1), and its start from `M_FGB` up (`τ_x` = 0,
/// `τ_y` = `τ_bl`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CoreHeliumBurning {
    /// The (effective initial) mass the phase was built for, M☉.
    mass: SolarMasses,
    regime: Regime,
    t_hei: Megayears,
    t_he: Megayears,
    mc_hei: SolarMasses,
    mc_bagb: SolarMasses,
    l_hei: SolarLuminosities,
    l_x: SolarLuminosities,
    l_bagb: SolarLuminosities,
    tau_bl: f64,
    tau_x: f64,
    tau_y: f64,
    /// The zero-age horizontal branch at the phase's mass, below `M_HeF`.
    zahb: Option<ZeroAgeHorizontalBranch>,
    /// `R_mHe`(`M_HeF`) ÷ `R_GB`(`L_ZAHB`(`M_HeF`)), the constant of equation 55 below `M_HeF`.
    at_hef: f64,
    /// The radius at helium ignition from `M_FGB` up.
    ignition: Option<gb::IgnitionRadius>,
    /// From `M_FGB` up, what the blue phase's shape reads at the initial mass whatever the current
    /// one: `R_mHe` and equation 50's exponent µ (see [`CoreHeliumBurning::radii_at`]).
    high_blue: Option<HighBlue>,
    /// The radius formulae at [`CoreHeliumBurning::mass`].
    radii: Radii,
    /// The constant luminosities' powers in the radius laws at the current mass: `L_x` and
    /// `L_HeI` in the giant's, `L_BAGB` in the asymptotic giant's.
    powers: PhasePowers,
}

/// [`CoreHeliumBurning`]'s constant luminosities' powers in its radius laws.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PhasePowers {
    /// `L_x`'s, in the giant's law.
    blue_start: LuminosityPowers,
    /// `L_HeI`'s, in the giant's law.
    ignition: LuminosityPowers,
    /// `L_BAGB`'s, in the asymptotic giant's law.
    agb_base: LuminosityPowers,
}

/// The terms of the blue phase's shape from `M_FGB` up that keep the initial mass: `R_mHe`
/// (equation 55) and, below 12 M☉, µ = log(M ÷ 12) ÷ log(`M_FGB` ÷ 12) of equation 50. They are
/// constants of the phase, evaluated once rather than at every step of the track.
#[derive(Debug, Clone, Copy, PartialEq)]
struct HighBlue {
    r_mhe: SolarRadii,
    /// µ, where M is below 12 M☉.
    mu: Option<f64>,
}

impl HighBlue {
    /// The terms for a star of initial mass `m` at the metallicity of `c`.
    #[must_use]
    fn new(m: SolarMasses, c: &ZCoeffs) -> Self {
        let (m0, m_fgb) = (m.value(), c.m_fgb().value());
        Self {
            r_mhe: gb::r_mhe_intermediate(m, c),
            mu: (m0 < 12.0).then(|| math::log10(m0 / 12.0) / math::log10(m_fgb / 12.0)),
        }
    }

    /// ξ of equation 62 from `R_mHe` and equation 50's printed radius at ignition, `R_mHe`
    /// (`R_GB`(`L_HeI`) ÷ `R_mHe`)^µ below 12 M☉ with `giant` at the current mass: with the radius,
    /// what [`BlueShape::new`] makes of them.
    #[must_use]
    fn printed_and_xi(&self, giant: &RadiusLaw, l_hei: &LuminosityPowers) -> (SolarRadii, f64) {
        let r_mhe = self.r_mhe;
        let printed = match self.mu {
            None => r_mhe,
            Some(mu) => SolarRadii::new(
                r_mhe.value() * math::powf_positive(giant.at_powers(l_hei) / r_mhe, mu),
            ),
        };
        (printed, (r_mhe / printed).clamp(0.4, 2.5))
    }
}

/// Which of HPT's three regimes of core helium burning a star is in, by its mass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Regime {
    /// Below `M_HeF`: the horizontal branch after the helium flash.
    Low,
    /// From `M_HeF` to `M_FGB`: a descent of the giant branch, then the blue loop.
    Intermediate,
    /// From `M_FGB` up: ignition in the Hertzsprung gap, a blue phase, then red.
    High,
}

/// The radius formulae of the phase at one current mass: the giant and asymptotic-giant laws and
/// the blue phase's shape.
#[derive(Debug, Clone, PartialEq)]
struct Radii {
    blue: BluePhase,
    giant: RadiusLaw,
    asymptotic: RadiusLaw,
}

/// The shape of the blue phase: ξ of equation 62 and the radius of equations 64 and 65.
#[derive(Debug, Clone, PartialEq)]
enum BluePhase {
    /// From `M_HeF` up, where `R_x` is fixed at the start of the blue phase.
    Fixed(BlueShape),
    /// Below `M_HeF`, where `R_x` = `R_ZAHB` follows the growing core (see
    /// [`CoreHeliumBurning::new`]).
    HorizontalBranch {
        zahb: ZeroAgeHorizontalBranch,
        r_mhe: SolarRadii,
        r_y: SolarRadii,
    },
}

/// ξ, `R_min` and the cube roots of equation 65 for one `R_x`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BlueShape {
    xi: f64,
    r_min: SolarRadii,
    /// (ln(`R_x` ÷ `R_min`))^⅓.
    rho_x: f64,
    /// (ln(`R_y` ÷ `R_min`))^⅓.
    rho_y: f64,
}

impl BlueShape {
    /// The shape for a blue phase from `r_x` towards `r_y` with minimum radius `r_mhe`, with ξ from
    /// `r_x_for_xi`: ξ = min(2.5, max(0.4, `R_mHe` ÷ `R_x`)) (equation 62) and
    /// `R_min` = min(`R_mHe`, `R_x`).
    #[must_use]
    fn new(r_x: SolarRadii, r_mhe: SolarRadii, r_x_for_xi: SolarRadii, r_y: SolarRadii) -> Self {
        let r_min = if r_mhe < r_x { r_mhe } else { r_x };
        Self {
            xi: (r_mhe / r_x_for_xi).clamp(0.4, 2.5),
            r_min,
            rho_x: math::cbrt(math::ln(r_x / r_min)),
            rho_y: math::cbrt(math::ln(r_y / r_min).max(0.0)),
        }
    }
}

impl CoreHeliumBurning {
    /// Core helium burning of a star of mass `m` at the metallicity of `c`.
    ///
    /// Below `M_HeF`, `m` is the mass the star has on reaching the horizontal branch: HPT section
    /// 7.1 reset the initial mass to the current one at the helium flash, so a star that lost mass
    /// on the giant branch starts core helium burning as a new star of its remaining mass, with the
    /// core of equation 49's relation at that mass; its envelope sets the horizontal branch through
    /// µ (equation 52).
    ///
    /// The luminosity and radius where the blue phase starts, `L_x` and `R_x`, are those of the
    /// zero-age horizontal branch (equations 53 and 54, with the core at ignition) below `M_HeF`,
    /// of the minimum luminosity (51) on the giant branch up to `M_FGB`, and of helium ignition from
    /// there up (59, 60). The luminosity rises from `L_x` towards `L_BAGB` as λ = ((τ − `τ_x`) ÷
    /// (1 − `τ_x`))^ξ with ξ = min(2.5, max(0.4, `R_mHe` ÷ `R_x`)) (61, 62), and before `τ_x` falls
    /// from `L_HeI` as λ′ = ((`τ_x` − τ) ÷ `τ_x`)³ (63). The blue phase's radius passes through
    /// `R_min` = min(`R_mHe`, `R_x`) as `R_min` exp(|ρ|³) (64, 65) towards `R_y` = `R_AGB`(`L_y`),
    /// and outside it is `R_GB`(L) before and `R_AGB`(L) after.
    ///
    /// Below `M_HeF` the published SSE code evaluates `R_x` = `R_ZAHB` (equation 54) with the
    /// current core, which grows through the phase, and ξ and `R_min` with it, where the paper
    /// defines `R_x` at the start of the blue phase. With `R_x` held at the core at ignition the
    /// radius of a 0.7 M☉ star at Z = 0.0001 moves by up to 0.030 dex against the code (0.024 dex
    /// at Z = 0.001; under 0.016 dex elsewhere in the 5 Z × 31 mass grid) and the luminosity by up
    /// to 0.009 dex, beyond the 0.02 dex P06.T12.b validates to, so the code's form is used,
    /// pending the owner's confirmation. Both forms meet `R_ZAHB` at ignition and `R_AGB`(`L_BAGB`)
    /// at the end.
    ///
    /// From `M_FGB` up, `R_x` is [`gb::r_hei`], where the Hertzsprung gap ends, and ξ takes
    /// equation 50's `R_HeI` as printed: they differ only for a star with no blue phase, which
    /// starts on the asymptotic-giant radius (see [`gb::r_hei`]).
    #[must_use]
    pub(crate) fn new(m: SolarMasses, c: &ZCoeffs) -> Self {
        let (mass, m_hef, m_fgb) = (m.value(), c.m_hef().value(), c.m_fgb().value());
        let t_hei = t_hei(m, c);
        let mc_hei = gb::mc_hei(m, c);
        let l_hei = gb::l_hei(m, c);
        let l_bagb = l_bagb(m, c);
        let tau_bl = blue_fraction(m, c);
        let zahb = (mass < m_hef).then(|| ZeroAgeHorizontalBranch::new(m, c));
        let (regime, l_x, tau_x, tau_y) = if let Some(zahb) = &zahb {
            (Regime::Low, zahb.luminosity(mc_hei), 0.0, 1.0)
        } else if mass < m_fgb {
            (Regime::Intermediate, l_min_he(m, c), 1.0 - tau_bl, 1.0)
        } else {
            (Regime::High, l_hei, 0.0, tau_bl)
        };
        let mut phase = Self {
            mass: m,
            regime,
            t_hei,
            t_he: t_he(m, c),
            mc_hei,
            mc_bagb: gb::mc_bagb(m, c),
            l_hei,
            l_x,
            l_bagb,
            tau_bl,
            tau_x,
            tau_y,
            zahb,
            at_hef: r_mhe_low_constant(c),
            ignition: (regime == Regime::High).then(|| gb::IgnitionRadius::new(m, c)),
            high_blue: (regime == Regime::High).then(|| HighBlue::new(m, c)),
            powers: {
                let (giant, asymptotic) = (RadiusLaw::giant(m, c), RadiusLaw::asymptotic(m, c));
                PhasePowers {
                    blue_start: LuminosityPowers::new(&giant, l_x),
                    ignition: LuminosityPowers::new(&giant, l_hei),
                    agb_base: LuminosityPowers::new(&asymptotic, l_bagb),
                }
            },
            radii: Radii {
                blue: BluePhase::Fixed(BlueShape {
                    xi: 1.0,
                    r_min: SolarRadii::ZERO,
                    rho_x: 0.0,
                    rho_y: 0.0,
                }),
                giant: RadiusLaw::giant(m, c),
                asymptotic: RadiusLaw::asymptotic(m, c),
            },
        };
        phase.radii = phase.radii_at(m, c);
        phase
    }

    /// The radius formulae at current mass `mt` (HPT section 7.1: "we use Mt in all radius
    /// formulae"): the giant and asymptotic-giant radii (equations 46 and 74), the zero-age
    /// horizontal branch's (54) and, below `M_FGB`, the blue loop's minimum radius (55) take `mt`;
    /// the luminosities `L_x`, `L_BAGB` and `L_HeI`, the timescales and the blue-phase fraction
    /// `τ_bl` keep the initial mass. From `M_FGB` up `R_mHe` and the radius at ignition keep the
    /// initial mass, as in the published SSE code (see [`gb::r_hei_at`] for why). Equation 62's ξ
    /// is a ratio of radii, so it follows them, and with it the luminosity's rise, as in SSE
    /// (`hrdiag`'s `texp`).
    #[must_use]
    fn radii_at(&self, mt: SolarMasses, c: &ZCoeffs) -> Radii {
        let giant = RadiusLaw::giant(mt, c);
        let asymptotic = RadiusLaw::asymptotic(mt, c);
        let blue = match self.regime {
            Regime::Low => BluePhase::HorizontalBranch {
                zahb: self.zahb.as_ref().map_or_else(
                    || ZeroAgeHorizontalBranch::new(mt, c),
                    |zahb| zahb.at_mass(mt, c),
                ),
                r_mhe: r_mhe_low_with(mt, &self.powers.blue_start, self.at_hef, c),
                r_y: asymptotic.at_powers(&self.powers.agb_base),
            },
            Regime::Intermediate => {
                let r_x = giant.at_powers(&self.powers.blue_start);
                BluePhase::Fixed(BlueShape::new(
                    r_x,
                    gb::r_mhe_intermediate(mt, c),
                    r_x,
                    asymptotic.at_powers(&self.powers.agb_base),
                ))
            }
            Regime::High => {
                // `R_mHe` at the initial mass, as [`gb::r_hei_at`] explains.
                let high = self
                    .high_blue
                    .unwrap_or_else(|| HighBlue::new(self.mass, c));
                let r_mhe = high.r_mhe;
                let (printed, xi) = high.printed_and_xi(&giant, &self.powers.ignition);
                let l_y = SolarLuminosities::new(
                    self.l_hei.value()
                        * math::powf_positive(
                            self.l_bagb / self.l_hei,
                            math::powf(self.tau_bl, xi),
                        ),
                );
                let r_x = self
                    .ignition
                    .as_ref()
                    .map_or_else(|| gb::r_hei_at(self.mass, mt, c), |i| i.at(mt, c));
                BluePhase::Fixed(BlueShape::new(r_x, r_mhe, printed, asymptotic.at(l_y)))
            }
        };
        Radii {
            blue,
            giant,
            asymptotic,
        }
    }

    /// When core helium burning starts, `t_HeI`.
    #[must_use]
    pub(crate) const fn t_start(&self) -> Megayears {
        self.t_hei
    }

    /// When it ends at the base of the asymptotic giant branch, `t_HeI` + `t_He`.
    #[must_use]
    pub(crate) fn t_end(&self) -> Megayears {
        self.t_hei + self.t_he
    }

    /// Luminosity, radius and core mass at `t`, `t_HeI` ≤ t ≤ `t_HeI` + `t_He` (HPT equations
    /// 61–65 and 67).
    ///
    /// The core grows linearly in τ from `Mc,HeI` to `Mc,BAGB` (equations 66 and 67).
    ///
    /// # Panics
    ///
    /// In debug builds, if `t` lies outside the phase by more than rounding.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn at(&self, t: Megayears) -> PhasePoint {
        self.point(t, &self.radii)
    }

    /// `CoreHeliumBurning::at` for a star whose current mass `mt` has fallen below the mass the
    /// phase was built for, with every radius formula at `mt` (see
    /// [`CoreHeliumBurning::radii_at`]). Equal, bit for bit, to `CoreHeliumBurning::at` when
    /// `mt` is the phase's own mass.
    ///
    /// # Panics
    ///
    /// In debug builds, as `CoreHeliumBurning::at`.
    #[must_use]
    pub(crate) fn at_mass(&self, t: Megayears, mt: SolarMasses, c: &ZCoeffs) -> PhasePoint {
        // Only the radius formulae the age needs are evaluated: before `τ_x` the giant's alone,
        // and from `M_FGB` up after the blue phase the asymptotic giant's with ξ. The result is
        // `point` with every formula of `radii_at`, bit for bit; the blue phase itself takes the
        // whole set. (Plan 06's integrator speed: `radii_at` was most of core helium burning's
        // `pow` calls, which a track evaluates at every step of the phase.)
        let tau = self.tau_at(t);
        if tau < self.tau_x {
            let luminosity = self.luminosity_at(tau, || {
                unreachable!("before τ_x the luminosity does not read ξ")
            });
            return PhasePoint {
                luminosity,
                radius: RadiusLaw::giant(mt, c).at(luminosity),
                core_mass: self.core_mass_at_tau(tau),
            };
        }
        if let Some(high) = &self.high_blue
            && (tau > self.tau_y || self.tau_y <= self.tau_x)
        {
            let (_, xi) = high.printed_and_xi(&RadiusLaw::giant(mt, c), &self.powers.ignition);
            let luminosity = self.luminosity_at(tau, || xi);
            return PhasePoint {
                luminosity,
                radius: RadiusLaw::asymptotic(mt, c).at(luminosity),
                core_mass: self.core_mass_at_tau(tau),
            };
        }
        self.point(t, &self.radii_at(mt, c))
    }

    /// The relative age τ at `t`, held to 0–1.
    #[must_use]
    fn tau_at(&self, t: Megayears) -> f64 {
        let tau = (t - self.t_hei) / self.t_he;
        debug_assert!(
            (-1e-9..=1.0 + 1e-9).contains(&tau),
            "core helium burning runs from t_HeI to t_BAGB, not τ = {tau}"
        );
        // Rounding may put τ a hair outside 0–1, where the descent's λ′ divides by a zero `τ_x`.
        tau.clamp(0.0, 1.0)
    }

    /// The core at relative age `tau` (equation 67).
    #[must_use]
    fn core_mass_at_tau(&self, tau: f64) -> SolarMasses {
        self.mc_hei * (1.0 - tau) + self.mc_bagb * tau
    }

    /// The luminosity at relative age `tau` (equations 61 and 63), with ξ from `xi` from `τ_x` on.
    #[must_use]
    fn luminosity_at(&self, tau: f64, xi: impl FnOnce() -> f64) -> SolarLuminosities {
        let tau_x = self.tau_x;
        let l_x = self.l_x.value();
        SolarLuminosities::new(if tau < tau_x {
            let lambda = math::powi((tau_x - tau) / tau_x, 3);
            l_x * math::powf_positive(self.l_hei.value() / l_x, lambda)
        } else {
            let lambda = math::powf(((tau - tau_x) / (1.0 - tau_x)).max(0.0), xi());
            l_x * math::powf_positive(self.l_bagb.value() / l_x, lambda)
        })
    }

    /// The core mass at `t` (`CoreHeliumBurning::at`'s), M☉.
    #[must_use]
    pub(crate) fn core_mass(&self, t: Megayears) -> SolarMasses {
        let tau = ((t - self.t_hei) / self.t_he).clamp(0.0, 1.0);
        self.mc_hei * (1.0 - tau) + self.mc_bagb * tau
    }

    /// L, R and core at `t` with the radius formulae `radii`.
    #[must_use]
    fn point(&self, t: Megayears, radii: &Radii) -> PhasePoint {
        let tau = self.tau_at(t);
        let (tau_x, tau_y) = (self.tau_x, self.tau_y);
        let core_mass = self.core_mass_at_tau(tau);
        let shape = match &radii.blue {
            BluePhase::Fixed(shape) => *shape,
            BluePhase::HorizontalBranch { zahb, r_mhe, r_y } => {
                let r_x = zahb.radius(core_mass);
                BlueShape::new(r_x, *r_mhe, r_x, *r_y)
            }
        };
        let luminosity = self.luminosity_at(tau, || shape.xi);
        let radius = if tau < tau_x {
            radii.giant.at(luminosity)
        } else if tau > tau_y || tau_y <= tau_x {
            radii.asymptotic.at(luminosity)
        } else {
            let s = (tau - tau_x) / (tau_y - tau_x);
            let rho = shape.rho_x * (s - 1.0) + shape.rho_y * s;
            shape.r_min * math::exp(math::powi(rho.abs(), 3))
        };
        PhasePoint {
            luminosity,
            radius,
            core_mass,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::continuity::{assert_continuous_over, assert_no_jump};
    use super::super::hg::HertzsprungGap;
    use super::*;
    use crate::units::MetalFraction;
    use crate::units::consts::SOLAR_EFFECTIVE_TEMPERATURE_K;

    const REFERENCE_Z: [f64; 5] = [1e-4, 1e-3, 4e-3, 0.02, 0.03];

    fn coeffs(z: f64) -> ZCoeffs {
        ZCoeffs::new(MetalFraction::new(z))
    }

    /// Core helium burning evaluated with only the radius formulae its age needs is the phase with
    /// every formula at the current mass, bit for bit, in each regime, at masses below the phase's
    /// and at `τ_x`, `τ_y` and their neighbours.
    #[test]
    fn core_helium_burning_at_a_current_mass_is_the_full_evaluation_bit_for_bit() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for m in [0.7, 1.2, 1.9, 2.3, 4.0, 7.0, 11.0, 14.0, 20.0, 40.0, 90.0] {
                let phase = CoreHeliumBurning::new(SolarMasses::new(m), &c);
                let (t0, t1) = (phase.t_start().value(), phase.t_end().value());
                let mut fractions: Vec<f64> = (0..=40).map(|i| f64::from(i) / 40.0).collect();
                for edge in [phase.tau_x, phase.tau_y] {
                    fractions.extend([edge, edge.next_up(), edge.next_down(), edge + 1e-6]);
                }
                for share in [1.0, 0.97, 0.8, 0.5] {
                    let mt = SolarMasses::new(m * share);
                    let radii = phase.radii_at(mt, &c);
                    for &x in &fractions {
                        let t =
                            Megayears::new((1.0 - x.clamp(0.0, 1.0)) * t0 + x.clamp(0.0, 1.0) * t1);
                        assert_eq!(
                            phase.at_mass(t, mt, &c),
                            phase.point(t, &radii),
                            "Z = {z}, M = {m}, Mt = {mt:?}, x = {x}"
                        );
                    }
                }
            }
        }
    }

    fn mass(m: f64) -> SolarMasses {
        SolarMasses::new(m)
    }

    /// Masses from 0.5 to 100 M☉, evenly in log mass.
    fn masses(n: u32) -> Vec<f64> {
        (0..n)
            .map(|i| math::exp10(-0.3 + 2.3 * f64::from(i) / f64::from(n - 1)))
            .collect()
    }

    /// The phase at relative age `tau`.
    fn at_fraction(phase: &CoreHeliumBurning, tau: f64) -> PhasePoint {
        let (t0, t1) = (phase.t_start().value(), phase.t_end().value());
        phase.at(Megayears::new(t0 + (t1 - t0) * tau))
    }

    /// Effective temperature, K, from L and R by Stefan–Boltzmann.
    fn t_eff(p: &PhasePoint) -> f64 {
        SOLAR_EFFECTIVE_TEMPERATURE_K * math::powf(p.luminosity.value(), 0.25)
            / p.radius.value().sqrt()
    }

    #[track_caller]
    fn assert_close(what: &str, a: f64, b: f64, tolerance: f64) {
        assert!((a / b - 1.0).abs() < tolerance, "{what}: {a} against {b}");
    }

    /// From `M_HeF` up, core helium burning starts where the giant branch ends (to `M_FGB`) or the
    /// Hertzsprung gap ends (from `M_FGB`), in L, R and core mass. Below `M_HeF` the helium flash
    /// lies between them, which P06.T10.d bridges.
    #[test]
    fn core_helium_burning_starts_where_the_giant_branch_or_the_gap_ends() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for m in masses(80).into_iter().filter(|&m| m >= c.m_hef().value()) {
                let m = mass(m);
                let phase = CoreHeliumBurning::new(m, &c);
                let start = phase.at(phase.t_start());
                let end = if m.value() < c.m_fgb().value() {
                    let giant = FirstGiantBranch::new(m, &c);
                    assert_close(
                        "t_HeI",
                        phase.t_start().value(),
                        giant.t_hei().value(),
                        1e-15,
                    );
                    giant.at(giant.t_hei())
                } else {
                    let gap = HertzsprungGap::new(m, &c);
                    assert_close("t_HeI", phase.t_start().value(), gap.t_end().value(), 1e-15);
                    gap.at(gap.t_end())
                };
                let what = format!("{m:?}, Z = {z}");
                let pairs = [
                    ("L", start.luminosity.value(), end.luminosity.value()),
                    ("R", start.radius.value(), end.radius.value()),
                    ("Mc", start.core_mass.value(), end.core_mass.value()),
                ];
                for (name, a, b) in pairs {
                    assert_close(&format!("{name} at {what}"), a, b, 1e-9);
                }
                // A sweep in age across helium ignition finds no jump in L or R.
                let before = |t: f64| {
                    let t = Megayears::new(t);
                    if t < phase.t_start() {
                        if m.value() < c.m_fgb().value() {
                            FirstGiantBranch::new(m, &c).at(t)
                        } else {
                            HertzsprungGap::new(m, &c).at(t)
                        }
                    } else {
                        phase.at(t)
                    }
                };
                let t0 = phase.t_start().value();
                let previous_start = if m.value() < c.m_fgb().value() {
                    ms::t_bgb(m, &c)
                } else {
                    ms::t_ms(m, &c)
                };
                let dt = 0.1 * (t0 - previous_start.value()).min(phase.t_end().value() - t0);
                let log_l = |t: f64| math::log10(before(t).luminosity.value());
                let log_r = |t: f64| math::log10(before(t).radius.value());
                assert_no_jump(
                    &format!("L across t_HeI at {what}"),
                    log_l,
                    t0 - dt,
                    t0 + dt,
                    1e-6,
                );
                assert_no_jump(
                    &format!("R across t_HeI at {what}"),
                    log_r,
                    t0 - dt,
                    t0 + dt,
                    1e-6,
                );
            }
        }
    }

    /// A time a rounding error before `t_HeI` gives the state at `t_HeI`, not the infinity of the
    /// descent's λ′ at `τ_x` = 0 (below `M_HeF` and from `M_FGB` up).
    #[test]
    fn a_time_just_before_ignition_is_ignition() {
        for (z, m) in [(0.02, 1.0), (1e-4, 0.8), (0.02, 20.0)] {
            let phase = CoreHeliumBurning::new(mass(m), &coeffs(z));
            let t = phase.t_start() * (1.0 - 1e-13);
            let (early, start) = (phase.at(t), phase.at(phase.t_start()));
            let what = format!("M = {m}, Z = {z}");
            assert_close(
                &format!("L at {what}"),
                early.luminosity.value(),
                start.luminosity.value(),
                1e-12,
            );
            assert_close(
                &format!("R at {what}"),
                early.radius.value(),
                start.radius.value(),
                1e-12,
            );
        }
    }

    /// Core helium burning ends at the base of the asymptotic giant branch: `L_BAGB`,
    /// `R_AGB`(`L_BAGB`) and `Mc,BAGB`, where the early AGB of P06.T8 starts.
    #[test]
    fn core_helium_burning_ends_at_the_base_of_the_asymptotic_giant_branch() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for m in masses(80) {
                let m = mass(m);
                let phase = CoreHeliumBurning::new(m, &c);
                let end = phase.at(phase.t_end());
                let l = l_bagb(m, &c);
                let what = format!("{m:?}, Z = {z}");
                let pairs = [
                    ("L", end.luminosity.value(), l.value()),
                    ("R", end.radius.value(), gb::agb_radius(m, l, &c).value()),
                    ("Mc", end.core_mass.value(), gb::mc_bagb(m, &c).value()),
                ];
                for (name, a, b) in pairs {
                    assert_close(&format!("{name} at {what}"), a, b, 1e-9);
                }
            }
        }
    }

    /// The plan's 5 M☉ blue loop "crossing 5,500–6,500 K" holds at Z = 0.004, where the loop spans
    /// 4,475–7,260 K. At Z = 0.02 HPT's formulae keep the whole phase between 4,010 and 4,665 K, as
    /// the published SSE code does (4,012–4,663 K at 2,001 ages, `hrdiag` with no mass loss): the
    /// loop there never reaches the instability strip.
    #[test]
    fn a_five_solar_mass_blue_loop_crosses_the_instability_strip_at_low_metallicity() {
        let range = |z: f64| {
            let phase = CoreHeliumBurning::new(mass(5.0), &coeffs(z));
            (0..=2_000).fold((f64::INFINITY, 0.0_f64), |(lo, hi), i| {
                let t = t_eff(&at_fraction(&phase, f64::from(i) / 2_000.0));
                (lo.min(t), hi.max(t))
            })
        };
        let (lo, hi) = range(0.004);
        assert!(lo < 5_500.0 && hi > 6_500.0, "Z = 0.004: {lo}–{hi} K");
        let (lo, hi) = range(0.02);
        assert!(
            (4_000.0..4_020.0).contains(&lo) && (4_655.0..4_670.0).contains(&hi),
            "Z = 0.02: {lo}–{hi} K"
        );
    }

    /// The horizontal branch at Z = 0.0005 by envelope mass. HPT section 7.1 reset the initial mass
    /// to the current one at the flash, so a star that lost mass on the giant branch is evaluated
    /// as a new star of mass M = Mc + envelope. With 0.1–0.2 M☉ of envelope the formulae put the
    /// zero-age horizontal branch at 15,400–9,000 K, not the plan's 6,000–7,500 K: that band, the
    /// RR Lyrae region, takes 0.25–0.30 M☉ (7,720 K at M = 0.75 M☉, 6,910 K at 0.8). SSE agrees
    /// (at helium ignition, `hrdiag` with no mass loss: 11,162 K at 0.65 M☉, 8,990 K at 0.7, 7,720 K
    /// at 0.75 and 6,912 K at 0.8; lighter stars have μ < 1 there and are perturbed). Less envelope
    /// is bluer throughout.
    #[test]
    fn the_horizontal_branch_is_bluer_with_less_envelope() {
        let c = coeffs(0.0005);
        let zahb = |m: f64| {
            let phase = CoreHeliumBurning::new(mass(m), &c);
            let start = phase.at(phase.t_start());
            (m - start.core_mass.value(), t_eff(&start))
        };
        let mut last = f64::INFINITY;
        for i in 0..=60 {
            let (envelope, t) = zahb(0.55 + 0.25 * f64::from(i) / 60.0);
            assert!(t < last, "not bluer with less envelope at {envelope}");
            last = t;
            if (0.1..=0.2).contains(&envelope) {
                assert!((8_900.0..15_500.0).contains(&t), "{envelope}: {t} K");
            }
            if (0.25..=0.3).contains(&envelope) {
                assert!((6_000.0..7_800.0).contains(&t), "{envelope}: {t} K");
            }
        }
        let (envelope, t) = zahb(0.8);
        assert!(
            (envelope - 0.299).abs() < 0.001,
            "envelope {envelope} at 0.8 M☉"
        );
        assert!((t - 6_912.0).abs() < 5.0, "{t} K at 0.8 M☉");
    }

    /// `t_He`(1 M☉) at Z = 0.02 is 131.5 Myr, as SSE gives it; 107–135 Myr over the five
    /// metallicities.
    #[test]
    fn a_solar_mass_star_burns_helium_for_about_130_million_years() {
        let t = t_he(mass(1.0), &coeffs(0.02)).value();
        assert!((t - 131.514_427_062_987).abs() < 1e-9, "{t}");
        for z in REFERENCE_Z {
            let t = t_he(mass(1.0), &coeffs(z)).value();
            assert!((100.0..140.0).contains(&t), "t_He = {t} Myr at Z = {z}");
        }
    }

    /// As the envelope vanishes the horizontal branch meets the naked helium star of its core
    /// (HPT section 5.3): `L_ZAHB` → `L_ZHe`(Mc), `R_ZAHB` → `R_ZHe`(Mc), `t_He` → `t_HeMS`(Mc). At
    /// `M_HeF` (µ = 1) `L_ZAHB` is `L_min,He`(`M_HeF`) whatever the core.
    #[test]
    fn the_horizontal_branch_meets_the_helium_star_as_the_envelope_vanishes() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for mc in [0.35, 0.45, 0.5] {
                let core = mass(mc);
                let what = format!("Mc = {mc}, Z = {z}");
                let l = l_zahb(core, core, &c).value();
                assert_close(
                    &format!("L at {what}"),
                    l,
                    helium::zams_luminosity(core).value(),
                    1e-12,
                );
                let r = r_zahb(core, core, &c).value();
                assert_close(
                    &format!("R at {what}"),
                    r,
                    helium::zams_radius(core).value(),
                    1e-12,
                );
                let t = t_he_low(core, core, &c).value();
                let t_hems = helium::main_sequence_lifetime(core).value();
                assert_close(&format!("t at {what}"), t, t_hems, 1e-8);
                let at_hef = l_zahb(c.m_hef(), core, &c).value();
                assert_close(
                    &format!("L(M_HeF) at {what}"),
                    at_hef,
                    l_min_he(c.m_hef(), &c).value(),
                    1e-12,
                );
            }
        }
    }

    /// Landmarks against the published SSE code (see [`sse`](super::super) for the run): `t_He` and
    /// `L_BAGB` from `star` to 10⁻¹²; `τ_bl` (`tblf`) and `L_min,He` ÷ `L_HeI` (`lhef`, the second
    /// catching the misprinted b17 exponent, which moved it by up to a third) to 10⁻⁴, since they
    /// depend on `M_FGB`, whose printed constants differ from the code's by up to 0.18% (P06.T4.b).
    #[test]
    fn landmarks_match_the_published_sse_code() {
        for &(z, m, t_sse, l_sse) in SSE_T_HE_L_BAGB {
            let (c, m) = (coeffs(z), mass(m));
            assert_close(
                &format!("t_He at Z = {z}, {m:?}"),
                t_he(m, &c).value(),
                t_sse,
                1e-12,
            );
            assert_close(
                &format!("L_BAGB at Z = {z}, {m:?}"),
                l_bagb(m, &c).value(),
                l_sse,
                1e-12,
            );
        }
        for &(z, m, ratio_sse, tau_sse) in SSE_LHEF_TBLF {
            let (c, m) = (coeffs(z), mass(m));
            let ratio = l_min_he(m, &c) / gb::l_hei(m, &c);
            assert_close(
                &format!("L_min,He at Z = {z}, {m:?}"),
                ratio,
                ratio_sse,
                1e-4,
            );
            let tau = blue_fraction(m, &c);
            assert!(
                (tau - tau_sse).abs() < 1e-4,
                "τ_bl at Z = {z}, {m:?}: {tau} against {tau_sse}"
            );
        }
    }

    /// (Z, M, `t_He` Myr, `L_BAGB`) from SSE's `star`.
    const SSE_T_HE_L_BAGB: &[(f64, f64, f64, f64)] = &[
        (0.02, 1.0, 131.514_427_062_987, 162.606_179_639_010_66),
        (0.004, 5.0, 12.596_497_474_853_697, 3_342.982_983_132_481),
        (0.0001, 0.8, 120.100_819_169_135_35, 141.464_401_839_067_02),
        (0.03, 40.0, 0.502_337_010_229_288_8, 666_012.354_968_608_7),
    ];

    /// (Z, M, `lhef`, `tblf`) from SSE's functions.
    const SSE_LHEF_TBLF: &[(f64, f64, f64, f64)] = &[
        (0.004, 5.0, 0.676_755_950_997_596_8, 0.515_880_621_044_041_4),
        (0.03, 8.0, 0.475_501_205_808_725_46, 0.343_103_366_551_174_9),
        (0.02, 4.0, 0.280_600_981_000_361_57, 0.700_439_647_573_529_7),
        (0.001, 5.0, 0.921_172_836_390_689_3, 0.729_309_881_706_540_3),
    ];

    /// L, R and core mass against the published SSE code's `hrdiag` with no mass loss (see
    /// [`sse`](super::super) for the run), at points in each regime where its small-envelope
    /// perturbation (HPT section 6.3, μ < 1) does not act, or, for the massive rows, from the same
    /// run with that perturbation switched off, since SSE applies it to every star above about 12
    /// M☉ in this phase. L and R agree to 10⁻⁵: `M_FGB`'s printed constants (P06.T4.b) and eq. 53's
    /// printed 1.6479 move them by up to 4 × 10⁻⁶ here (with the code's two constants every
    /// unperturbed row of the 5 Z × 31 mass run agrees to 10⁻¹³). `Mc` agrees to 10⁻⁷ (see
    /// [`gb`]'s test).
    #[test]
    fn matches_the_published_sse_code() {
        for &(z, m, t, l_sse, r_sse, mc_sse) in SSE_CHEB {
            let c = coeffs(z);
            let point = CoreHeliumBurning::new(mass(m), &c).at(Megayears::new(t));
            let what = format!("Z = {z}, M = {m}, t = {t}");
            assert_close(
                &format!("L at {what}"),
                point.luminosity.value(),
                l_sse,
                1e-5,
            );
            assert_close(&format!("R at {what}"), point.radius.value(), r_sse, 1e-5);
            assert_close(
                &format!("Mc at {what}"),
                point.core_mass.value(),
                mc_sse,
                1e-7,
            );
        }
    }

    /// (Z, M, t Myr, L, R, Mc) from SSE's `hrdiag` in core helium burning: low mass on the
    /// horizontal branch at three metallicities; 5 M☉ descending the giant branch and on its blue
    /// loop, and 2.5 M☉ at Z = 0.004; 25 M☉ at Z = 0.001 blue and red, and 40 M☉ at Z = 0.03, which
    /// has no blue phase (the last three with the perturbation switched off).
    const SSE_CHEB: &[(f64, f64, f64, f64, f64, f64)] = &[
        (
            0.02,
            1.0,
            12_367.984_295_050_126,
            65.147_587_716_925_09,
            12.408_305_547_567_206,
            0.488_297_323_189_284_87,
        ),
        (
            0.0001,
            0.8,
            14_061.432_968_017_207,
            73.929_075_134_195_73,
            5.022_271_789_898_946,
            0.524_127_048_377_043_2,
        ),
        (
            0.001,
            1.5,
            1_879.058_716_223_223_2,
            130.721_324_734_541_78,
            14.041_868_681_261_931,
            0.524_196_876_978_864_3,
        ),
        (
            0.02,
            5.0,
            105.530_385_689_381_09,
            2_107.481_029_057_634_3,
            87.584_673_249_385_5,
            0.881_311_883_288_908_7,
        ),
        (
            0.02,
            5.0,
            113.573_860_208_388_8,
            1_401.957_210_464_944_4,
            58.130_702_657_020_21,
            1.051_060_738_900_658_8,
        ),
        (
            0.004,
            2.5,
            537.088_309_673_461_1,
            104.072_541_746_861_73,
            13.092_394_194_680_976,
            0.458_991_366_498_649_36,
        ),
        (
            0.001,
            25.0,
            8.205_643_919_371_344,
            261_370.002_310_667_57,
            84.266_015_813_763_59,
            8.623_725_948_114_696,
        ),
        (
            0.001,
            25.0,
            8.545_656_788_958_555,
            283_704.272_172_771_1,
            961.071_254_207_367_9,
            9.641_893_606_327_505,
        ),
        (
            0.03,
            40.0,
            4.761_585_128_525_859,
            586_664.488_342_348_3,
            2_008.502_950_215_066_2,
            16.217_430_229_928_436,
        ),
    ];

    /// L and R are continuous in τ through the phase for every mass and metallicity, across `τ_x`
    /// and `τ_y` too.
    #[test]
    fn luminosity_and_radius_are_continuous_through_the_phase() {
        let taus: Vec<f64> = (0..=1_000).map(|i| f64::from(i) / 1_000.0).collect();
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for m in masses(60) {
                let phase = CoreHeliumBurning::new(mass(m), &c);
                let log_l = |tau: f64| math::log10(at_fraction(&phase, tau).luminosity.value());
                let log_r = |tau: f64| math::log10(at_fraction(&phase, tau).radius.value());
                assert_continuous_over(&format!("L at M = {m}, Z = {z}"), log_l, &taus, 2e-3, 1e-6);
                assert_continuous_over(&format!("R at M = {m}, Z = {z}"), log_r, &taus, 2e-3, 1e-6);
            }
        }
    }

    /// The landmarks, and L and R at fixed τ, are continuous in mass across `M_HeF` and 12 M☉, and
    /// across `M_FGB` for Z ≤ 0.002. For Z > 0.002 HPT declare the formulae discontinuous at
    /// `M_FGB` (section 5.3: `L_min,He` = b17 `L_HeI` and `τ_bl` = b45 below it, `L_HeI` and 1 − b47
    /// above), and the sweep stops either side of it; the test checks that the jump is there.
    #[test]
    fn the_phase_is_continuous_in_mass_except_at_m_fgb_above_z_0_002() {
        type Quantity = fn(SolarMasses, &ZCoeffs) -> f64;
        fn log_l(m: SolarMasses, c: &ZCoeffs, tau: f64) -> f64 {
            math::log10(
                at_fraction(&CoreHeliumBurning::new(m, c), tau)
                    .luminosity
                    .value(),
            )
        }
        fn log_r(m: SolarMasses, c: &ZCoeffs, tau: f64) -> f64 {
            math::log10(
                at_fraction(&CoreHeliumBurning::new(m, c), tau)
                    .radius
                    .value(),
            )
        }
        let quantities: [(&str, Quantity); 9] = [
            ("t_He", |m, c| math::log10(t_he(m, c).value())),
            ("L_BAGB", |m, c| math::log10(l_bagb(m, c).value())),
            ("tau_bl", blue_fraction),
            ("L(0.2)", |m, c| log_l(m, c, 0.2)),
            ("R(0.2)", |m, c| log_r(m, c, 0.2)),
            ("L(0.5)", |m, c| log_l(m, c, 0.5)),
            ("R(0.5)", |m, c| log_r(m, c, 0.5)),
            ("L(0.8)", |m, c| log_l(m, c, 0.8)),
            ("R(0.8)", |m, c| log_r(m, c, 0.8)),
        ];
        let log_masses: Vec<f64> = masses(600).into_iter().map(math::log10).collect();
        for z in REFERENCE_Z {
            let c = coeffs(z);
            let m_fgb = c.m_fgb().value();
            let log_fgb = math::log10(m_fgb);
            let split = if z > 0.002 { log_fgb } else { f64::INFINITY };
            let below: Vec<f64> = log_masses.iter().copied().filter(|&x| x < split).collect();
            let above: Vec<f64> = log_masses.iter().copied().filter(|&x| x >= split).collect();
            for (name, quantity) in quantities {
                let f = |log_m: f64| quantity(mass(math::exp10(log_m)), &c);
                for sweep in [&below, &above] {
                    if sweep.len() > 1 {
                        assert_continuous_over(
                            &format!("{name} at Z = {z}"),
                            f,
                            sweep,
                            0.015,
                            1e-6,
                        );
                    }
                }
            }
            let jump =
                blue_fraction(mass(m_fgb), &c) - blue_fraction(mass(m_fgb * (1.0 - 1e-12)), &c);
            if z > 0.002 {
                assert!(
                    jump.abs() > 1e-3,
                    "τ_bl jumps by only {jump} at M_FGB, Z = {z}"
                );
            } else {
                assert!(jump.abs() < 1e-9, "τ_bl jumps by {jump} at M_FGB, Z = {z}");
            }
        }
    }
}
