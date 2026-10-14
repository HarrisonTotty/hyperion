//! The ultimate bearing capacity of the ground under a footing: a function of the footing's width
//! and the body's gravity, not a constant of the ground (plan R10, T1.a, Design note 2).
//!
//! # The formula
//!
//! For a square or circular footing of width B resting on the surface, with no surcharge beside it,
//!
//! ```text
//! q_ult = c N_c s_c + ½ ρ g B N_γ s_γ
//! N_q = exp(π tan φ) tan²(45° + φ/2)    N_c = (N_q − 1) cot φ    N_γ = 2 (N_q + 1) tan φ
//! s_c = 1 + N_q ÷ N_c                    s_γ = 0.6                (B = L)
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

use hyperion_base::math;
use hyperion_base::units::{KilogramsPerCubicMetre, Metres, MetresPerSecondSquared, Pascals};

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

/// The ultimate bearing capacity, Pa, of ground of strength `strength` and bulk density
/// `bulk_density` under a square or circular surface footing of width `width` at gravity
/// `gravity`: `q_ult` = c `N_c` `s_c` + ½ ρ g B `N_γ` `s_γ` (the [module](self)'s formula).
#[must_use]
pub fn ultimate_bearing_capacity(
    strength: MohrCoulomb,
    bulk_density: KilogramsPerCubicMetre,
    gravity: MetresPerSecondSquared,
    width: Metres,
) -> Pascals {
    let f = BearingFactors::of(strength);
    let s_c = 1.0 + f.surcharge / f.cohesion;
    let cohesion = strength.cohesion().value() * f.cohesion * s_c;
    let weight = 0.5
        * bulk_density.value()
        * gravity.value()
        * width.value()
        * f.self_weight
        * SQUARE_SELF_WEIGHT_SHAPE;
    Pascals::new(cohesion + weight)
}

/// The ultimate bearing capacity, Pa, of the ground of `p` under a square or circular surface
/// footing of width `footing_width` at gravity `gravity`, from the layer at the depth of one
/// footing width ([`MaterialProperties::shear_layer_at`]) with its bulk density
/// ([`MaterialProperties::bulk_density`]); `None` where the form has no shear table, or for a
/// width or gravity that is negative or not finite (Design note 2).
///
/// It grows with the width and the gravity, the self-weight term in proportion and the layer
/// deepening with the width.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{MaterialPalette, MechanicsFamily, PaletteEntry, PaletteRole};
    use crate::material::{Material, MaterialClass, UNASSIGNED_PHASE_ROW, properties};
    use crate::substance_key::SubstanceKey;
    use hyperion_base::units::{Kelvin, Radians};
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// The Moon's surface gravity, 1.62 m s⁻², as the Lunar Sourcebook takes it (§9.1.9, p. 517).
    const MOON_G: MetresPerSecondSquared = MetresPerSecondSquared::new(1.62);

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
}
