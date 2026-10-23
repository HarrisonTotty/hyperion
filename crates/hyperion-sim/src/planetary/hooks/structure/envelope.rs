//! A gas envelope's vertical structure (plan 14, P14.T24.d; science-r08-gases-2.md §1.3–§1.7):
//! Guillot 2010's irradiated grey profile above, joined to the dry adiabat below the level where
//! its gradient first exceeds R ÷ `c_p`, read through the one
//! [`temperature_at`](crate::planetary::hooks::structure::temperature_at) as
//! [`VerticalStructure::Envelope`](crate::planetary::hooks::structure::VerticalStructure::Envelope).
//!
//! # The radiative profile ([`GreyProfile`])
//!
//! Guillot (2010, A&A 520, A27, Eq. 29) gives the temperature of a semi-grey atmosphere heated
//! from below by an internal flux σ`T_int`⁴ and from above by starlight absorbed at a visible
//! opacity γ times the thermal one, isotropically averaged:
//!
//! ```text
//! T⁴ = ¾ T_int⁴ (⅔ + τ) + ¾ f T_irr⁴ [⅔ + 1 ÷ (γ√3) + (γ ÷ √3 − 1 ÷ (γ√3)) e^(−γ τ √3)]
//! ```
//!
//! (his Eq. 27 at μ* = 1 ÷ √3, with μ* σ`T_irr`⁴ replaced by f σ`T_irr`⁴; §2.4), with τ the thermal
//! optical depth, dτ = `κ_th` dm = `κ_th` dp ÷ g. Guillot takes his opacities constant (§6); at
//! depth collision-induced absorption makes them "to first order proportional to pressure"
//! (§2.5). His moment equations are written in the column mass m, so at a fixed γ the solution
//! holds in τ for any `κ_th`(p) (decision-p14-t24d-profile §1.2). Here `κ_th` = κ₁ (p ÷
//! p₁)^(n − 1), so that τ = κ₁ p₁ (p ÷ p₁)ⁿ ÷ (n g), with [`REFERENCE_PRESSURE`] p₁ 1 bar and n
//! [`GreyProfile::optical_depth_exponent`] (1 for a constant opacity, Guillot's own case).
//!
//! The envelope's constants ([`GreyProfile::ADOPTED`]; decision-p14-t24d-profile §1.2–§1.3):
//! - **n = 2** ([`OPTICAL_DEPTH_EXPONENT`]): an opacity proportional to pressure, as H₂'s
//!   collision-induced absorption is (Robinson and Catling 2012, ApJ 757, 104, Eq. 6: "a
//!   well-mixed gas providing collision-induced opacity (e.g., H2 in gas giant atmospheres in the
//!   Solar System) … so that n = 2"; their 2014 Table 1 takes it for all four giants; Mollière et
//!   al. 2020, A&A 640, A131, §2.3, take τ = δPᵅ with α between 1 and 2). A constant opacity
//!   never joins (below).
//! - **γ = 0.4** ([`OPACITY_RATIO`]): Guillot's fiducial for HD 209458 b (§2.5, after his Eq. 30;
//!   `κ_v` 4 × 10⁻³ against `κ_th` 10⁻² cm² g⁻¹). Below 1 it draws no inversion, and the gradient's
//!   one local peak before its rise stays under 0.19, below every H₂–He R ÷ `c_p`, so the first
//!   crossing is unique and the join moves continuously with its inputs.
//! - **f = ¼** ([`MEAN_REDISTRIBUTION`]), the whole planet's mean.
//! - **κ₁** ([`THERMAL_OPACITY_AT_ONE_BAR`]), fitted through this module on the Solar System
//!   table's Jupiter to its measured 166.1 K at 1 bar (Seiff et al. 1998), the own-model rule.
//!
//! `T_irr` is Guillot's T* (R* ÷ D)^½ (his Eq. 1, with `T_eq`⁴ = `T_irr`⁴ ÷ 4 for no albedo, Eq.
//! 2), here of the starlight absorbed: `T_irr`⁴ = 4 `T_eq`⁴ ([`irradiation_temperature`]), `T_eq`
//! the record's equilibrium temperature over the whole sphere with its Bond albedo folded in
//! (ruling 112.7's light-only split). Eq. 29 holds no albedo (Guillot neglects scattering, §2.2),
//! so this scaling of `T_irr`⁴ by 1 − `A_B` is the module's own extension. f is the redistribution
//! over the planet: 1 at the substellar point, ½ for a dayside average and ¼ over the whole planet
//! (§2.4, after Burrows et al. 2003); [`MEAN_REDISTRIBUTION`]'s ¼ gives f `T_irr`⁴ = `T_eq`⁴, so
//! that the profile emits σ`T_eff`⁴ = σ(`T_eq`⁴ + `T_int`⁴).
//!
//! At τ = 0 the profile's skin is `T_skin`⁴ = `T_int`⁴ ÷ 2 + f `T_irr`⁴ (½ + √3 γ ÷ 4), the surface
//! air's `T_eff`⁴ ÷ 2 (P14.T24.e's [`skin_temperature`](super::skin_temperature)) in the limit
//! γ → 0, warmer for any positive γ.
//!
//! # The join
//!
//! The radiative gradient ∇ = d ln T ÷ d ln p = n τ (dT⁴ ÷ dτ) ÷ (4 T⁴), with dT⁴ ÷ dτ = ¾ [`T_int`⁴ +
//! f `T_irr`⁴ (1 − γ²) e^(−γ τ √3)], is compared with the dry adiabat's ∇_ad = R ÷ `c_p`(T), P14.T49.b's
//! statistical sum over the envelope's mixture ([`mixture_heat_capacity_over_r_at`]; hydrogen
//! frozen at 3:1 ortho to para, normal hydrogen, science-r08-gases-2.md §1.3–§1.4). Going down from
//! τ = [`JOIN_SCAN_TOP_OPTICAL_DEPTH`] in steps of [`JOIN_SCAN_STEP_LN_P`] in ln p, the first level
//! at which ∇ exceeds ∇_ad is bracketed and refined by [`JOIN_BISECTIONS`] bisections in ln p: the
//! join, `p_join`, where the profile becomes convective (Mollière et al. 2020 force the profile
//! onto the adiabat "as soon as the atmosphere is found to be Schwarzschild-unstable"). Its
//! temperature is the grey profile's there.
//!
//! The gradient is below n ÷ 4 at every depth, for every γ, `T_int` and `T_irr`, and tends to it
//! at depth where `T_int` > 0 (decision-p14-t24d-profile §1.1). A constant opacity therefore keeps
//! it under ¼, below every H₂–He R ÷ `c_p` (0.28–0.40 below 1,000 K), and never joins
//! (Robinson and Catling 2012's 4β ÷ n < 1). At n = 2 it tends to ½, above every ideal gas's
//! R ÷ `c_p` (at most ⅖), so wherever `T_int` is positive the join forms, and for γ ≤ 1 above
//! `τ_max` = 4 (⅔ + `c₀` (`T_eq` ÷ `T_int`)⁴), `c₀` = ⅔ + 1 ÷ (γ√3), with f `T_irr`⁴ = `T_eq`⁴
//! ([`GreyProfile::join_bracket_optical_depth`]); the scan ends one step past it, where a gas
//! whose `c_p` ÷ R is exactly 5/2 meets it. An envelope with no join above it has
//! none ([`DeriveEnvelopeProfileError::NoJoin`]): a constant opacity's, or one with no internal
//! heat.
//!
//! # The adiabat
//!
//! Below the join the profile is the dry adiabat, integrated as P14.T24.e's is through the
//! mixture's closed-form entropy S̃ = ∫ `c_p` ÷ (R T) dT ([`mixture_entropy_over_r`]):
//!
//! ```text
//! S̃(T) = S̃(T_join) + ln(p ÷ p_join)
//! ```
//!
//! published as (p, T) nodes at P14.T24.e's spacing, the pressures ascending evenly in ln p from
//! `p_join` to the larger of [`ADIABAT_BOTTOM`], 100 bar, and e^0.3 `p_join`, so that a join deeper
//! than 100 bar (a weakly heated envelope under strong light; decision-p14-t24d-profile §1.4)
//! still carries one segment: max(16,
//! ⌈ln(`p_bottom` ÷ `p_join`) ÷ 0.3⌉) segments, each node to 10⁻¹² of the exact adiabat. Between
//! nodes [`EnvelopeProfile::temperature_at`] is the power law through them, and deeper the last
//! segment's. Near 100 bar hydrogen's density departs from the ideal gas's by about 3% (its second
//! virial coefficient, from memory; science-r08-gases-2.md §1.7.3), the recorded limit for a later
//! real-gas form.
//!
//! # Not on the record yet
//!
//! This module is pure: nothing on the record calls it at generator version 23
//! (decision-bump-plan-after-22 §1.3). P14.T54.b puts it in the record's envelope section and on
//! the wire (`BodyEnvelopeDto`'s `profile`, `join_pressure_pa` and `adiabat`).
//!
//! # Determinism
//!
//! Every scan and bisection step count is fixed, each node a fixed count of Newton steps from the
//! node above it, every transcendental [`math`]'s and every sum in the mixture's given order.

use std::error::Error;
use std::fmt;

use super::{
    MIXTURE_TOLERANCE, NODE_SPACING_LN_P, ProfileNode, adiabat_temperature, entropy, heat_capacity,
    is_positive, segment_count,
};
use crate::math;
use crate::substance::{SubstanceId, mixture_entropy_over_r, mixture_heat_capacity_over_r_at};
use crate::units::{Kelvin, MetresPerSecondSquared, Pascals};

#[cfg(test)]
mod tests;

/// √3, the isotropic two-stream's cosine, inverted.
const SQRT_3: f64 = 1.732_050_807_568_877_2;

/// The reference pressure of the thermal opacity, `p_ref`: 1 bar.
pub const REFERENCE_PRESSURE: Pascals = Pascals::new(1.0e5);

/// The redistribution factor f of a profile averaged over the whole planet: ¼ (Guillot 2010, §2.4),
/// at which f `T_irr`⁴ = `T_eq`⁴.
pub const MEAN_REDISTRIBUTION: f64 = 0.25;

/// The envelope's exponent n of the thermal optical depth in pressure, τ ∝ pⁿ: 2, an opacity
/// proportional to pressure, H₂'s collision-induced absorption (Robinson and Catling 2012, Eq. 6;
/// decision-p14-t24d-profile §1.2).
pub const OPTICAL_DEPTH_EXPONENT: f64 = 2.0;

/// The envelope's γ, its visible opacity over its thermal one: 0.4, Guillot 2010's fiducial for HD
/// 209458 b ("With the fiducial opacity coefficients for `HD209458b` γ = 0.4", §2.5;
/// decision-p14-t24d-profile §1.3).
pub const OPACITY_RATIO: f64 = 0.4;

/// The envelope's thermal opacity at 1 bar, κ₁, m² kg⁻¹: FITTED, fitted through this module on the
/// Solar System table's Jupiter.
pub const THERMAL_OPACITY_AT_ONE_BAR: f64 = 2.87e-3;

/// The deepest level of the published adiabat, `p_bottom`: 100 bar (10⁷ Pa; science-r08-gases-2.md
/// §1.7.3). Deeper, [`EnvelopeProfile::temperature_at`] takes the last segment's exponent.
pub const ADIABAT_BOTTOM: Pascals = Pascals::new(1.0e7);

/// The optical depth from which the join is sought downward, 10⁻⁶: far above any level whose
/// gradient can reach an adiabat's.
pub const JOIN_SCAN_TOP_OPTICAL_DEPTH: f64 = 1e-6;

/// The scan's step in ln p while the join is sought: 0.05.
pub const JOIN_SCAN_STEP_LN_P: f64 = 0.05;

/// The bisections in ln p that refine the join inside its bracket: 50, to 0.05 × 2⁻⁵⁰ in ln p.
pub const JOIN_BISECTIONS: u32 = 50;

/// The irradiation temperature `T_irr` of a body of equilibrium temperature `equilibrium` (over the
/// whole sphere, its Bond albedo folded in): `T_irr`⁴ = 4 `T_eq`⁴, the starlight it absorbs per
/// unit area facing its star, so that [`MEAN_REDISTRIBUTION`]'s ¼ gives back `T_eq`.
#[must_use]
pub fn irradiation_temperature(equilibrium: Kelvin) -> Kelvin {
    Kelvin::new(equilibrium.value() * core::f64::consts::SQRT_2)
}

/// Guillot 2010's grey profile's parameters (the [module](self)'s documentation): the thermal
/// opacity at [`REFERENCE_PRESSURE`], its exponent in pressure, the visible-to-thermal opacity
/// ratio γ and the redistribution factor f.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct GreyProfile {
    thermal_opacity: f64,
    optical_depth_exponent: f64,
    opacity_ratio: f64,
    redistribution: f64,
}

/// [`GreyProfile::new`] refused a parameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildGreyProfileError {
    /// The thermal opacity is not finite and positive, m² kg⁻¹.
    ThermalOpacity(f64),
    /// The opacity's exponent in pressure is not finite and positive.
    OpticalDepthExponent(f64),
    /// γ is not finite and positive.
    OpacityRatio(f64),
    /// f is not in (0, 1].
    Redistribution(f64),
}

impl fmt::Display for BuildGreyProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ThermalOpacity(value) => {
                write!(
                    f,
                    "a thermal opacity is finite and positive, got {value} m2/kg"
                )
            }
            Self::OpticalDepthExponent(value) => write!(
                f,
                "an opacity's exponent in pressure is finite and positive, got {value}"
            ),
            Self::OpacityRatio(value) => {
                write!(f, "an opacity ratio is finite and positive, got {value}")
            }
            Self::Redistribution(value) => {
                write!(f, "a redistribution factor lies in (0, 1], got {value}")
            }
        }
    }
}

impl Error for BuildGreyProfileError {}

impl GreyProfile {
    /// Every envelope's parameters (the [module](self)'s documentation): κ₁
    /// [`THERMAL_OPACITY_AT_ONE_BAR`], n [`OPTICAL_DEPTH_EXPONENT`], γ [`OPACITY_RATIO`] and f
    /// [`MEAN_REDISTRIBUTION`].
    pub const ADOPTED: Self = Self {
        thermal_opacity: THERMAL_OPACITY_AT_ONE_BAR,
        optical_depth_exponent: OPTICAL_DEPTH_EXPONENT,
        opacity_ratio: OPACITY_RATIO,
        redistribution: MEAN_REDISTRIBUTION,
    };

    /// The parameters: `thermal_opacity` `κ_ref`, m² kg⁻¹, at [`REFERENCE_PRESSURE`];
    /// `optical_depth_exponent` n, τ ∝ pⁿ (1 for a constant opacity); `opacity_ratio` γ; and
    /// `redistribution` f.
    ///
    /// # Errors
    ///
    /// [`BuildGreyProfileError`] for an opacity, exponent or ratio that is not finite and positive,
    /// or an f outside (0, 1].
    pub fn new(
        thermal_opacity: f64,
        optical_depth_exponent: f64,
        opacity_ratio: f64,
        redistribution: f64,
    ) -> Result<Self, BuildGreyProfileError> {
        if !is_positive(thermal_opacity) {
            return Err(BuildGreyProfileError::ThermalOpacity(thermal_opacity));
        }
        if !is_positive(optical_depth_exponent) {
            return Err(BuildGreyProfileError::OpticalDepthExponent(
                optical_depth_exponent,
            ));
        }
        if !is_positive(opacity_ratio) {
            return Err(BuildGreyProfileError::OpacityRatio(opacity_ratio));
        }
        if !(is_positive(redistribution) && redistribution <= 1.0) {
            return Err(BuildGreyProfileError::Redistribution(redistribution));
        }
        Ok(Self {
            thermal_opacity,
            optical_depth_exponent,
            opacity_ratio,
            redistribution,
        })
    }

    /// The thermal opacity `κ_ref` at [`REFERENCE_PRESSURE`], m² kg⁻¹.
    #[must_use]
    pub const fn thermal_opacity(&self) -> f64 {
        self.thermal_opacity
    }

    /// The opacity's exponent in pressure n: τ ∝ pⁿ, 1 for a constant opacity.
    #[must_use]
    pub const fn optical_depth_exponent(&self) -> f64 {
        self.optical_depth_exponent
    }

    /// γ, the visible opacity over the thermal, dimensionless.
    #[must_use]
    pub const fn opacity_ratio(&self) -> f64 {
        self.opacity_ratio
    }

    /// f, the redistribution of the irradiation over the planet.
    #[must_use]
    pub const fn redistribution(&self) -> f64 {
        self.redistribution
    }

    /// The thermal optical depth τ at pressure `p` under gravity `gravity`: `κ_ref` `p_ref` (p ÷
    /// `p_ref`)ⁿ ÷ (n g); 0 at and above p = 0.
    #[must_use]
    pub fn optical_depth(&self, p: Pascals, gravity: MetresPerSecondSquared) -> f64 {
        let p = p.value();
        if p.is_nan() {
            return f64::NAN;
        }
        if p <= 0.0 {
            return 0.0;
        }
        let n = self.optical_depth_exponent;
        let p_ref = REFERENCE_PRESSURE.value();
        self.thermal_opacity * p_ref * math::powf(p / p_ref, n) / (n * gravity.value())
    }

    /// The pressure at which the thermal optical depth is `tau` under gravity `gravity`, the
    /// inverse of [`optical_depth`](Self::optical_depth).
    #[must_use]
    pub fn pressure_at_depth(&self, tau: f64, gravity: MetresPerSecondSquared) -> Pascals {
        let n = self.optical_depth_exponent;
        let p_ref = REFERENCE_PRESSURE.value();
        Pascals::new(
            p_ref
                * math::powf(
                    tau * n * gravity.value() / (self.thermal_opacity * p_ref),
                    1.0 / n,
                ),
        )
    }

    /// Guillot's T⁴ at thermal optical depth `tau` of an envelope of internal temperature
    /// `internal` and irradiation temperature `irradiation`, K⁴ (Eq. 29).
    #[must_use]
    pub fn fourth_power(&self, tau: f64, internal: Kelvin, irradiation: Kelvin) -> f64 {
        let gamma = self.opacity_ratio;
        let int4 = math::powi(internal.value(), 4);
        let irr4 = self.redistribution * math::powi(irradiation.value(), 4);
        let inverse = 1.0 / (gamma * SQRT_3);
        0.75 * int4 * (2.0 / 3.0 + tau)
            + 0.75
                * irr4
                * (2.0 / 3.0
                    + inverse
                    + (gamma / SQRT_3 - inverse) * math::exp(-gamma * tau * SQRT_3))
    }

    /// Guillot's temperature at thermal optical depth `tau` (Eq. 29; [`fourth_power`](Self::fourth_power)).
    #[must_use]
    pub fn temperature(&self, tau: f64, internal: Kelvin, irradiation: Kelvin) -> Kelvin {
        Kelvin::new(math::powf(
            self.fourth_power(tau, internal, irradiation),
            0.25,
        ))
    }

    /// The radiative gradient d ln T ÷ d ln p at thermal optical depth `tau`: n τ (dT⁴ ÷ dτ) ÷ (4
    /// T⁴), with dT⁴ ÷ dτ = ¾ [`T_int`⁴ + f `T_irr`⁴ (1 − γ²) e^(−γ τ √3)].
    #[must_use]
    pub fn radiative_gradient(&self, tau: f64, internal: Kelvin, irradiation: Kelvin) -> f64 {
        let gamma = self.opacity_ratio;
        let int4 = math::powi(internal.value(), 4);
        let irr4 = self.redistribution * math::powi(irradiation.value(), 4);
        let slope = 0.75 * (int4 + irr4 * (1.0 - gamma * gamma) * math::exp(-gamma * tau * SQRT_3));
        self.optical_depth_exponent * tau * slope
            / (4.0 * self.fourth_power(tau, internal, irradiation))
    }

    /// The optical depth above which an envelope of internal temperature `internal` and
    /// irradiation temperature `irradiation` has joined its adiabat, for γ ≤ 1 and n = 2: `τ_max` =
    /// 4 (⅔ + `c₀` f `T_irr`⁴ ÷ `T_int`⁴), `c₀` = ⅔ + 1 ÷ (γ√3), where the gradient is at least ⅖, every
    /// ideal gas's largest R ÷ `c_p` (decision-p14-t24d-profile §1.2, checked over (`T_eq` ÷
    /// `T_int`)⁴ from 10⁻⁸ to 10¹² at γ 0.1–1); infinite where `T_int` is 0.
    #[must_use]
    pub fn join_bracket_optical_depth(&self, internal: Kelvin, irradiation: Kelvin) -> f64 {
        let int4 = math::powi(internal.value(), 4);
        let irr4 = self.redistribution * math::powi(irradiation.value(), 4);
        let c0 = 2.0 / 3.0 + 1.0 / (self.opacity_ratio * SQRT_3);
        if int4 > 0.0 {
            4.0 * (2.0 / 3.0 + c0 * irr4 / int4)
        } else {
            f64::INFINITY
        }
    }
}

/// What [`EnvelopeProfile::derive`] reads, as plain values. The derivation checks each, so they
/// are public.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvelopeProfileInputs<'a> {
    /// The internal temperature `T_int`, finite and not negative: the giant cooling's (ruling
    /// 112.7), or the one P14.T24.d states for an envelope the record gives none.
    pub internal_temperature: Kelvin,
    /// The irradiation temperature `T_irr` ([`irradiation_temperature`]), finite and not negative;
    /// 0 for a body no star lights.
    pub irradiation_temperature: Kelvin,
    /// The gravity at the envelope's visible level, the bulk section's surface gravity, finite and
    /// positive.
    pub gravity: MetresPerSecondSquared,
    /// The mixture whose `c_p`(T) sets the adiabat, by mole fraction, summing to 1 within 10⁻⁶,
    /// each row with gas columns (P14.T49.b).
    pub gases: &'a [(SubstanceId, f64)],
    /// Guillot's parameters.
    pub grey: GreyProfile,
}

/// Why [`EnvelopeProfile::derive`] gives no profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveEnvelopeProfileError {
    /// The mixture is empty, a mole fraction is not finite and not negative, or they do not sum
    /// to 1 within 10⁻⁶.
    Mixture,
    /// A row of the mixture has no gas columns, so no `c_p`(T).
    NoHeatCapacity(SubstanceId),
    /// The internal temperature is not finite and not negative.
    InternalTemperature(Kelvin),
    /// The irradiation temperature is not finite and not negative.
    IrradiationTemperature(Kelvin),
    /// Neither an internal flux nor starlight heats the envelope.
    Unheated,
    /// The gravity is not finite and positive, m s⁻².
    Gravity(f64),
    /// The radiative gradient exceeds no adiabat's above the join's bracket
    /// ([`GreyProfile::join_bracket_optical_depth`]): a constant opacity's, an envelope's with no
    /// internal heat, or one past the bracket's γ ≤ 1.
    NoJoin,
    /// The adiabat would need more than P14.T24.e's most segments.
    TooDeep,
}

impl fmt::Display for DeriveEnvelopeProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mixture => write!(f, "an envelope's mole fractions do not sum to one"),
            Self::NoHeatCapacity(id) => write!(
                f,
                "{} has no gas columns, so no heat capacity",
                id.substance().key().as_str()
            ),
            Self::InternalTemperature(t) => {
                write!(
                    f,
                    "an internal temperature is finite and not negative, got {t:?}"
                )
            }
            Self::IrradiationTemperature(t) => write!(
                f,
                "an irradiation temperature is finite and not negative, got {t:?}"
            ),
            Self::Unheated => write!(f, "an envelope with neither internal heat nor starlight"),
            Self::Gravity(g) => write!(f, "a gravity is finite and positive, got {g} m/s2"),
            Self::NoJoin => write!(
                f,
                "the grey profile's gradient never exceeds the adiabat's above 100 kbar"
            ),
            Self::TooDeep => write!(f, "the adiabat needs too many segments"),
        }
    }
}

impl Error for DeriveEnvelopeProfileError {}

/// A gas envelope's vertical structure (the [module](self)'s documentation): Guillot's grey
/// profile above its join, the dry adiabat's nodes below it.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvelopeProfile {
    internal_temperature: Kelvin,
    irradiation_temperature: Kelvin,
    gravity: MetresPerSecondSquared,
    grey: GreyProfile,
    nodes: Vec<ProfileNode>,
}

impl EnvelopeProfile {
    /// The profile of the envelope `inputs` describe: its join, the first level going down at
    /// which the grey profile's gradient exceeds the adiabat's, and the adiabat's nodes from there
    /// to [`ADIABAT_BOTTOM`] (at least one segment).
    ///
    /// # Errors
    ///
    /// As [`DeriveEnvelopeProfileError`]'s variants: a mixture that is not one or holds a row with
    /// no gas columns, a temperature or gravity out of its range, an envelope neither heated from
    /// below nor lit, no join above its bracket, or an adiabat too deep.
    pub fn derive(inputs: &EnvelopeProfileInputs<'_>) -> Result<Self, DeriveEnvelopeProfileError> {
        check(inputs)?;
        let gases = inputs.gases;
        let (grey, g) = (inputs.grey, inputs.gravity);
        let (internal, irradiation) = (inputs.internal_temperature, inputs.irradiation_temperature);
        let gap = |ln_p: f64| {
            let tau = grey.optical_depth(Pascals::new(math::exp(ln_p)), g);
            let t = grey.temperature(tau, internal, irradiation);
            grey.radiative_gradient(tau, internal, irradiation) - 1.0 / heat_capacity(gases, t)
        };
        let top = math::ln(
            grey.pressure_at_depth(JOIN_SCAN_TOP_OPTICAL_DEPTH, g)
                .value(),
        );
        let bracket_depth = grey.join_bracket_optical_depth(internal, irradiation);
        if !bracket_depth.is_finite() {
            return Err(DeriveEnvelopeProfileError::NoJoin);
        }
        // One step past the bracket, where a gas of c_p ÷ R exactly 5/2 meets it.
        let bottom =
            math::ln(grey.pressure_at_depth(bracket_depth, g).value()) + JOIN_SCAN_STEP_LN_P;
        let levels = ((bottom - top) / JOIN_SCAN_STEP_LN_P).ceil();
        if !(levels.is_finite() && levels >= 1.0) {
            return Err(DeriveEnvelopeProfileError::NoJoin);
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole number of at most about 29,000 levels (ln p spans under 1,500 \
                      between any two positive doubles), checked positive above"
        )]
        let levels = levels as u32;
        let mut upper = top;
        let mut bracket = None;
        for k in 1..=levels {
            let lower = (top + f64::from(k) * JOIN_SCAN_STEP_LN_P).min(bottom);
            if gap(lower) > 0.0 {
                bracket = Some((upper, lower));
                break;
            }
            upper = lower;
        }
        let (mut above, mut below) = bracket.ok_or(DeriveEnvelopeProfileError::NoJoin)?;
        for _ in 0..JOIN_BISECTIONS {
            let middle = f64::midpoint(above, below);
            if gap(middle) > 0.0 {
                below = middle;
            } else {
                above = middle;
            }
        }
        let p_join = Pascals::new(math::exp(below));
        let t_join = grey.temperature(grey.optical_depth(p_join, g), internal, irradiation);
        let nodes = adiabat_nodes(gases, p_join, t_join)?;
        Ok(Self {
            internal_temperature: internal,
            irradiation_temperature: irradiation,
            gravity: g,
            grey,
            nodes,
        })
    }

    /// The internal temperature `T_int`.
    #[must_use]
    pub const fn internal_temperature(&self) -> Kelvin {
        self.internal_temperature
    }

    /// The irradiation temperature `T_irr`.
    #[must_use]
    pub const fn irradiation_temperature(&self) -> Kelvin {
        self.irradiation_temperature
    }

    /// The gravity the optical depth is taken under.
    #[must_use]
    pub const fn gravity(&self) -> MetresPerSecondSquared {
        self.gravity
    }

    /// Guillot's parameters.
    #[must_use]
    pub const fn grey(&self) -> &GreyProfile {
        &self.grey
    }

    /// The join's pressure `p_join`, where the profile becomes the adiabat: the first node's.
    #[must_use]
    pub fn join_pressure(&self) -> Pascals {
        self.nodes[0].pressure()
    }

    /// The adiabat's nodes below the join, the pressures ascending evenly in ln p: (`p_join`,
    /// `T_join`) first and the deepest, at [`ADIABAT_BOTTOM`] or one segment below the join, last.
    #[must_use]
    pub fn nodes(&self) -> &[ProfileNode] {
        &self.nodes
    }

    /// The temperature at τ = 0, the grey profile's skin: `T_int`⁴ ÷ 2 + f `T_irr`⁴ (½ + √3 γ ÷ 4).
    #[must_use]
    pub fn skin_temperature(&self) -> Kelvin {
        self.grey
            .temperature(0.0, self.internal_temperature, self.irradiation_temperature)
    }

    /// The thermal photosphere's pressure, where τ = ⅔.
    #[must_use]
    pub fn photosphere_pressure(&self) -> Pascals {
        self.grey.pressure_at_depth(2.0 / 3.0, self.gravity)
    }

    /// The temperature at pressure `p`: Guillot's above the join (its skin at and above p = 0),
    /// the power law between the two nodes about p below it, and the last segment's power law
    /// below the last node. At the join it is the join's temperature from either side, and at a
    /// node's own pressure that node's temperature, bit for bit. A NaN pressure gives a NaN
    /// temperature.
    #[must_use]
    pub fn temperature_at(&self, p: Pascals) -> Kelvin {
        let value = p.value();
        if value.is_nan() {
            return Kelvin::new(f64::NAN);
        }
        if value < self.join_pressure().value() {
            let tau = self.grey.optical_depth(p, self.gravity);
            return self.grey.temperature(
                tau,
                self.internal_temperature,
                self.irradiation_temperature,
            );
        }
        // The first node deeper than p, so that a node's own pressure takes the segment it starts;
        // below the deepest node, its own segment's power law, anchored at it.
        let deeper = self
            .nodes
            .partition_point(|node| node.pressure().value() <= value);
        let len = self.nodes.len();
        let (a, b, anchor) = if deeper >= len {
            let (a, b) = (self.nodes[len - 2], self.nodes[len - 1]);
            (a, b, b)
        } else {
            let (a, b) = (self.nodes[deeper - 1], self.nodes[deeper]);
            (a, b, a)
        };
        let exponent = math::ln(b.temperature().value() / a.temperature().value())
            / math::ln(b.pressure().value() / a.pressure().value());
        Kelvin::new(
            anchor.temperature().value()
                * math::exp(exponent * math::ln(value / anchor.pressure().value())),
        )
    }
}

/// The temperature of the dry adiabat of the mixture `gases` through (`from`, `start`) at pressure
/// `p`: S̃(T) = S̃(`start`) + ln(p ÷ `from`), by P14.T24.e's Newton steps from the power law at the
/// start's `c_p`; `None` for a mixture with a row of no gas columns, or a pressure or temperature
/// that is not finite and positive.
///
/// # Examples
///
/// The Galileo probe's 427.7 K at 22 bar (Seiff et al. 1998) carried up Jupiter's adiabat, normal
/// hydrogen with its measured helium and methane, reaches its measured 166.1 K at 1 bar within a
/// kelvin (science-r08-gases-2.md §1.4: 166.5 K):
///
/// ```
/// use hyperion_sim::planetary::hooks::structure::envelope::dry_adiabat_temperature;
/// use hyperion_sim::substance::by_key;
/// use hyperion_sim::units::{Kelvin, Pascals};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let [h2, he, ch4] = ["H2", "He", "CH4"].map(|k| by_key(k).ok_or("a row"));
/// let jupiter = [(h2?, 0.8620), (he?, 0.1359), (ch4?, 0.0021)];
/// let one_bar = dry_adiabat_temperature(&jupiter, Pascals::new(22e5), Kelvin::new(427.7), Pascals::new(1e5))
///     .ok_or("an adiabat")?;
/// assert!((one_bar.value() - 166.1).abs() < 1.0);
/// # Ok(())
/// # }
/// ```
#[must_use]
pub fn dry_adiabat_temperature(
    gases: &[(SubstanceId, f64)],
    from: Pascals,
    start: Kelvin,
    p: Pascals,
) -> Option<Kelvin> {
    if !(is_positive(from.value()) && is_positive(start.value()) && is_positive(p.value())) {
        return None;
    }
    let s0 = mixture_entropy_over_r(gases, start)?;
    let c0 = mixture_heat_capacity_over_r_at(gases, start)?;
    let ln_ratio = math::ln(p.value() / from.value());
    let predictor = Kelvin::new(start.value() * math::exp(ln_ratio / c0));
    let t = adiabat_temperature(gases, s0 + ln_ratio, predictor);
    is_positive(t.value()).then_some(t)
}

/// The inputs' checks (their doc comments' ranges).
fn check(inputs: &EnvelopeProfileInputs<'_>) -> Result<(), DeriveEnvelopeProfileError> {
    let gases = inputs.gases;
    if gases.is_empty() {
        return Err(DeriveEnvelopeProfileError::Mixture);
    }
    let mut total = 0.0;
    for &(_, x) in gases {
        if !(x.is_finite() && x >= 0.0) {
            return Err(DeriveEnvelopeProfileError::Mixture);
        }
        total += x;
    }
    if (total - 1.0).abs().is_nan() || (total - 1.0).abs() > MIXTURE_TOLERANCE {
        return Err(DeriveEnvelopeProfileError::Mixture);
    }
    if let Some(&(id, _)) = gases.iter().find(|(id, _)| id.substance().gas().is_none()) {
        return Err(DeriveEnvelopeProfileError::NoHeatCapacity(id));
    }
    let (internal, irradiation) = (inputs.internal_temperature, inputs.irradiation_temperature);
    let not_negative = |t: Kelvin| t.value().is_finite() && t.value() >= 0.0;
    if !not_negative(internal) {
        return Err(DeriveEnvelopeProfileError::InternalTemperature(internal));
    }
    if !not_negative(irradiation) {
        return Err(DeriveEnvelopeProfileError::IrradiationTemperature(
            irradiation,
        ));
    }
    if internal.value() <= 0.0 && irradiation.value() <= 0.0 {
        return Err(DeriveEnvelopeProfileError::Unheated);
    }
    let g = inputs.gravity.value();
    if !is_positive(g) {
        return Err(DeriveEnvelopeProfileError::Gravity(g));
    }
    Ok(())
}

/// The adiabat's nodes from (`p_join`, `t_join`) down to [`ADIABAT_BOTTOM`], or one segment below
/// the join where it lies deeper (the [module](self)'s documentation).
fn adiabat_nodes(
    gases: &[(SubstanceId, f64)],
    p_join: Pascals,
    t_join: Kelvin,
) -> Result<Vec<ProfileNode>, DeriveEnvelopeProfileError> {
    let join = ProfileNode::new(p_join, t_join);
    let deepest = ADIABAT_BOTTOM
        .value()
        .max(p_join.value() * math::exp(NODE_SPACING_LN_P));
    let span = math::ln(deepest / p_join.value());
    let segments = segment_count(span).ok_or(DeriveEnvelopeProfileError::TooDeep)?;
    let n = f64::from(segments);
    let step = span / n;
    let join_entropy = entropy(gases, t_join);
    // A capacity hint only: at most P14.T24.e's most segments plus one.
    let mut nodes = Vec::with_capacity(usize::try_from(segments).map_or(0, |n| n + 1));
    nodes.push(join);
    let (mut previous_temperature, mut previous_heat_capacity) =
        (t_join, heat_capacity(gases, t_join));
    for k in 1..=segments {
        let ln_ratio = (f64::from(k) / n) * span;
        let start =
            Kelvin::new(previous_temperature.value() * math::exp(step / previous_heat_capacity));
        let temperature = adiabat_temperature(gases, join_entropy + ln_ratio, start);
        let pressure = if k == segments {
            Pascals::new(deepest)
        } else {
            Pascals::new(p_join.value() * math::exp(ln_ratio))
        };
        nodes.push(ProfileNode::new(pressure, temperature));
        previous_temperature = temperature;
        previous_heat_capacity = heat_capacity(gases, temperature);
    }
    Ok(nodes)
}
