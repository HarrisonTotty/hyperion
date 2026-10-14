//! The ultimate bearing capacity of the ground under a footing: a function of the footing's width,
//! the body's gravity and the ground's slope, not a constant of the ground (plan R10, T1.a, Design
//! note 2).
//!
//! # The formula
//!
//! For a square or circular footing of width B resting on the surface, with no surcharge beside it,
//! its base level, the ground beside it falling away at a slope β (radians, 0 on level ground),
//!
//! ```text
//! q_ult = c N_c s_c g_c + ½ ρ g B N_γ s_γ g_γ
//! N_q = exp(π tan φ) tan²(45° + φ/2)    N_c = (N_q − 1) cot φ    N_γ = 2 (N_q + 1) tan φ
//! s_c = 1 + N_q ÷ N_c                    s_γ = 0.6                (B = L)
//! g_γ = (1 − tan β)²                     g_c = g_γ − (1 − g_γ) ÷ (N_c tan φ), at least 0
//! for 0 ≤ β < φ and β < 45°; g_c = g_γ = 1 on level ground
//! ```
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
//! On a slope the capacity takes Vesić's ground-slope factors, for a footing whose base is level
//! and beside which the ground falls away at ω, under a vertical load: `ζ_qg` = `ζ_γg` =
//! (1 − tan ω)², his Eq. 3.21, and for the cohesion term `ζ_cg` = `ζ_qg` − (1 − `ζ_qg`) ÷ (`N_c`
//! tan φ), "as before, from expression 3.15" (Vesić 1975, "Bearing capacity of shallow
//! foundations", in Winterkorn and Fang, Foundation Engineering Handbook, ch. 3, §3.7, pp.
//! 131–132, Fig. 3.18; read in a scanned copy). His Eq. 3.15 comes "from the theorem of
//! correspondence" (De Beer and Ladanyi 1961, as he cites them; §3.6, p. 130). Here they are
//! `g_γ` and `g_c`, with ω = β.
//! - _Range._ They hold for ω < 45° and ω < φ (his Eqs. 3.24 and 3.25, p. 132).
//! - _Slope stability._ The analyses behind them leave out the shear stresses already in the
//!   ground, which "may be negligible as long as 0 < ω < φ/2"; past φ/2 he advises a
//!   slope-stability analysis besides, which nothing here models.
//! - _Shape._ They are plane-strain factors, "valid only for long rectangular footings, with main
//!   axis parallel to the slope", and "there are, however, still no experimental data" on how they
//!   vary with the footing's shape (p. 132), so a square pad takes them as they stand.
//! - _Worked example._ His Example 3.6 (p. 132) is a footing on ground sloping 5 to 1 under a
//!   central vertical load. It gives `ζ_qg` = `ζ_γg` 0.64 and `ζ_cg` 0.59 at `N_c` 18.05 and tan φ
//!   0.42, which a test checks.
//! - _EM 1110-1-1905._ EM reproduces the factors (Table 4-6, p. 4-9, "base on slope"; its worked
//!   example §4-5c, p. 4-28, takes `ζ_γβ` = (1 − tan 15°)² = 0.536), but its cohesion row prints
//!   ÷ 147.3 at φ > 0 where Vesić has ÷ (`N_c` tan φ), a misprint: 147.3 is its φ = 0 row's, whose
//!   1 − β ÷ 147.3 with β in degrees is Vesić's 1 − 2ω ÷ (π + 2) (his Eq. 3.22).
//!
//! A lander's pad rests flush on the ground, its base tilted with the slope, and no source read
//! gives that case its factors.
//! - Brinch Hansen resolves a footing's load normal to its base and in it. He gives his tilt and
//!   slope factors "only … for positive values of ν and β, the latter being smaller than φ", a
//!   positive tilt ν being a base rising toward the side the ground falls away to (1970, "A revised
//!   and extended formula for bearing capacity", Danish Geotechnical Institute Bulletin 28, §7,
//!   pp. 7–8, Figs. 8 and 9; read in the scan GEO publishes). Vesić's Fig. 3.18 draws the same.
//! - A flush pad is therefore a negative tilt, −β, under a load inclined to its base at β. That is
//!   outside Hansen's stated range, and a tilt Vesić's analyses do not treat, though his
//!   inclined-load factors (§3.6) cover the load.
//! - EM's worked example of base tilt with ground slope (§4-5c, pp. 4-27 to 4-29, Figure 4-7) takes
//!   a base tilted the same way as the ground as a positive tilt, against both primaries' sign. It
//!   is not followed.
//!
//! So the capacity takes the level base's factors, the case the primaries cover. That leaves out
//! the downslope part of the pad's load. Against the flush pad taken by the sources' factors
//! extrapolated, as normal pressure on its base (plan R10, Risks, "Question for main (T1.a's slope
//! follow-up)"):
//! - on the deeper lunar soil (φ 54°), which a 1 m pad reaches, the two lie within a tenth of each
//!   other up to 20°;
//! - on the top 15 cm's pair (φ 42°) this figure errs high, by a tenth at 10° and by two fifths at
//!   20°;
//! - on a cohesionless soil of φ 30° it errs high by a third at 10°.
//!
//! Hansen's ground factor, the alternative, is `g_q` = `g_γ` = (1 − 0.5 tan β)⁵ (his Eq. 15, p. 8;
//! EM Table 4-5, p. 4-8). He gives `g_c` only at φ = 0, as the additive β ÷ 147° (his Eq. 12,
//! p. 7); EM's Table 4-5 prints a φ > 0 row with the same ÷ 147.3 as its Table 4-6. It is not
//! built, since this module's `N_γ` and shape factors are Vesić's; at 10° it is 0.630 against
//! Vesić's 0.678.
//!
//! Outside the factors' range the capacity is `None`, never 0 ([`SlopeFactors::of`]). A `g_c` that
//! would fall below 0, near the range's end, is 0: the cohesion then carries nothing.
//!
//! On the Moon a 1 m pad on lunar regolith on a 10° slope bears 3,640 kPa, 68% of its level
//! 5,370 kPa.

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

/// Vesić's ground-slope factors, `g_c` and `g_γ`, of a footing with a level base on ground of slope β.
///
/// Each lies in [0, 1], and on level ground each is 1 (the [module](self)'s formula and its
/// sources).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlopeFactors {
    cohesion: f64,
    self_weight: f64,
}

impl SlopeFactors {
    /// Level ground, β = 0: both factors exactly 1, so the capacity is the level footing's bit for
    /// bit.
    pub const LEVEL: Self = Self {
        cohesion: 1.0,
        self_weight: 1.0,
    };

    /// The factors at `strength`'s friction angle φ on ground of slope `slope`, β, radians.
    ///
    /// A slope of 0, of either sign, is level ground ([`Self::LEVEL`]) whatever φ. `None` outside
    /// the factors' range, so that a slope beyond it is never taken for a capacity of 0:
    /// - a slope at or above φ, or at or above 45° (Vesić 1975, Eqs. 3.24 and 3.25, p. 132; for φ,
    ///   Hansen 1970, §7, p. 8; the [module](self)'s sources);
    /// - a slope that is negative (a slope is a magnitude) or NaN;
    /// - so any slope above 0 on a soil of φ at or below 0. Vesić's frictionless forms, `ζ_cg` =
    ///   1 − 2ω ÷ (π + 2) with `N_γ` = −2 sin ω (his Eqs. 3.22 and 3.23), are not built, since no
    ///   form's table has φ = 0.
    ///
    /// A `g_c` that would fall below 0 is 0, from 41.73° at φ 42° and from 43.91° at φ 54°: the
    /// cohesion then carries nothing, and the self-weight term what is left.
    #[must_use]
    pub fn of(strength: MohrCoulomb, slope: Radians) -> Option<Self> {
        Self::from_factors(
            BearingFactors::of(strength),
            strength.friction_angle(),
            slope,
        )
    }

    /// [`Self::of`], from the bearing factors `factors` of the friction angle `friction_angle`.
    #[must_use]
    fn from_factors(
        factors: BearingFactors,
        friction_angle: Radians,
        slope: Radians,
    ) -> Option<Self> {
        let beta = slope.value();
        if beta.is_nan() || beta < 0.0 {
            return None;
        }
        // Zero of either sign is level ground, whatever the friction angle.
        if beta <= 0.0 {
            return Some(Self::LEVEL);
        }
        // 1 − tan β reaches 0 at 45°, where `math::tan` gives 1 − 2⁻⁵³, so the bound is the angle's.
        if beta >= friction_angle.value() || beta >= core::f64::consts::FRAC_PI_4 {
            return None;
        }
        let base = 1.0 - math::tan(beta);
        let self_weight = base * base;
        // N_c tan φ, which is N_q − 1 exactly in the module's factors.
        let corresponding = factors.n_q() - 1.0;
        Some(Self {
            cohesion: at_least_zero(self_weight - (1.0 - self_weight) / corresponding),
            self_weight,
        })
    }

    /// `g_c`, the ground-slope factor of the cohesion term (Vesić's `ζ_cg`).
    #[must_use]
    pub const fn g_c(self) -> f64 {
        self.cohesion
    }

    /// `g_γ`, the ground-slope factor of the self-weight term (Vesić's `ζ_γg`, equal to the surcharge
    /// term's `ζ_qg`, which a surface footing does not have).
    #[must_use]
    pub const fn g_gamma(self) -> f64 {
        self.self_weight
    }
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
/// `bulk_density` under a square or circular footing of width `width` at gravity `gravity`, on
/// ground of slope `slope`, radians.
///
/// The footing's base is level, and the capacity is `q_ult` = c `N_c` `s_c` `g_c` + ½ ρ g B `N_γ`
/// `s_γ` `g_γ` (the [module](self)'s formula). At a slope of 0 it is
/// [`ultimate_bearing_capacity`]'s figure bit for bit. `None` for a bulk density, gravity or width
/// that is negative or not finite, and for a slope outside the factors' range
/// ([`SlopeFactors::of`]), so that a `Some` is always finite. The slope's basis is
/// [`bearing_capacity_on_slope`]'s.
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
    let factors = BearingFactors::of(strength);
    let g = SlopeFactors::from_factors(factors, strength.friction_angle(), slope)?;
    let (cohesion, weight) = level_terms(factors, strength, bulk_density, gravity, width);
    // Each term's level figure times its factor, which is exactly 1 on level ground.
    Some(Pascals::new(cohesion * g.cohesion + weight * g.self_weight))
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
/// `footing_width` at gravity `gravity`, on ground of slope `slope`, radians.
///
/// It is taken from the layer at the depth of one footing width
/// ([`MaterialProperties::shear_layer_at`]) with its bulk density
/// ([`MaterialProperties::bulk_density`]) (Design note 2), by the [module](self)'s formula. That
/// takes the factors of a level base on the slope, the case its sources cover; a lander's pad
/// rests flush on the ground, which no source read gives factors for.
///
/// `None` where the form has no shear table; for a width or gravity that is negative or not
/// finite; and for a slope outside the factors' range ([`SlopeFactors::of`]): at or above the
/// layer's φ or 45°, negative or NaN, never a capacity of 0. Every cause is `None` alike, since
/// the terrain readout shows each the same way, as the em dash of a figure not computed (plan R10,
/// T12's Draft 1). A slope of 0 is level ground, and gives [`bearing_capacity`]'s figure bit for
/// bit. The capacity never grows with the slope.
///
/// # The slope's basis
///
/// This is the contract for a caller; the function takes the slope it is given.
/// - `slope` is the ground's adirectional slope at the point over a fixed 2 m baseline: the
///   gradient's magnitude from central differences 2 m apart along the local east and north axes
///   (plan R10, Design note 7). 2 m is the band limit, a lander pad's scale, and the finest of the
///   "2-5 m length scale" at which `InSight`'s site selection characterised slopes, against a 15°
///   limit set because "the seismometer cannot level itself on steeper slopes" (Golombek et al.
///   2014, LPSC 45, abstract 1499, p. 1).
/// - It is never the baseline a display's selector shows, so that a display control never changes
///   what the capacity is taken on.
/// - Where the point is unresolved, it is the RMS slope at that baseline, √(|g₀|² + 2v) over the
///   resolved slope g₀ and the unresolved bands' variance v (Design note 7), which errs steep, the
///   safe direction.
///
/// The sources take the slope as uniform over the ground the failure reaches, which is larger than
/// 2 m: about H = B tan(45° + φ/2) deep, 3.1 m under a 1 m pad at φ 54°, and (H + D) tan(45° +
/// φ/2) out from the footing, 9.5 m there at the surface (EM 1110-1-1905, §1-3a(2) and (3), Eqs.
/// 1-4 and 1-5, p. 1-10, after Vesić 1973; its "about 3B" is the φ 30° case). On
/// rough ground the RMS slope falls with the baseline (Design note 7's structure function), so a
/// 2 m slope is in the mean at least as steep as the slope over that ground, and the capacity errs
/// low.
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

    /// A silicate entry's form `class`, on the Sourcebook's lunar grain density, 3,100 kg m⁻³.
    fn soil_of(class: MaterialClass) -> MaterialProperties {
        let palette = MaterialPalette::new(vec![PaletteEntry {
            substance: SubstanceKey::new("feldspathic").unwrap(),
            role: PaletteRole::PrimaryCrust,
            normal_albedo_bvr: [0.12; 3],
            phase_row: UNASSIGNED_PHASE_ROW,
            grain_density_kg_m3: 3_100.0,
            transition: Kelvin::new(1_500.0),
            mechanics: MechanicsFamily::Silicate,
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

    /// On level ground, a slope of 0 of either sign, the capacity on a slope is T1.a's level
    /// capacity bit for bit, and that is the formula as it stood: on every layer of the lunar soil
    /// across the widths that reach them, through both the level functions and the slope ones. The
    /// pins of [`material_bearing_capacity_of_lunar_regolith_matches_the_sourcebook`] hold to 0.01
    /// kPa: 5,368.55 kPa under 1 m (5,368.547 from the formula) and, from the top 15 cm's pair
    /// under 1 m, 206.51 kPa (206.512).
    #[test]
    fn material_bearing_capacity_on_level_ground_is_t1a_s_bit_for_bit() {
        let soil = lunar_regolith();
        for i in 0..=150 {
            let width = Metres::new(0.01 * f64::from(i));
            let layer = soil.shear_layer_at(width).unwrap();
            let (strength, rho) = (layer.strength(), soil.bulk_density(layer));
            let level = level_capacity_before_the_slope(strength, rho, MOON_G, width);
            assert_same_bits(
                bearing_capacity(&soil, MOON_G, width).unwrap().value(),
                level,
            );
            assert_same_bits(
                ultimate_bearing_capacity(strength, rho, MOON_G, width).value(),
                level,
            );
            for zero in [LEVEL, Radians::new(-0.0)] {
                assert_eq!(
                    SlopeFactors::of(strength, zero),
                    Some(SlopeFactors::LEVEL),
                    "{zero:?}"
                );
                let q = bearing_capacity_on_slope(&soil, MOON_G, width, zero)
                    .unwrap()
                    .value();
                assert_same_bits(q, level);
                let q = ultimate_bearing_capacity_on_slope(strength, rho, MOON_G, width, zero)
                    .unwrap()
                    .value();
                assert_same_bits(q, level);
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
        // A frictionless soil keeps its level capacity, and has no other.
        let clay = strength(10.0, 0.0);
        assert_eq!(SlopeFactors::of(clay, LEVEL), Some(SlopeFactors::LEVEL));
        assert_eq!(SlopeFactors::of(clay, Radians::new(0.01)), None);
    }

    /// On a 10° slope, a 1 m pad on lunar regolith at 1.62 m s⁻² bears 3,640.6 kPa, 68% of the
    /// level 5,368.5 kPa, and the top 15 cm's pair under 1 m 139.75 kPa, from the factor formulas
    /// at φ 54° and 42° (the [module](super)'s formula; computed apart from the code, in Python's
    /// `math`): `g_γ` = (1 − tan 10°)² = 0.678437 at either φ, and `g_c` 0.677987 at 54° (`N_q` −
    /// 1 = 714.07) and 0.674626 at 42° (84.37). Pinned to 1%, as the brief asks, with the factors to
    /// 1e-6.
    #[test]
    fn material_bearing_capacity_on_a_ten_degree_slope_matches_the_hand_worked_value() {
        let ten = Radians::new(10.0_f64.to_radians());
        let s = SlopeFactors::of(strength(3.0, 54.0), ten).unwrap();
        assert!((s.g_gamma() - 0.678_437).abs() < 1e-6, "{s:?}");
        assert!((s.g_c() - 0.677_987).abs() < 1e-6, "{s:?}");
        let s = SlopeFactors::of(strength(0.52, 42.0), ten).unwrap();
        assert!((s.g_gamma() - 0.678_437).abs() < 1e-6, "{s:?}");
        assert!((s.g_c() - 0.674_626).abs() < 1e-6, "{s:?}");

        let soil = lunar_regolith();
        let q = kpa(bearing_capacity_on_slope(&soil, MOON_G, Metres::new(1.0), ten).unwrap());
        assert!((q - 3_640.555).abs() <= 0.01 * 3_640.555, "{q} kPa");
        let shallow = soil.shear_layer_at(Metres::new(0.1)).unwrap();
        let q = kpa(ultimate_bearing_capacity_on_slope(
            shallow.strength(),
            soil.bulk_density(shallow),
            MOON_G,
            Metres::new(1.0),
            ten,
        )
        .unwrap());
        assert!((q - 139.751).abs() <= 0.01 * 139.751, "{q} kPa");
    }

    /// The sources' worked examples. Vesić's Example 3.6, a footing on ground sloping 5 to 1 under
    /// a central vertical load at `N_c` 18.05 and tan φ 0.42, takes `ζ_qg` = `ζ_γg` = 0.64 and
    /// `ζ_cg` = 0.64 − (1 − 0.64) ÷ (18.05 × 0.42) = 0.59 (Vesić 1975, p. 132); those are φ 23°'s
    /// `N_c` 18.049 and tan φ 0.4245, at which `g_c` is 0.5930. EM 1110-1-1905's example of base
    /// tilt and ground slope takes Vesić's `ζ_γβ` = (1 − tan 15°)² = 0.536, with `N_γ` 12.54 and
    /// `N_q` 11.85 at φ′ 26° (§4-5c, p. 4-28; Table 4-4).
    #[test]
    fn material_slope_factors_match_the_sources_worked_examples() {
        let vesic = strength(0.0, 23.0);
        let f = BearingFactors::of(vesic);
        assert!((f.n_c() - 18.05).abs() < 0.005, "{f:?}");
        let five_to_one = Radians::new(math::atan(0.2));
        let s = SlopeFactors::of(vesic, five_to_one).unwrap();
        assert!((s.g_gamma() - 0.64).abs() < 1e-12, "{s:?}");
        assert!((s.g_c() - 0.59).abs() < 0.005, "{s:?}");
        assert!((s.g_c() - 0.5930).abs() < 1e-4, "{s:?}");

        let em = strength(0.0, 26.0);
        let f = BearingFactors::of(em);
        assert!((f.n_gamma() - 12.54).abs() < 0.005, "{f:?}");
        assert!((f.n_q() - 11.85).abs() < 0.005, "{f:?}");
        let ground = SlopeFactors::of(em, Radians::new(15.0_f64.to_radians()))
            .unwrap()
            .g_gamma();
        assert!((ground - 0.536).abs() < 0.0005, "{ground}");
    }

    /// The capacity never grows with the slope, on a grid of 20,000 steps from level to just under
    /// φ, on every lunar layer a pad's width reaches (φ 42°, 46° and 54°) and on the dust and the
    /// soil of silicate (18° and 35°). It is positive below the first of φ and 45° and `None` from
    /// it: from 45° on the 46° and 54° layers, and nowhere below φ on the others.
    #[test]
    fn material_bearing_capacity_never_increases_with_slope() {
        let regolith = lunar_regolith();
        let cases: [(&MaterialProperties, Metres, f64); 5] = [
            (&regolith, Metres::new(0.1), 42.0),
            (&regolith, Metres::new(0.2), 46.0),
            (&regolith, Metres::new(1.0), 54.0),
            (&soil_of(MaterialClass::Dust), Metres::new(1.0), 18.0),
            (&soil_of(MaterialClass::Soil), Metres::new(1.0), 35.0),
        ];
        for (soil, width, phi_deg) in cases {
            let phi = soil
                .shear_layer_at(width)
                .unwrap()
                .strength()
                .friction_angle()
                .value();
            assert!(
                (phi - phi_deg.to_radians()).abs() < 1e-12,
                "{phi} at {width:?}"
            );
            let top = phi * (1.0 - 1e-9);
            let mut last = f64::INFINITY;
            let mut nones = 0;
            for i in 0..=20_000 {
                let beta = top * f64::from(i) / 20_000.0;
                let q = bearing_capacity_on_slope(soil, MOON_G, width, Radians::new(beta));
                if beta >= core::f64::consts::FRAC_PI_4 {
                    assert_eq!(q, None, "at {beta} rad, φ {phi_deg}°");
                    nones += 1;
                    continue;
                }
                let q = q.unwrap().value();
                assert!(
                    q > 0.0 && q <= last,
                    "{q} Pa at {beta} rad after {last} Pa, φ {phi_deg}°"
                );
                last = q;
            }
            assert_eq!(nones > 0, phi_deg > 45.0, "φ {phi_deg}°");
        }
    }

    /// As β nears φ the capacity reaches the limit of its factors, and at φ it is `None`, never 0,
    /// the sources' ω < φ. On the top 15 cm's pair (φ 42°) under 1 m that limit is 1,124.8 Pa,
    /// 0.54% of the level 206.5 kPa, with `g_c` at 0 and `g_γ` 0.009919. On the deeper lunar soil
    /// (φ 54°) 45° comes first, Vesić's ω < 45°, where `g_γ` reaches 0: the capacity falls to 0
    /// there and is `None` from it. On the dust of the test's silicate entry (φ 18°, its bulk
    /// density 1,431 kg m⁻³ from the grain density 3,100 kg m⁻³) under 1 m at 1.62 m s⁻² it is
    /// 33.77% of the level 36.8 kPa: the factors' limit, which stops short of 0 on a soil of small
    /// φ. All computed apart from the code, in Python's `math`.
    #[test]
    fn material_bearing_capacity_reaches_its_factors_limit_as_the_slope_nears_phi() {
        let regolith = lunar_regolith();
        let deep = Metres::new(1.0);
        let at = |soil: &MaterialProperties, width: Metres, deg: f64| {
            bearing_capacity_on_slope(soil, MOON_G, width, Radians::new(deg.to_radians()))
        };
        let below_45 = Radians::new(core::f64::consts::FRAC_PI_4 * (1.0 - 1e-9));
        let near = bearing_capacity_on_slope(&regolith, MOON_G, deep, below_45)
            .unwrap()
            .value();
        assert!(near > 0.0 && near < 1e-6, "{near} Pa");
        assert!(at(&regolith, deep, 44.99).is_some());
        for deg in [45.0, 45.01, 50.0, 53.99, 54.0, 60.0] {
            assert_eq!(at(&regolith, deep, deg), None, "at {deg}°");
        }
        // 45° is refused by the angle, since `math::tan` puts tan 45° a rounding below 1.
        assert!(math::tan(core::f64::consts::FRAC_PI_4) < 1.0);
        assert_eq!(
            SlopeFactors::of(
                strength(0.0, 46.0),
                Radians::new(core::f64::consts::FRAC_PI_4)
            ),
            None
        );

        let shallow = regolith.shear_layer_at(Metres::new(0.1)).unwrap();
        let phi = shallow.strength().friction_angle();
        let q = |beta: Radians| {
            ultimate_bearing_capacity_on_slope(
                shallow.strength(),
                regolith.bulk_density(shallow),
                MOON_G,
                Metres::new(1.0),
                beta,
            )
        };
        let just_under = Radians::new(phi.value() * (1.0 - 1e-12));
        let limit = q(just_under).unwrap().value();
        assert!((limit - 1_124.76).abs() <= 0.01 * 1_124.76, "{limit} Pa");
        assert_eq!(q(phi), None);
        let s = SlopeFactors::of(shallow.strength(), just_under).unwrap();
        assert!(s.g_c().abs() < 1e-15, "{s:?}");
        assert!((s.g_gamma() - 0.009_919).abs() < 1e-6, "{s:?}");

        let dust = soil_of(MaterialClass::Dust);
        let level = at(&dust, deep, 0.0).unwrap().value();
        assert!((level - 36_796.0).abs() <= 0.01 * 36_796.0, "{level} Pa");
        let near = at(&dust, deep, 18.0 - 1e-9).unwrap().value();
        assert!(
            (near / level - 0.3377).abs() <= 0.01 * 0.3377,
            "{near} Pa of {level} Pa"
        );
        assert_eq!(at(&dust, deep, 18.0), None);
    }

    /// The slope is refused where it is negative or NaN, and outside the factors' range, as are a
    /// bulk density, gravity or width that is negative or not finite; the factors of a valid slope
    /// are each in [0, 1].
    #[test]
    fn material_slope_factors_are_none_outside_their_range() {
        let soil = lunar_regolith();
        let none = |beta: f64| {
            bearing_capacity_on_slope(&soil, MOON_G, Metres::new(1.0), Radians::new(beta)).is_none()
        };
        for beta in [
            -1e-9,
            -0.1,
            f64::NAN,
            -f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            assert!(none(beta), "{beta}");
            assert_eq!(
                SlopeFactors::of(strength(3.0, 54.0), Radians::new(beta)),
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
                LEVEL,
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
        // Every factor in [0, 1] up to each soil's first `None`: 44.9° at φ 54°, 41.9° at 42°.
        for (soil, last_step) in [(strength(3.0, 54.0), 449), (strength(0.52, 42.0), 419)] {
            for i in 0..=last_step {
                let beta = Radians::new((0.1 * f64::from(i)).to_radians());
                let s = SlopeFactors::of(soil, beta).unwrap();
                for f in [s.g_c(), s.g_gamma()] {
                    assert!((0.0..=1.0).contains(&f), "{s:?} at {beta:?}");
                }
            }
        }
    }
}
