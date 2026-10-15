//! The ultimate bearing capacity of the ground under a footing: a function of the footing's width,
//! the body's gravity and the ground's slope, not a constant of the ground (plan R10, T1.a, Design
//! note 2).
//!
//! # The formula
//!
//! For a square or circular footing of width B resting flush on the surface, with no surcharge
//! beside it, on ground of slope β (radians, 0 on level ground), under a vertical load through its
//! centre, the ultimate pressure normal to its base is
//!
//! ```text
//! q_ult = c N_c s_c i_c + ½ ρ (g cos β) B N_γ s_γ i_γ z_γ               0 ≤ β < φ
//! N_q = exp(π tan φ) tan²(45° + φ/2)    N_c = (N_q − 1) cot φ    N_γ = 2 (N_q + 1) tan φ
//! s_c = 1 + N_q ÷ N_c                    s_γ = 0.6                (B = L)
//! x = tan β · q_ult ÷ (q_ult + c cot φ)  i_q = (1 − x)^m   i_γ = (1 − x)^(m+1)   m = 1.5 (B = L)
//! i_c = i_q − (1 − i_q) ÷ (N_c tan φ)    z_γ = (1 − tan β ÷ tan φ)^0.35
//! ```
//!
//! On level ground every i and z is 1, and the formula is the level footing's,
//! `q_ult` = c `N_c` `s_c` + ½ ρ g B `N_γ` `s_γ` ("On a slope", below, for the rest).
//!
//! Terzaghi's general bearing-capacity equation (Terzaghi 1943, Theoretical Soil Mechanics, Wiley)
//! with its surcharge term zero, with Prandtl's and Reissner's `N_q`, its `N_c`, Vesić's `N_γ` and
//! Vesić's shape factors at B ÷ L = 1, `s_c` = 1 + (B ÷ L)(`N_q` ÷ `N_c`) and `s_γ` = 1 − 0.4 B ÷ L
//! (Vesić 1973, J. Soil Mech. Found. Div. ASCE 99 (SM1), 45; Vesić 1975, in Winterkorn and Fang,
//! Foundation Engineering Handbook, ch. 3; both as US Army Corps of Engineers EM 1110-1-1905 (1992)
//! gives them, Eq. 4-1, p. 4-2, and Table 4-6, p. 4-9). The Lunar Sourcebook writes it after
//! Durgunoglu and Mitchell, `q_ult` = c `N_c` `ξ_c` + ρ `g_m` B `N_γq` `ξ_γq`, with no one half
//! (Carrier, Olhoeft and Mendell 1991, ch. 9, §9.1.9, p. 517): their combined factor is `N_γq` = ½
//! `N_γ` + (D ÷ B) `N_q` and their `ξ_γq` = 1 − 0.4 B ÷ L at a surface footing, D ÷ B = 0
//! (Durgunoglu and Mitchell 1973, "Static penetration resistance of soils", NASA-CR-133460, eqs.
//! 4.28a and 4.30, p. 91), so the two forms agree. Vesić's `N_γ` is a closed form for Caquot and
//! Kérisel's (1953) factor, as EM 1110-1-1905 notes; at the deeper lunar soil's 54° it may lie past
//! the range it was fitted over, unverified (plan R10, Risks). c, φ and ρ are the layer's at the
//! depth the failure zone reaches, about one footing width
//! ([`MaterialProperties::shear_layer_at`]), ρ its bulk density.
//!
//! On the Moon the Sourcebook gives about 6,000 kPa for a 1 m footing (p. 517) and, in its text on
//! p. 518, read off its Fig. 9.36, 3,000–11,000 kPa at the Apollo 11 lunar module's footpads,
//! nearly 1 m across. The formula on its 30–60 cm layer gives 5,370 kPa, and on its 0–15 cm layer
//! 207 kPa: the near surface alone could not carry what the deeper soil does (Design note 2).
//!
//! # On a slope
//!
//! A lander's pad rests flush on the ground, its base tilted with the slope β, under the craft's
//! vertical weight. Turned through β, that is a surface footing on level ground in two ways:
//! - its load leans at β to the base's normal, H = V tan β;
//! - its soil's weight leans by the same β, ρ g cos β normal to the base and tan β times that
//!   along it.
//!
//! That is Paolucci and Pecker's pseudo-static seismic problem with a horizontal coefficient
//! `k_h` = tan β on both, "the soil inertia `f_x` = `k_h` `f_y` and the horizontal component of the
//! seismic action H = `k_h` V" (Paolucci and Pecker 1997, "Seismic bearing capacity of shallow
//! strip foundations on dry soils", Soils and Foundations 37(3), 95–105, Fig. 11, p. 101; read in
//! the scan J-STAGE publishes). On a static slope it is exact, so no ground-slope or base-tilt
//! factor enters (ruled 2026-10-14; plan R10, Risks, "Deviations in T1.a's slope follow-up, as
//! built").
//! - _The load's inclination_ takes Vesić's inclination factors (Vesić 1975, "Bearing capacity of
//!   shallow foundations", in Winterkorn and Fang, Foundation Engineering Handbook, ch. 3, §3.6;
//!   read in a scanned copy):
//!   - `ζ_qi` = [1 − P ÷ (Q + B′L′c cot φ)]^m, his Eq. 3.14a (p. 130);
//!   - `ζ_γi`, the same with the exponent m + 1, Eq. 3.17a (p. 131);
//!   - m = (2 + B/L) ÷ (1 + B/L) for a load inclined along B, Eq. 3.18a (p. 131), 1.5 at B = L;
//!   - `ζ_ci` = `ζ_qi` − (1 − `ζ_qi`) ÷ (`N_c` tan φ), "from the theorem of correspondence",
//!     Eq. 3.15 (p. 130, after De Beer and Ladanyi 1961).
//!
//!   With P = V tan β and Q = V = `q_ult` B² normal to the base, P ÷ (Q + B²c cot φ) is x. Vesić
//!   prints the soil's c there, but his Example 3.3(b) applies it with the adhesion `c_a` = 0
//!   (p. 131), as US Army Corps of Engineers EM 1110-1-1905 (1992) writes `c_a` (Table 4-6,
//!   p. 4-9). Here `c_a` = c: the pad grips as the ground holds.
//! - _The soil's inertia_ takes Paolucci and Pecker's `z_γ` = (1 − `k_h` ÷ tan φ)^0.35 on the
//!   weight term (their Eq. 10, p. 99). It was fitted in plane strain at φ 30°–40° (their Fig. 9b,
//!   p. 100) and confirmed by finite elements for `k_h` ≤ 0.3 (p. 99), and is used here at
//!   18°–54° on a square pad and up to `k_h` = tan φ, extrapolated. The cohesion term takes `z_c` =
//!   1, since the cohesion term is the weightless soil's and no body force acts on it. Their Fig.
//!   9a's `z_c` has no formula, and is not adopted: read off the figure it is about 5% on the
//!   cohesion term at 10°, so about 3–4% of the lunar 1 m pad's capacity there.
//! - _Which dominates._ They find "the effect of the soil inertia is negligible with respect to the
//!   seismic actions transmitted by the structure … at least for low seismic coefficients (`k_h` ≤
//!   0.2 approximately)" (p. 102). Their kinematic approach "predicts the failure of the foundation
//!   for `k_h` = tan φ" (p. 99), which is β = φ.
//!
//! _The solve._ The pressure enters x, since V is the capacity itself. The capacity is the root of
//! q = `q_ult`(x(q)), found by bisection on [0, the level capacity] with a fixed 64 halvings. The
//! right side falls as q rises, so the root is unique, and a fixed count makes it the same on every
//! target. Without the c cot φ term `i_c` would diverge as φ nears 0, and the Martian drift's
//! figure at 10° would be 7% low. With c = 0, x = tan β and there is no solve. 1 − x is held at 0.
//!
//! _The range._ The capacity holds for 0 ≤ β < φ:
//! - Paolucci and Pecker's analysis holds for `k_h` < tan φ, at which their kinematic approach
//!   predicts the foundation's failure;
//! - Brinch Hansen's slope factors hold for β "smaller than φ" (1970, "A revised and extended
//!   formula for bearing capacity", Danish Geotechnical Institute Bulletin 28, §7, p. 8), and
//!   Vesić's for ω < φ (his Eq. 3.25, p. 132);
//! - for a pad as rough as the ground, H = V tan β stays within Vesić's sliding limit
//!   `P_max` = Q tan φ + A′`c_a`, his φ there the soil–footing friction angle (his Eq. 3.12,
//!   p. 130), at the soil's φ and `c_a` = c.
//!
//! Vesić's further bound, α < 45° and ω < 45° (his Eq. 3.24, p. 132), belongs to his tilt and
//! slope factors, which are not used, so a cohesive soil of φ above 45° keeps a capacity past 45°.
//! The shape factors multiply the inclination factors, as in Vesić's Eq. 3.13 (p. 130) and his
//! Example 3.3, where EM 1110-1-1905's note under its Table 4-6 would not use eccentricity and
//! inclination factors together.
//!
//! At β ≥ φ, outside every source's range, the capacity is `None`, never 0. It is also `None` on a
//! cohesionless soil at tan β ≥ 1, tested on the angle at 45°, since `math::tan` puts tan 45° a
//! rounding below 1, and wherever else it is not positive and finite: a cohesionless soil with no
//! weight term, or an overflow ([`ultimate_bearing_capacity_on_slope`]).
//!
//! _Toward φ._ A cohesionless capacity falls to 0, as (φ − β)^0.35 on a soil of φ below 45°, and
//! as (1 − tan β)^2.5 toward 45° on a steeper one. A cohesive one falls to its inclined cohesion
//! term alone, c `N_c` `s_c` `i_c`. Under a 1 m pad at φ − 10⁻⁹ rad, that is 0.488 of the level
//! figure on Mars's drift and 0.144 on Viking Lander 2's soil at 3.71 m s⁻², and 0.029 on the lunar
//! 0–15 cm pair and 0.0010 on the lunar 30–60 cm layer at 1.62 m s⁻².
//!
//! _Dropped readings_ (plan R10, Risks). Two were built or weighed and dropped:
//! - Vesić's ground-slope factors (1 − tan β)² for a level base cut into the slope (his Eq. 3.21).
//!   They leave out the shear the pad's own weight puts on its base.
//! - Those factors with his or Hansen's base-tilt factors. A flush pad takes those at a negative
//!   tilt, outside Hansen's range, and in weightless soil their product is not 1, as a flush pad's
//!   must be.
//!
//! On the Moon a 1 m pad on lunar regolith on a 10° slope bears 3,730 kPa, 69% of its level
//! 5,370 kPa, and on a 20° slope 2,331 kPa.

use hyperion_base::math;
use hyperion_base::units::{
    KilogramsPerCubicMetre, Metres, MetresPerSecondSquared, Pascals, Radians,
};

use super::{MaterialProperties, MohrCoulomb};

/// The bearing-capacity factors of a friction angle: `N_q`, `N_c` and `N_γ` (Vesić 1973, 1975; the
/// [module](self)'s formula).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BearingFactors {
    surcharge: f64,
    cohesion: f64,
    self_weight: f64,
}

impl BearingFactors {
    /// The factors at `strength`'s friction angle φ (the [module](self)'s formulas).
    ///
    /// A φ at or below 0 takes the frictionless limit, `N_q` = 1, `N_c` = π + 2 (Prandtl 1921)
    /// and `N_γ` = 0.
    #[must_use]
    pub fn of(strength: MohrCoulomb) -> Self {
        let phi = strength.friction_angle().value();
        if phi <= 0.0 {
            return Self {
                surcharge: 1.0,
                cohesion: core::f64::consts::PI + 2.0,
                self_weight: 0.0,
            };
        }
        let tan_phi = math::tan(phi);
        let passive = math::tan(core::f64::consts::FRAC_PI_4 + 0.5 * phi);
        let n_q = math::exp(core::f64::consts::PI * tan_phi) * passive * passive;
        Self {
            surcharge: n_q,
            cohesion: (n_q - 1.0) / tan_phi,
            self_weight: 2.0 * (n_q + 1.0) * tan_phi,
        }
    }

    /// `N_q`, the surcharge factor.
    #[must_use]
    pub const fn n_q(self) -> f64 {
        self.surcharge
    }

    /// `N_c`, the cohesion factor.
    #[must_use]
    pub const fn n_c(self) -> f64 {
        self.cohesion
    }

    /// `N_γ`, the self-weight factor.
    #[must_use]
    pub const fn n_gamma(self) -> f64 {
        self.self_weight
    }
}

/// The self-weight shape factor of a square or circular footing, `s_γ` = 1 − 0.4 B ÷ L at B = L
/// (Vesić 1973, 1975, as EM 1110-1-1905 Table 4-6 gives it).
const SQUARE_SELF_WEIGHT_SHAPE: f64 = 0.6;

/// The exponent m of Vesić's inclination factors for a load inclined across a square or circular
/// footing: (2 + B ÷ L) ÷ (1 + B ÷ L) at B = L, for a load inclined along B (Vesić 1975, Eq. 3.18a,
/// p. 131).
const SQUARE_INCLINATION_EXPONENT: f64 = 1.5;

/// The exponent of Paolucci and Pecker's soil-inertia factor, `z_γ` = (1 − `k_h` ÷ tan φ)^0.35
/// (Paolucci and Pecker 1997, Eq. 10, p. 99), fitted in plane strain at φ 30°–40° (their Fig. 9b,
/// p. 100) and confirmed by finite elements for `k_h` ≤ 0.3 (p. 99); used here at 18°–54° on a
/// square pad, extrapolated.
const SOIL_INERTIA_EXPONENT: f64 = 0.35;

/// The halvings of the solve for the pressure on a slope: a fixed count, so that the capacity is
/// the same on every target and in every call (the [module](self)'s "The solve"). 64 bring the
/// root within 2⁻⁶⁴ of the level capacity, which is within an ulp of the root wherever the root is
/// at least 2⁻¹² of the level figure, as on every table's layer.
const PRESSURE_HALVINGS: u32 = 64;

/// Vesić's inclination factors (`ζ_qi`, `ζ_ci`, `ζ_γi`) at x = P ÷ (Q + B′L′c cot φ), for the
/// exponent `m` and `corresponding` = `N_c` tan φ: (1 − x)^m, its cohesion factor by Eq. 3.15, and
/// (1 − x)^(m+1) (Vesić 1975, Eqs. 3.14a, 3.15 and 3.17a, pp. 130–131), 1 − x held at 0.
#[must_use]
fn inclination_factors(x: f64, m: f64, corresponding: f64) -> (f64, f64, f64) {
    let base = at_least_zero(1.0 - x);
    let surcharge = math::powf(base, m);
    (
        surcharge,
        surcharge - (1.0 - surcharge) / corresponding,
        math::powf(base, m + 1.0),
    )
}

/// Paolucci and Pecker's soil-inertia factor `z_γ` = (1 − `k_h` ÷ tan φ)^0.35 at `k_h` = `tan_beta`
/// and tan φ = `tan_phi` (1997, Eq. 10, p. 99), its base held at 0.
#[must_use]
fn soil_inertia_factor(tan_beta: f64, tan_phi: f64) -> f64 {
    math::powf(
        at_least_zero(1.0 - tan_beta / tan_phi),
        SOIL_INERTIA_EXPONENT,
    )
}

/// The pressure normal to the base of a pad resting flush on ground of slope `slope` (above 0 and
/// below φ), and its x, by the [module](self)'s formula, solved for the pressure where c > 0.
///
/// It takes the level footing's terms, `cohesion_term` (c `N_c` `s_c`) and `weight_term`
/// (½ ρ g B `N_γ` `s_γ`), and the bearing factors `factors` of the soil's strength `strength`.
#[must_use]
fn flush_pad(
    factors: BearingFactors,
    strength: MohrCoulomb,
    slope: Radians,
    cohesion_term: Pascals,
    weight_term: Pascals,
) -> (Pascals, f64) {
    let (c, beta) = (strength.cohesion().value(), slope.value());
    let (cohesion, weight) = (cohesion_term.value(), weight_term.value());
    let tan_phi = math::tan(strength.friction_angle().value());
    let tan_beta = math::tan(beta);
    // N_c tan φ, which is N_q − 1 exactly in the module's factors.
    let corresponding = factors.n_q() - 1.0;
    let inclined_weight = weight * math::cos(beta) * soil_inertia_factor(tan_beta, tan_phi);
    let pressure_at = |x: f64| {
        let (_, i_c, i_gamma) = inclination_factors(x, SQUARE_INCLINATION_EXPONENT, corresponding);
        cohesion * i_c + inclined_weight * i_gamma
    };
    if c <= 0.0 {
        return (Pascals::new(pressure_at(tan_beta)), tan_beta);
    }
    // x = tan β q ÷ (q + c cot φ) rises with q, so the right side falls with it: the root of
    // q = pressure_at(x(q)) is unique and lies between 0 and the level capacity.
    let held = c / tan_phi;
    let x_at = |q: f64| tan_beta * q / (q + held);
    let (mut low, mut high) = (0.0, cohesion + weight);
    for _ in 0..PRESSURE_HALVINGS {
        let mid = f64::midpoint(low, high);
        if pressure_at(x_at(mid)) > mid {
            low = mid;
        } else {
            high = mid;
        }
    }
    let pressure = f64::midpoint(low, high);
    (Pascals::new(pressure), x_at(pressure))
}

/// `x`, or 0 where `x` is below 0 or NaN, without `f64::max`, which may return either zero at a
/// signed zero (the sim-determinism skill's hazards).
#[must_use]
fn at_least_zero(x: f64) -> f64 {
    if x > 0.0 { x } else { 0.0 }
}

/// The level footing's two terms, Pa: the cohesion term c `N_c` `s_c` and the self-weight term
/// ½ ρ g B `N_γ` `s_γ`, in the order and with the roundings T1.a's level capacity always had.
#[must_use]
fn level_terms(
    factors: BearingFactors,
    strength: MohrCoulomb,
    bulk_density: KilogramsPerCubicMetre,
    gravity: MetresPerSecondSquared,
    width: Metres,
) -> (f64, f64) {
    let s_c = 1.0 + factors.surcharge / factors.cohesion;
    let cohesion = strength.cohesion().value() * factors.cohesion * s_c;
    let weight = 0.5
        * bulk_density.value()
        * gravity.value()
        * width.value()
        * factors.self_weight
        * SQUARE_SELF_WEIGHT_SHAPE;
    (cohesion, weight)
}

/// The ultimate bearing capacity, Pa, of ground of strength `strength` and bulk density
/// `bulk_density` under a square or circular surface footing of width `width` at gravity
/// `gravity`, on level ground: `q_ult` = c `N_c` `s_c` + ½ ρ g B `N_γ` `s_γ` (the
/// [module](self)'s formula).
///
/// On a slope it is [`ultimate_bearing_capacity_on_slope`], which gives this figure bit for bit at
/// a slope of 0.
#[must_use]
pub fn ultimate_bearing_capacity(
    strength: MohrCoulomb,
    bulk_density: KilogramsPerCubicMetre,
    gravity: MetresPerSecondSquared,
    width: Metres,
) -> Pascals {
    let (cohesion, weight) = level_terms(
        BearingFactors::of(strength),
        strength,
        bulk_density,
        gravity,
        width,
    );
    Pascals::new(cohesion + weight)
}

/// The ultimate bearing capacity, Pa, of ground of strength `strength` and bulk density
/// `bulk_density` under a square or circular pad of width `width` at gravity `gravity`, resting
/// flush on ground of slope `slope`, radians.
///
/// It is the ultimate pressure normal to the pad's base, under a vertical load through its centre,
/// by the [module](self)'s formula. At a slope of 0, of either sign, it is
/// [`ultimate_bearing_capacity`]'s figure bit for bit, with no solve, whatever φ, wherever that
/// figure is positive and finite. The slope's basis is [`bearing_capacity_on_slope`]'s.
///
/// `None`, never 0, in these cases, so that a `Some` is always positive and finite:
/// - a bulk density, gravity or width that is negative or not finite;
/// - a slope that is negative (a slope is a magnitude) or NaN;
/// - a slope above 0 at or above φ, outside every source's range;
/// - a cohesionless soil at tan β ≥ 1, tested on the angle at 45°;
/// - any other pressure that is not positive and finite: a cohesionless soil with no weight term
///   (zero gravity, density or width), or one that overflows.
#[must_use]
pub fn ultimate_bearing_capacity_on_slope(
    strength: MohrCoulomb,
    bulk_density: KilogramsPerCubicMetre,
    gravity: MetresPerSecondSquared,
    width: Metres,
    slope: Radians,
) -> Option<Pascals> {
    let valid = |x: f64| x.is_finite() && x >= 0.0;
    if !valid(bulk_density.value()) || !valid(gravity.value()) || !valid(width.value()) {
        return None;
    }
    let beta = slope.value();
    if beta.is_nan() || beta < 0.0 {
        return None;
    }
    let factors = BearingFactors::of(strength);
    let (cohesion, weight) = level_terms(factors, strength, bulk_density, gravity, width);
    // Zero of either sign is level ground, whatever the friction angle: the level path.
    let pressure = if beta <= 0.0 {
        cohesion + weight
    } else {
        let phi = strength.friction_angle().value();
        let cohesionless = strength.cohesion().value() <= 0.0;
        // A cohesionless pad's x is tan β, which reaches 1 at 45°, where `math::tan` gives
        // 1 − 2⁻⁵³.
        if beta >= phi || (cohesionless && beta >= core::f64::consts::FRAC_PI_4) {
            return None;
        }
        let (cohesion, weight) = (Pascals::new(cohesion), Pascals::new(weight));
        flush_pad(factors, strength, slope, cohesion, weight)
            .0
            .value()
    };
    (pressure.is_finite() && pressure > 0.0).then(|| Pascals::new(pressure))
}

/// The ultimate bearing capacity, Pa, of the ground of `p` under a square or circular surface
/// footing of width `footing_width` at gravity `gravity`, on level ground, from the layer at the
/// depth of one footing width ([`MaterialProperties::shear_layer_at`]) with its bulk density
/// ([`MaterialProperties::bulk_density`]); `None` where the form has no shear table, or for a
/// width or gravity that is negative or not finite (Design note 2).
///
/// It grows with the width and the gravity, the self-weight term in proportion and the layer
/// deepening with the width. On a slope it is [`bearing_capacity_on_slope`], which gives this
/// figure bit for bit at a slope of 0.
#[must_use]
pub fn bearing_capacity(
    p: &MaterialProperties,
    gravity: MetresPerSecondSquared,
    footing_width: Metres,
) -> Option<Pascals> {
    let valid = |x: f64| x.is_finite() && x >= 0.0;
    if !valid(gravity.value()) || !valid(footing_width.value()) {
        return None;
    }
    let layer = p.shear_layer_at(footing_width)?;
    Some(ultimate_bearing_capacity(
        layer.strength(),
        p.bulk_density(layer),
        gravity,
        footing_width,
    ))
}

/// The ultimate bearing capacity, Pa, of the ground of `p` under a square or circular pad of width
/// `footing_width` at gravity `gravity`, resting flush on ground of slope `slope`, radians.
///
/// It is taken from the layer at the depth of one footing width
/// ([`MaterialProperties::shear_layer_at`]) with its bulk density
/// ([`MaterialProperties::bulk_density`]) (Design note 2), by the [module](self)'s formula: the
/// ultimate pressure normal to the pad's base, under a vertical load through its centre, on a
/// uniform slope. A crest's or a toe's nearness is not modelled.
///
/// `None` where the form has no shear table, and in [`ultimate_bearing_capacity_on_slope`]'s cases:
/// a bad width or gravity, a negative or NaN slope, a slope at or above the layer's φ, and a
/// pressure that is not positive and finite. It is never 0. Every cause is `None` alike, since the
/// terrain readout shows each the same way, as the em dash of a figure not computed (plan R10,
/// T12's Draft 1). A slope of 0 is level ground, and gives [`bearing_capacity`]'s figure bit for
/// bit. The capacity never grows with the slope.
///
/// # The slope's basis
///
/// This is the contract for a caller; the function takes the slope it is given.
/// - `slope` is the ground's adirectional slope at the point over a fixed 2 m baseline: the
///   gradient's magnitude from central differences 2 m apart along the local east and north axes
///   (plan R10, Design note 7).
/// - It is never the baseline a display's selector shows, so that a display control never changes
///   what the capacity is taken on.
/// - The capacity's dominant term is the load's inclination to the pad's base, which the ground
///   under the pad sets. 2 m is the finest scale the terrain resolves, its band limit, about two
///   widths of the 1.0 m reference pad; a wider basis would smooth away the pad's seat. It is also
///   the finest of the "2-5 m length scale" at which `InSight`'s site selection characterised
///   slopes, against a 15° limit set because "the seismometer cannot level itself on steeper
///   slopes" (Golombek et al. 2014, LPSC 45, abstract 1499, p. 1).
/// - Where the point is unresolved, it is the RMS slope at that baseline, √(|g₀|² + 2v) over the
///   resolved slope g₀ and the unresolved bands' variance v (Design note 7). The capacity falls
///   with the slope, so that upper estimate errs low, the safe direction.
#[must_use]
pub fn bearing_capacity_on_slope(
    p: &MaterialProperties,
    gravity: MetresPerSecondSquared,
    footing_width: Metres,
    slope: Radians,
) -> Option<Pascals> {
    let layer = p.shear_layer_at(footing_width)?;
    ultimate_bearing_capacity_on_slope(
        layer.strength(),
        p.bulk_density(layer),
        gravity,
        footing_width,
        slope,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{MaterialPalette, MechanicsFamily, PaletteEntry, PaletteRole};
    use crate::material::{Material, MaterialClass, UNASSIGNED_PHASE_ROW, properties};
    use crate::substance_key::SubstanceKey;
    use hyperion_base::units::{Kelvin, Radians};
    use hyperion_testkit::float::assert_same_bits;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// The Moon's surface gravity, 1.62 m s⁻², as the Lunar Sourcebook takes it (§9.1.9, p. 517).
    const MOON_G: MetresPerSecondSquared = MetresPerSecondSquared::new(1.62);

    /// Level ground.
    const LEVEL: Radians = Radians::ZERO;

    /// A lunar soil's properties: the regolith of an entry of the Sourcebook's recommended specific
    /// gravity, 3.1, so that each layer's bulk density is the Sourcebook's own.
    fn lunar_regolith() -> MaterialProperties {
        let palette = MaterialPalette::new(vec![PaletteEntry {
            substance: SubstanceKey::new("feldspathic").unwrap(),
            role: PaletteRole::PrimaryCrust,
            normal_albedo_bvr: [0.12; 3],
            phase_row: UNASSIGNED_PHASE_ROW,
            grain_density_kg_m3: 3_100.0,
            transition: Kelvin::new(1_500.0),
            mechanics: MechanicsFamily::Silicate,
            liquid: None,
        }])
        .unwrap();
        properties(Material::new(MaterialClass::Regolith, 0).unwrap(), &palette).unwrap()
    }

    fn kpa(q: Pascals) -> f64 {
        q.value() / 1_000.0
    }

    /// A 1 m square footing on lunar regolith at 1.62 m s⁻² takes the 30–60 cm layer (c 3.0 kPa,
    /// φ 54°, ρ 1,740 kg m⁻³) and lies inside the Sourcebook's 3,000–11,000 kPa and within ±50% of
    /// its 6,000 kPa; the formula on the 0–15 cm pair (c 0.52 kPa, φ 42°, ρ 1,500 kg m⁻³) gives
    /// 150–300 kPa (Design note 2). Both are pinned to 1% at the values the cited factors give,
    /// 5,368.6 and 206.5 kPa (computed apart from the code, in Python's `math`), since the
    /// Sourcebook's band is too wide to catch a factor error.
    #[test]
    fn material_bearing_capacity_of_lunar_regolith_matches_the_sourcebook() {
        let soil = lunar_regolith();
        let deep = soil.shear_layer_at(Metres::new(1.0)).unwrap();
        assert!((deep.top().value() - 0.30).abs() < 1e-12);
        assert!((deep.bottom().value() - 0.60).abs() < 1e-12);
        assert!((soil.bulk_density(deep).value() - 1_740.0).abs() < 1e-9);
        let q = kpa(bearing_capacity(&soil, MOON_G, Metres::new(1.0)).unwrap());
        assert!((3_000.0..=11_000.0).contains(&q), "{q} kPa");
        assert!((q - 6_000.0).abs() <= 3_000.0, "{q} kPa");
        assert!((q - 5_368.55).abs() <= 0.01 * 5_368.55, "{q} kPa");

        let shallow = soil.shear_layer_at(Metres::new(0.1)).unwrap();
        assert!((shallow.bottom().value() - 0.15).abs() < 1e-12);
        assert!((soil.bulk_density(shallow).value() - 1_500.0).abs() < 1e-9);
        let q = kpa(ultimate_bearing_capacity(
            shallow.strength(),
            soil.bulk_density(shallow),
            MOON_G,
            Metres::new(1.0),
        ));
        assert!((150.0..=300.0).contains(&q), "{q} kPa");
        assert!((q - 206.51).abs() <= 0.01 * 206.51, "{q} kPa");
    }

    /// The factors at 42° and 54° are Vesić's (`N_c` 93.71 and 518.8, `N_γ` 155.5 and 1,971; the
    /// research's "`N_c` ≈ 94, `N_γ` ≈ 156" at 42°, Design note 2), and φ = 0 is Prandtl's limit.
    #[test]
    fn material_bearing_factors_are_vesic_s() {
        let at = |deg: f64| {
            BearingFactors::of(MohrCoulomb::new(
                Pascals::ZERO,
                Radians::new(deg.to_radians()),
            ))
        };
        let f = at(42.0);
        assert!((f.n_q() - 85.3736).abs() < 1e-3, "{f:?}");
        assert!((f.n_c() - 93.7064).abs() < 1e-3, "{f:?}");
        assert!((f.n_gamma() - 155.5423).abs() < 1e-3, "{f:?}");
        let f = at(54.0);
        assert!((f.n_c() - 518.805).abs() < 1e-2, "{f:?}");
        assert!((f.n_gamma() - 1_971.18).abs() < 1e-1, "{f:?}");
        let f = at(0.0);
        assert!((f.n_c() - (core::f64::consts::PI + 2.0)).abs() < 1e-15);
        assert!((f.n_q() - 1.0).abs() < 1e-15);
        assert!(f.n_gamma().abs() < 1e-15);
        // Continuous into the limit.
        let f = at(1e-6);
        assert!(
            (f.n_c() - (core::f64::consts::PI + 2.0)).abs() < 1e-4,
            "{f:?}"
        );
    }

    /// The capacity grows with the footing's width, within a layer and as the width reaches deeper
    /// layers, and with gravity (Design note 2).
    #[test]
    fn material_bearing_capacity_grows_with_width_and_gravity() {
        let soil = lunar_regolith();
        let mut last = 0.0;
        for i in 1..=200 {
            let width = Metres::new(0.01 * f64::from(i));
            let q = bearing_capacity(&soil, MOON_G, width).unwrap().value();
            assert!(q > last, "{q} Pa at {width:?} after {last} Pa");
            last = q;
        }
        let width = Metres::new(1.0);
        let mut last = 0.0;
        for g in [0.1, 0.38, 1.62, 3.7, 9.8, 25.0] {
            let q = bearing_capacity(&soil, MetresPerSecondSquared::new(g), width)
                .unwrap()
                .value();
            assert!(q > last, "{q} Pa at {g} m/s² after {last} Pa");
            last = q;
        }
    }

    /// No shear table, no capacity; nor for a negative or non-finite width or gravity.
    #[test]
    fn material_bearing_capacity_is_none_without_data_or_valid_inputs() {
        let soil = lunar_regolith();
        let none = |p: &MaterialProperties, g: f64, width: f64| {
            bearing_capacity(p, MetresPerSecondSquared::new(g), Metres::new(width)).is_none()
        };
        assert!(none(&soil, 1.62, -0.1));
        assert!(none(&soil, 1.62, f64::NAN));
        assert!(none(&soil, f64::INFINITY, 1.0));
        assert!(none(&soil, -1.0, 1.0));
        let palette = MaterialPalette::new(vec![PaletteEntry {
            substance: SubstanceKey::new("basalt").unwrap(),
            role: PaletteRole::SecondaryCrust,
            normal_albedo_bvr: [0.1; 3],
            phase_row: UNASSIGNED_PHASE_ROW,
            grain_density_kg_m3: 3_010.0,
            transition: Kelvin::new(1_253.15),
            mechanics: MechanicsFamily::Silicate,
            liquid: None,
        }])
        .unwrap();
        let bedrock =
            properties(Material::new(MaterialClass::Bedrock, 0).unwrap(), &palette).unwrap();
        assert!(bedrock.shear().is_empty());
        assert!(none(&bedrock, 1.62, 1.0));
        // A footing of zero width carries the cohesion term alone.
        let q = bearing_capacity(&soil, MOON_G, Metres::ZERO).unwrap();
        assert!(q.value() > 0.0);
    }

    /// T1.a's level formula as it stood at `89c27081`, the same arithmetic in the same order, to
    /// hold level ground to it bit for bit.
    fn level_capacity_before_the_slope(
        strength: MohrCoulomb,
        bulk_density: KilogramsPerCubicMetre,
        gravity: MetresPerSecondSquared,
        width: Metres,
    ) -> f64 {
        let f = BearingFactors::of(strength);
        let s_c = 1.0 + f.n_q() / f.n_c();
        let cohesion = strength.cohesion().value() * f.n_c() * s_c;
        let weight = 0.5
            * bulk_density.value()
            * gravity.value()
            * width.value()
            * f.n_gamma()
            * SQUARE_SELF_WEIGHT_SHAPE;
        cohesion + weight
    }

    /// Mars's surface gravity, 3.71 m s⁻², as plan R10's T12 drafts take it for the Viking soils.
    const MARS_G: MetresPerSecondSquared = MetresPerSecondSquared::new(3.71);

    /// The grain density Moore et al. assume for the Viking soils, 2,600 kg m⁻³ (USGS Professional
    /// Paper 1389, p. 121, as `properties.rs` takes it), so that their bulk densities are Moore et
    /// al.'s 1,200 (the drift) and 1,400 kg m⁻³ (Lander 2's soil).
    const VIKING_GRAIN_DENSITY: f64 = 2_600.0;

    /// A silicate entry's form `class`, at the grain density `grain_density`, kg m⁻³.
    fn soil_of(class: MaterialClass, grain_density: f64) -> MaterialProperties {
        let palette = MaterialPalette::new(vec![PaletteEntry {
            substance: SubstanceKey::new("feldspathic").unwrap(),
            role: PaletteRole::PrimaryCrust,
            normal_albedo_bvr: [0.12; 3],
            phase_row: UNASSIGNED_PHASE_ROW,
            grain_density_kg_m3: grain_density,
            transition: Kelvin::new(1_500.0),
            mechanics: MechanicsFamily::Silicate,
            liquid: None,
        }])
        .unwrap();
        properties(Material::new(class, 0).unwrap(), &palette).unwrap()
    }

    fn strength(c_kpa: f64, phi_deg: f64) -> MohrCoulomb {
        MohrCoulomb::new(
            Pascals::new(c_kpa * 1_000.0),
            Radians::new(phi_deg.to_radians()),
        )
    }

    fn degrees(deg: f64) -> Radians {
        Radians::new(deg.to_radians())
    }

    /// The flush pad's pressure, Pa, and its x, on a layer of strength `s` and bulk density `rho`
    /// under a pad of width `width` at gravity `g`, at a slope of `deg` degrees above 0 and below
    /// φ.
    fn flush(
        s: MohrCoulomb,
        rho: f64,
        g: MetresPerSecondSquared,
        width: f64,
        deg: f64,
    ) -> (f64, f64) {
        let factors = BearingFactors::of(s);
        let rho = KilogramsPerCubicMetre::new(rho);
        let (cohesion, weight) = level_terms(factors, s, rho, g, Metres::new(width));
        let (cohesion, weight) = (Pascals::new(cohesion), Pascals::new(weight));
        let (pressure, x) = flush_pad(factors, s, degrees(deg), cohesion, weight);
        (pressure.value(), x)
    }

    /// `actual` and `expected` are the same bits, or the test fails naming `what`.
    fn same_bits(actual: f64, expected: f64, what: &str) {
        assert!(
            actual.total_cmp(&expected).is_eq(),
            "{what}: {actual:e} against {expected:e}"
        );
    }

    fn within(actual: f64, expected: f64, relative: f64) -> bool {
        (actual - expected).abs() <= relative * expected.abs()
    }

    /// On level ground, a slope of 0 of either sign, the capacity on a slope is T1.a's level
    /// capacity bit for bit, with no solve, and that is the formula as it stood: on every layer of
    /// the lunar soil across the widths that reach them, through both the level functions and the
    /// slope ones. The pins of
    /// [`material_bearing_capacity_of_lunar_regolith_matches_the_sourcebook`] hold to 0.01 kPa:
    /// 5,368.55 kPa under 1 m (5,368.547 from the formula) and, from the top 15 cm's pair under
    /// 1 m, 206.51 kPa (206.512). A frictionless soil keeps its level figure, bit for bit, at 0,
    /// and has none on a slope.
    #[test]
    fn material_bearing_capacity_on_level_ground_is_t1a_s_bit_for_bit() {
        let soil = lunar_regolith();
        for i in 0..=150 {
            let width = Metres::new(0.01 * f64::from(i));
            let layer = soil.shear_layer_at(width).unwrap();
            let (strength, rho) = (layer.strength(), soil.bulk_density(layer));
            let level = level_capacity_before_the_slope(strength, rho, MOON_G, width);
            let q = bearing_capacity(&soil, MOON_G, width).unwrap().value();
            same_bits(q, level, &format!("bearing_capacity at {width:?}"));
            let q = ultimate_bearing_capacity(strength, rho, MOON_G, width).value();
            same_bits(q, level, &format!("ultimate_bearing_capacity at {width:?}"));
            for zero in [LEVEL, Radians::new(-0.0)] {
                let what = format!("at {width:?} and a slope of {zero:?}");
                let q = bearing_capacity_on_slope(&soil, MOON_G, width, zero)
                    .unwrap_or_else(|| panic!("None {what}"))
                    .value();
                same_bits(q, level, &what);
                let q = ultimate_bearing_capacity_on_slope(strength, rho, MOON_G, width, zero)
                    .unwrap_or_else(|| panic!("None {what}"))
                    .value();
                same_bits(q, level, &what);
            }
        }
        let q = kpa(bearing_capacity_on_slope(&soil, MOON_G, Metres::new(1.0), LEVEL).unwrap());
        assert!((q - 5_368.55).abs() < 0.01, "{q} kPa");
        let shallow = soil.shear_layer_at(Metres::new(0.1)).unwrap();
        let q = kpa(ultimate_bearing_capacity_on_slope(
            shallow.strength(),
            soil.bulk_density(shallow),
            MOON_G,
            Metres::new(1.0),
            LEVEL,
        )
        .unwrap());
        assert!((q - 206.51).abs() < 0.01, "{q} kPa");
        let clay_rho = KilogramsPerCubicMetre::new(1_800.0);
        let clay = |deg: f64| {
            ultimate_bearing_capacity_on_slope(
                strength(10.0, 0.0),
                clay_rho,
                MOON_G,
                Metres::new(1.0),
                degrees(deg),
            )
        };
        let level =
            ultimate_bearing_capacity(strength(10.0, 0.0), clay_rho, MOON_G, Metres::new(1.0));
        assert_same_bits(clay(0.0).unwrap().value(), level.value());
        assert_eq!(clay(0.5), None);
    }

    /// A slope's capacity is `None`, never 0: on a form with no shear table (bedrock), and on a
    /// cohesionless soil with no weight term (a pad of no width), on level ground as on a slope.
    #[test]
    fn material_bearing_capacity_on_slope_is_never_zero() {
        let palette = MaterialPalette::new(vec![PaletteEntry {
            substance: SubstanceKey::new("basalt").unwrap(),
            role: PaletteRole::SecondaryCrust,
            normal_albedo_bvr: [0.1; 3],
            phase_row: UNASSIGNED_PHASE_ROW,
            grain_density_kg_m3: 3_010.0,
            transition: Kelvin::new(1_253.15),
            mechanics: MechanicsFamily::Silicate,
            liquid: None,
        }])
        .unwrap();
        let bedrock =
            properties(Material::new(MaterialClass::Bedrock, 0).unwrap(), &palette).unwrap();
        for deg in [0.0, 10.0] {
            let q = bearing_capacity_on_slope(&bedrock, MOON_G, Metres::new(1.0), degrees(deg));
            assert_eq!(q, None, "bedrock at {deg}°");
            let q = ultimate_bearing_capacity_on_slope(
                strength(0.0, 30.0),
                KilogramsPerCubicMetre::new(1_600.0),
                MOON_G,
                Metres::ZERO,
                degrees(deg),
            );
            assert_eq!(q, None, "a cohesionless soil under no width at {deg}°");
        }
    }

    /// The ruled values on the Moon, at 1.62 m s⁻² under a 1 m pad, pinned to 0.1% (each computed
    /// apart from the code, in Python's `math`, and the ruling's own):
    /// - lunar regolith at 10°: 3,730.03 kPa, from x 0.176224, `i_c` 0.747323, `i_γ` 0.615918,
    ///   `z_γ` 0.953151 and cos β 0.984808, a cohesion term of 2,766.32 kPa and a weight term of
    ///   963.71 kPa;
    /// - at 20°: 2,331.04 kPa;
    /// - the top 15 cm's pair under 1 m at 10°: 133.285 kPa.
    #[test]
    fn material_bearing_capacity_on_a_lunar_slope_matches_the_ruled_values() {
        let soil = lunar_regolith();
        let q = |deg: f64| {
            kpa(bearing_capacity_on_slope(&soil, MOON_G, Metres::new(1.0), degrees(deg)).unwrap())
        };
        assert!(within(q(10.0), 3_730.03, 1e-3), "{} kPa", q(10.0));
        assert!(within(q(20.0), 2_331.04, 1e-3), "{} kPa", q(20.0));

        let deep = soil.shear_layer_at(Metres::new(1.0)).unwrap();
        let rho = soil.bulk_density(deep).value();
        let (pressure, x) = flush(deep.strength(), rho, MOON_G, 1.0, 10.0);
        assert!(within(pressure / 1_000.0, 3_730.03, 1e-3), "{pressure} Pa");
        assert!(within(x, 0.176_224, 1e-5), "{x}");
        let corresponding = BearingFactors::of(deep.strength()).n_q() - 1.0;
        let (_, i_c, i_gamma) = inclination_factors(x, SQUARE_INCLINATION_EXPONENT, corresponding);
        assert!(within(i_c, 0.747_323, 1e-5), "{i_c}");
        assert!(within(i_gamma, 0.615_918, 1e-5), "{i_gamma}");
        let ten = 10.0_f64.to_radians();
        let z = soil_inertia_factor(math::tan(ten), math::tan(54.0_f64.to_radians()));
        assert!(within(z, 0.953_151, 1e-5), "{z}");
        assert!(within(math::cos(ten), 0.984_808, 1e-6));
        let (cohesion, weight) = level_terms(
            BearingFactors::of(deep.strength()),
            deep.strength(),
            soil.bulk_density(deep),
            MOON_G,
            Metres::new(1.0),
        );
        assert!(
            within(cohesion * i_c / 1_000.0, 2_766.32, 1e-3),
            "{cohesion}"
        );
        let weight = weight * math::cos(ten) * z * i_gamma;
        assert!(within(weight / 1_000.0, 963.71, 1e-3), "{weight}");

        let shallow = soil.shear_layer_at(Metres::new(0.1)).unwrap();
        let q = kpa(ultimate_bearing_capacity_on_slope(
            shallow.strength(),
            soil.bulk_density(shallow),
            MOON_G,
            Metres::new(1.0),
            degrees(10.0),
        )
        .unwrap());
        assert!(within(q, 133.285, 1e-3), "{q} kPa");
    }

    /// The ruled values on Mars, at 3.71 m s⁻² under a 1 m pad at 10°, pinned to 0.1%: Viking
    /// Lander 2's soil bears 209.916 kPa and the drift 27.8416 kPa (Python's `math`, and the
    /// ruling's). The drift's figure pins the solve: with x = tan β, leaving out c cot φ, it would
    /// be 25.88 kPa, 7% lower.
    #[test]
    fn material_bearing_capacity_on_a_martian_slope_matches_the_ruled_values() {
        let q = |class: MaterialClass| {
            let soil = soil_of(class, VIKING_GRAIN_DENSITY);
            kpa(bearing_capacity_on_slope(&soil, MARS_G, Metres::new(1.0), degrees(10.0)).unwrap())
        };
        let lander_2 = q(MaterialClass::Soil);
        assert!(within(lander_2, 209.916, 1e-3), "{lander_2} kPa");
        let drift = q(MaterialClass::Dust);
        assert!(within(drift, 27.8416, 1e-3), "{drift} kPa");
        assert!(!within(drift, 25.88, 0.01), "{drift} kPa");
    }

    /// On a cohesionless soil x is tan β, and q(β) ÷ q(0) = cos β (1 − tan β)^2.5 (1 − tan β ÷
    /// tan φ)^0.35: at φ 30°, 0.53376 at 10° and 0.21398 at 20° (the ruling's, and Python's
    /// `math`).
    #[test]
    fn material_bearing_capacity_on_a_cohesionless_slope_is_the_closed_form() {
        let sand = strength(0.0, 30.0);
        let q = |deg: f64| {
            ultimate_bearing_capacity_on_slope(
                sand,
                KilogramsPerCubicMetre::new(1_600.0),
                MetresPerSecondSquared::new(9.81),
                Metres::new(1.0),
                degrees(deg),
            )
            .unwrap()
            .value()
        };
        for (deg, ruled) in [(10.0_f64, 0.533_76), (20.0, 0.213_98)] {
            let (tan_beta, beta) = (math::tan(deg.to_radians()), deg.to_radians());
            let closed = math::cos(beta)
                * math::powf(1.0 - tan_beta, 2.5)
                * math::powf(1.0 - tan_beta / math::tan(30.0_f64.to_radians()), 0.35);
            let ratio = q(deg) / q(0.0);
            assert!(
                within(ratio, closed, 1e-12),
                "{ratio} against {closed} at {deg}°"
            );
            assert!(within(ratio, ruled, 1e-4), "{ratio} at {deg}°");
        }
    }

    /// At zero gravity the weight term is gone and the flush pad's capacity is the level footing's
    /// cohesion term under the inclined load alone, the identity that tells this reading from the
    /// ground-slope and base-tilt ones: on the lunar 30–60 cm layer at 10°, 2,766.50 kPa = c `N_c`
    /// `s_c`, 3,701.64 kPa, times `i_c` 0.747372, at x 0.176188 (the ruling's, and Python's
    /// `math`).
    #[test]
    fn material_bearing_capacity_at_zero_gravity_is_the_inclined_cohesion_term() {
        let deep = strength(3.0, 54.0);
        let (pressure, x) = flush(deep, 1_740.0, MetresPerSecondSquared::ZERO, 1.0, 10.0);
        assert!(within(pressure / 1_000.0, 2_766.50, 1e-3), "{pressure} Pa");
        assert!(within(x, 0.176_188, 1e-5), "{x}");
        let factors = BearingFactors::of(deep);
        let cohesion = 3_000.0 * factors.n_c() * (1.0 + factors.n_q() / factors.n_c());
        assert!(within(cohesion / 1_000.0, 3_701.64, 1e-5), "{cohesion}");
        let (_, i_c, _) = inclination_factors(x, SQUARE_INCLINATION_EXPONENT, factors.n_q() - 1.0);
        assert!(within(i_c, 0.747_372, 1e-5), "{i_c}");
        assert!(within(pressure, cohesion * i_c, 1e-9), "{pressure} Pa");
    }

    /// Vesić's inclined-load Example 3.3(b): at P ÷ Q 0.21 and m 1.75, `ζ_qi` = (1 − 0.21)^1.75 =
    /// 0.66, `ζ_ci` = 0.66 − (1 − 0.66) ÷ [(18.05)(0.42)] = 0.62 and `ζ_γi` = (1 − 0.21)^2.75 =
    /// 0.52 (Vesić 1975, p. 131).
    #[test]
    fn material_inclination_factors_match_vesic_s_example() {
        let (i_q, i_c, i_gamma) = inclination_factors(0.21, 1.75, 18.05 * 0.42);
        assert!((i_q - 0.66).abs() < 0.005, "{i_q}");
        assert!((i_c - 0.62).abs() < 0.005, "{i_c}");
        assert!((i_gamma - 0.52).abs() < 0.005, "{i_gamma}");
        // Level: every factor 1.
        let (i_q, i_c, i_gamma) = inclination_factors(0.0, 1.5, 714.0);
        for i in [i_q, i_c, i_gamma] {
            assert_same_bits(i, 1.0);
        }
    }

    /// The capacity never grows with the slope, on a grid of 20,000 steps from level to just under
    /// φ, on every layer of every shear table at its body's gravity under a 1 m pad (the lunar
    /// soil's four at 1.62 m s⁻², the drift's and Viking Lander 2's soil's at 3.71 m s⁻²) and on a
    /// cohesionless soil of φ 30°, and it stays positive.
    #[test]
    fn material_bearing_capacity_never_increases_with_slope() {
        let mut cases: Vec<(MohrCoulomb, f64, MetresPerSecondSquared)> = Vec::new();
        for (soil, g) in [
            (lunar_regolith(), MOON_G),
            (soil_of(MaterialClass::Dust, VIKING_GRAIN_DENSITY), MARS_G),
            (soil_of(MaterialClass::Soil, VIKING_GRAIN_DENSITY), MARS_G),
        ] {
            for layer in soil.shear() {
                cases.push((layer.strength(), soil.bulk_density(*layer).value(), g));
            }
        }
        assert_eq!(cases.len(), 6);
        cases.push((
            strength(0.0, 30.0),
            1_600.0,
            MetresPerSecondSquared::new(9.81),
        ));
        for (s, rho, g) in cases {
            let phi = s.friction_angle().value();
            let top = phi * (1.0 - 1e-9);
            let mut last = f64::INFINITY;
            for i in 0..=20_000 {
                let beta = Radians::new(top * f64::from(i) / 20_000.0);
                let q = ultimate_bearing_capacity_on_slope(
                    s,
                    KilogramsPerCubicMetre::new(rho),
                    g,
                    Metres::new(1.0),
                    beta,
                )
                .unwrap_or_else(|| panic!("None at {beta:?}, {s:?}"))
                .value();
                assert!(
                    q > 0.0 && q <= last,
                    "{q} Pa at {beta:?} after {last} Pa, {s:?}"
                );
                last = q;
            }
        }
    }

    /// Toward φ a cohesionless capacity falls to 0, and a cohesive one to its cohesion term alone:
    /// at φ − 10⁻⁹ rad under a 1 m pad, 0.488117 of the level figure on the drift and 0.144005 on
    /// Viking Lander 2's soil at 3.71 m s⁻², and 0.0290892 on the lunar 0–15 cm pair and 0.00100339
    /// on the 30–60 cm layer at 1.62 m s⁻² (the ruling's, and Python's `math`). At φ, just above it
    /// and at 89° it is `None`, never 0; and a cohesionless soil of φ 50° has none from 45°.
    #[test]
    fn material_bearing_capacity_reaches_its_limit_as_the_slope_nears_phi() {
        let near = |s: MohrCoulomb, rho: f64, g: MetresPerSecondSquared| {
            let at = |beta: f64| {
                ultimate_bearing_capacity_on_slope(
                    s,
                    KilogramsPerCubicMetre::new(rho),
                    g,
                    Metres::new(1.0),
                    Radians::new(beta),
                )
            };
            let phi = s.friction_angle().value();
            for beyond in [phi, phi + 1e-9, 89.0_f64.to_radians()] {
                assert_eq!(at(beyond), None, "{beyond} rad, {s:?}");
            }
            at(phi - 1e-9).unwrap().value() / at(0.0).unwrap().value()
        };
        let sand = near(
            strength(0.0, 30.0),
            1_600.0,
            MetresPerSecondSquared::new(9.81),
        );
        assert!(sand > 0.0 && sand < 1e-3, "{sand}");
        let mars = |class: MaterialClass| {
            let soil = soil_of(class, VIKING_GRAIN_DENSITY);
            let layer = soil.shear_layer_at(Metres::new(1.0)).unwrap();
            near(layer.strength(), soil.bulk_density(layer).value(), MARS_G)
        };
        let drift = mars(MaterialClass::Dust);
        assert!(within(drift, 0.488_117, 1e-3), "{drift}");
        let lander_2 = mars(MaterialClass::Soil);
        assert!(within(lander_2, 0.144_005, 1e-3), "{lander_2}");
        let regolith = lunar_regolith();
        let moon = |depth: f64| {
            let layer = regolith.shear_layer_at(Metres::new(depth)).unwrap();
            near(
                layer.strength(),
                regolith.bulk_density(layer).value(),
                MOON_G,
            )
        };
        let top = moon(0.1);
        assert!(within(top, 0.029_089_2, 1e-3), "{top}");
        let deep = moon(1.0);
        assert!(within(deep, 0.001_003_39, 1e-3), "{deep}");

        let steep_sand = |deg: f64| {
            ultimate_bearing_capacity_on_slope(
                strength(0.0, 50.0),
                KilogramsPerCubicMetre::new(1_600.0),
                MetresPerSecondSquared::new(9.81),
                Metres::new(1.0),
                degrees(deg),
            )
        };
        assert!(steep_sand(44.9).is_some_and(|q| q.value() > 0.0));
        assert_eq!(steep_sand(45.0), None);
        assert_eq!(steep_sand(46.0), None);
        // 45° is refused by the angle, since `math::tan` puts tan 45° a rounding below 1.
        assert!(math::tan(core::f64::consts::FRAC_PI_4) < 1.0);
        // A cohesive soil steeper than 45° keeps a capacity past 45°, below φ.
        let steep_clay = ultimate_bearing_capacity_on_slope(
            strength(3.0, 54.0),
            KilogramsPerCubicMetre::new(1_740.0),
            MOON_G,
            Metres::new(1.0),
            degrees(50.0),
        );
        assert!(
            steep_clay.is_some_and(|q| q.value() > 0.0),
            "{steep_clay:?}"
        );
    }

    /// The slope is refused where it is negative or NaN, as are a bulk density, gravity or width
    /// that is negative or not finite.
    #[test]
    fn material_bearing_capacity_on_slope_is_none_outside_its_inputs() {
        let soil = lunar_regolith();
        for beta in [
            -1e-9,
            -0.1,
            f64::NAN,
            -f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            assert_eq!(
                bearing_capacity_on_slope(&soil, MOON_G, Metres::new(1.0), Radians::new(beta)),
                None,
                "{beta}"
            );
        }
        let deep = soil.shear_layer_at(Metres::new(1.0)).unwrap();
        let ultimate = |rho: f64, g: f64, width: f64| {
            ultimate_bearing_capacity_on_slope(
                deep.strength(),
                KilogramsPerCubicMetre::new(rho),
                MetresPerSecondSquared::new(g),
                Metres::new(width),
                degrees(10.0),
            )
        };
        assert!(ultimate(1_740.0, 1.62, 1.0).is_some());
        for (rho, g, width) in [
            (f64::NAN, 1.62, 1.0),
            (-1.0, 1.62, 1.0),
            (1_740.0, f64::INFINITY, 1.0),
            (1_740.0, -1.62, 1.0),
            (1_740.0, 1.62, f64::NAN),
            (1_740.0, 1.62, -1.0),
        ] {
            assert_eq!(ultimate(rho, g, width), None, "{rho}, {g}, {width}");
        }
        assert_eq!(
            bearing_capacity_on_slope(&soil, MOON_G, Metres::new(-1.0), degrees(10.0)),
            None
        );
    }

    /// The pinned cases of [`material_bearing_capacity_on_slope_keeps_its_bits`], by name.
    fn pinned_cases() -> Vec<(&'static str, f64)> {
        let regolith = lunar_regolith();
        let moon = |deg: f64| {
            bearing_capacity_on_slope(&regolith, MOON_G, Metres::new(1.0), degrees(deg))
                .unwrap()
                .value()
        };
        let near_phi = bearing_capacity_on_slope(
            &regolith,
            MOON_G,
            Metres::new(1.0),
            Radians::new(54.0_f64.to_radians() - 1e-9),
        )
        .unwrap()
        .value();
        let mars = |class: MaterialClass| {
            let soil = soil_of(class, VIKING_GRAIN_DENSITY);
            bearing_capacity_on_slope(&soil, MARS_G, Metres::new(1.0), degrees(10.0))
                .unwrap()
                .value()
        };
        let weightless = ultimate_bearing_capacity_on_slope(
            strength(3.0, 54.0),
            KilogramsPerCubicMetre::new(1_740.0),
            MetresPerSecondSquared::ZERO,
            Metres::new(1.0),
            degrees(10.0),
        )
        .unwrap()
        .value();
        let sand = ultimate_bearing_capacity_on_slope(
            strength(0.0, 30.0),
            KilogramsPerCubicMetre::new(1_600.0),
            MetresPerSecondSquared::new(9.81),
            Metres::new(1.0),
            degrees(10.0),
        )
        .unwrap()
        .value();
        vec![
            ("lunar 1 m at 10°", moon(10.0)),
            ("lunar 1 m at 20°", moon(20.0)),
            ("lunar 1 m just under φ", near_phi),
            ("Lander 2's soil at 10°", mars(MaterialClass::Soil)),
            ("Mars's drift at 10°", mars(MaterialClass::Dust)),
            ("lunar 30–60 cm at zero gravity, 10°", weightless),
            ("cohesionless φ 30° at 10°", sand),
        ]
    }

    /// The slope's capacity keeps its bits on every target: the solve's fixed 64 halvings over
    /// `math`'s pinned `libm` give the same `f64` natively and on both WebAssembly targets, where
    /// this test also runs. The bits were taken natively when the solve was built (2026-10-14),
    /// and read 3,730,032.64, 2,331,035.44, 5,386.74, 209,915.94, 27,841.61, 2,766,499.18 and
    /// 56,305.47 Pa.
    #[test]
    fn material_bearing_capacity_on_slope_keeps_its_bits() {
        let bits: [u64; 7] = [
            0x414c_7538_517b_58ac,
            0x4141_c8cd_b808_5120,
            0x40b5_0abe_9d7d_0eee,
            0x4109_9fdf_86da_0070,
            0x40db_3067_26e6_817c,
            0x4145_1b51_96ff_4b0e,
            0x40eb_7e2f_18a4_ce16,
        ];
        let cases = pinned_cases();
        assert_eq!(cases.len(), bits.len());
        for ((name, q), bits) in cases.into_iter().zip(bits) {
            same_bits(q, f64::from_bits(bits), name);
        }
    }
}
